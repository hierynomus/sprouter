// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use std::any::type_name_of_val;

use crate::{
    kubernetes::manager::ResourceManager,
    sprout::kind::AsSproutKind,
    utils::{is_sprout, is_sprout_recent},
};
use anyhow::Result;
use kube::api::ResourceExt;

use tracing::{info, warn};

pub async fn grow_sprouts<K, M>(resource: K, manager: &M) -> Result<()>
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + Send
        + Sync
        + 'static
        + AsSproutKind,
    M: ResourceManager<K> + Sync,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let namespaces = manager.list_namespaces().await?;
    let hash = &resource.hash();
    let res = crate::utils::create_sprout(resource, hash);
    let mut created = 0;
    let mut updated = 0;
    let mut ignored = 0;
    let mut validated = 0;
    for target_ns in namespaces {
        if target_ns == src_ns {
            continue;
        }

        // Check if resource already exists in the target namespace
        let pot_sprout = manager.get_in_namespace(&target_ns, &name).await?;
        match pot_sprout {
            Some(s) if is_sprout(s.meta()) && !is_sprout_recent(s.meta(), hash) => {
                info!(
                    "Updating sprout '{}/{}' of '{}/{}'",
                    target_ns, name, src_ns, name
                );
                manager.update_in_namespace(&target_ns, &res).await?;
                updated += 1;
            }
            Some(s) if is_sprout(s.meta()) => {
                validated += 1;
            }
            Some(s) => {
                warn!(
                    "{} '{}/{}' exists but is no sprout",
                    type_name_of_val(&s),
                    target_ns,
                    name
                );
                ignored += 1;
            }
            None => {
                info!(
                    "Creating sprout '{}/{}' of '{}/{}'",
                    target_ns, name, src_ns, name
                );
                manager.create_in_namespace(&target_ns, &res).await?;
                created += 1;
            }
        }
    }

    info!(
        "Growing sprouts of '{}/{}' completed: {} created, {} updated, {} ignored, {} validated",
        src_ns, name, created, updated, ignored, validated
    );
    Ok(())
}

pub async fn delete_sprouts<K, M>(resource: K, manager: &M) -> Result<()>
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
    M: ResourceManager<K> + Sync,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let namespaces = manager.list_namespaces().await?;
    let mut deleted = 0;
    let mut ignored = 0;
    for target_ns in namespaces {
        if target_ns == src_ns {
            continue;
        }

        // Check if the sprout exists in the target namespace
        let pot_sprout = manager.get_in_namespace(&target_ns, &name).await?;
        match pot_sprout {
            Some(sprout) if is_sprout(sprout.meta()) => {
                info!(
                    "Deleting sprout '{}/{}' of '{}/{}'",
                    target_ns, name, src_ns, name
                );
                manager.delete_from_namespace(&target_ns, &name).await?;
                deleted += 1;
            }
            Some(s) => {
                warn!(
                    "{} '{}/{}' exists but is no sprout",
                    type_name_of_val(&s),
                    target_ns,
                    name
                );
                ignored += 1;
            }
            _ => {
                continue;
            }
        }
    }
    info!(
        "Deleting sprouts of '{}/{}' completed: {} deleted, {} ignored",
        src_ns, name, deleted, ignored
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes::manager::MockResourceManager;
    use crate::sprout::kind::AsSproutKind;
    use k8s_openapi::api::core::v1::ConfigMap;
    use std::collections::BTreeMap;

    fn seed_cm(name: &str, ns: &str) -> ConfigMap {
        let mut cm = ConfigMap::default();
        cm.metadata.name = Some(name.to_string());
        cm.metadata.namespace = Some(ns.to_string());
        cm.data = Some(BTreeMap::from([("key".to_string(), "value".to_string())]));
        cm
    }

    /// Builds a sprout whose stored hash matches the seed's current data hash.
    fn up_to_date_sprout(seed: &ConfigMap) -> ConfigMap {
        crate::utils::create_sprout(seed.clone(), &seed.hash())
    }

    /// Builds a sprout whose stored hash does NOT match the seed's current data hash.
    fn stale_sprout(seed: &ConfigMap) -> ConfigMap {
        crate::utils::create_sprout(seed.clone(), &Some("old-hash".to_string()))
    }

    /// A plain ConfigMap with no sprout annotations.
    fn non_sprout_cm(name: &str, ns: &str) -> ConfigMap {
        let mut cm = ConfigMap::default();
        cm.metadata.name = Some(name.to_string());
        cm.metadata.namespace = Some(ns.to_string());
        cm
    }

    // ── grow_sprouts ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn grow_creates_sprout_in_new_namespace() {
        let seed = seed_cm("cfg", "src");
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(|_, _| Ok(None));
        mock.expect_create_in_namespace()
            .once()
            .returning(|_, _| Ok(()));

        grow_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn grow_skips_source_namespace() {
        let seed = seed_cm("cfg", "src");
        let mut mock = MockResourceManager::<ConfigMap>::new();
        // Only the source namespace is returned; nothing else should be called.
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string()]));

        grow_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn grow_updates_stale_sprout() {
        let seed = seed_cm("cfg", "src");
        let stale = stale_sprout(&seed);
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(move |_, _| Ok(Some(stale.clone())));
        mock.expect_update_in_namespace()
            .once()
            .returning(|_, _| Ok(()));

        grow_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn grow_skips_up_to_date_sprout() {
        let seed = seed_cm("cfg", "src");
        let current = up_to_date_sprout(&seed);
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(move |_, _| Ok(Some(current.clone())));
        // No create or update should be called.

        grow_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn grow_ignores_non_sprout_existing_resource() {
        let seed = seed_cm("cfg", "src");
        let other = non_sprout_cm("cfg", "other");
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(move |_, _| Ok(Some(other.clone())));
        // Resource exists but has no sprout annotation — should not create or update.

        grow_sprouts(seed, &mock).await.unwrap();
    }

    // ── delete_sprouts ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_removes_sprout() {
        let seed = seed_cm("cfg", "src");
        let sprout = up_to_date_sprout(&seed);
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(move |_, _| Ok(Some(sprout.clone())));
        mock.expect_delete_from_namespace()
            .once()
            .returning(|_, _| Ok(()));

        delete_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn delete_skips_source_namespace() {
        let seed = seed_cm("cfg", "src");
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string()]));
        // Nothing else should be called.

        delete_sprouts(seed, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn delete_ignores_non_sprout_existing_resource() {
        let seed = seed_cm("cfg", "src");
        let other = non_sprout_cm("cfg", "other");
        let mut mock = MockResourceManager::<ConfigMap>::new();
        mock.expect_list_namespaces()
            .once()
            .returning(|| Ok(vec!["src".to_string(), "other".to_string()]));
        mock.expect_get_in_namespace()
            .once()
            .returning(move |_, _| Ok(Some(other.clone())));
        // Resource is not ours — delete should not be called.

        delete_sprouts(seed, &mock).await.unwrap();
    }
}
