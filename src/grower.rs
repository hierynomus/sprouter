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
