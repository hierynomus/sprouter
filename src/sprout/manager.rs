// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use std::collections::HashSet;

use crate::utils::is_seed;
use crate::{
    grower::{delete_sprouts, grow_sprouts},
    kubernetes::manager::{KubeResourceManager, ResourceManager},
    sprout::kind::{AsSproutKind, SproutKind, kind_of},
};
use anyhow::Result;
use k8s_openapi::{
    NamespaceResourceScope,
    api::core::v1::{ConfigMap, Secret},
};
use kube::ResourceExt;
use kube::{Api, Client, api::ListParams};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Eq, Hash, PartialEq, Clone)]
pub struct Seed {
    name: String,
    namespace: String,
    resource_type: SproutKind,
}

pub struct SproutManager {
    client: Client,
    seeds: Arc<RwLock<HashSet<Seed>>>,
}

impl SproutManager {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            seeds: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn init(&self) -> Result<()> {
        self.init_seeds::<ConfigMap>().await?;
        self.init_seeds::<Secret>().await?;

        let lock = self.seeds.read().await;
        info!("SproutManager initialized with {} seeds", lock.len());

        Ok(())
    }

    async fn init_seeds<K>(&self) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope>
            + Clone
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Sync
            + Send
            + std::fmt::Debug
            + 'static
            + AsSproutKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        let api: Api<K> = Api::all(self.client.clone());
        let pot_seeds = api.list(&ListParams::default()).await?;
        for pot_seed in pot_seeds {
            if is_seed(pot_seed.meta()) {
                self.add_seed(pot_seed).await?;
            }
        }
        Ok(())
    }

    pub async fn add_seed<K>(&self, resource: K) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope>
            + Clone
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Sync
            + Send
            + std::fmt::Debug
            + 'static
            + AsSproutKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        info!(
            "Growing sprouts for '{}/{}'",
            resource.namespace().unwrap_or_default(),
            resource.name_any()
        );
        let mut lock: tokio::sync::RwLockWriteGuard<'_, HashSet<Seed>> = self.seeds.write().await;
        lock.insert(Seed {
            name: resource.name_any(),
            namespace: resource.namespace().unwrap_or_default(),
            resource_type: kind_of(&resource),
        });
        let mgr = KubeResourceManager::<K>::new(self.client.clone());
        grow_sprouts(resource.clone(), &mgr).await?;
        Ok(())
    }

    pub async fn delete_seed<K>(&self, resource: K) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope>
            + Clone
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Sync
            + Send
            + std::fmt::Debug
            + 'static
            + AsSproutKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        info!(
            "Deleting sprouts for '{}/{}'",
            resource.namespace().unwrap_or_default(),
            resource.name_any()
        );
        let mut lock = self.seeds.write().await;
        lock.remove(&Seed {
            name: resource.name_any(),
            namespace: resource.namespace().unwrap_or_default(),
            resource_type: kind_of(&resource),
        });
        let mgr = KubeResourceManager::<K>::new(self.client.clone());
        delete_sprouts(resource.clone(), &mgr).await?;
        Ok(())
    }

    pub async fn is_known_seed<K>(&self, resource: K) -> bool
    where
        K: kube::Resource<Scope = NamespaceResourceScope>
            + Clone
            + serde::de::DeserializeOwned
            + serde::Serialize
            + Sync
            + Send
            + std::fmt::Debug
            + 'static
            + AsSproutKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        let lock = self.seeds.read().await;
        lock.contains(&Seed {
            name: resource.name_any(),
            namespace: resource.namespace().unwrap_or_default(),
            resource_type: kind_of(&resource),
        })
    }

    pub async fn new_namespace(&self, namespace: &str) -> Result<()> {
        let lock = self.seeds.read().await;

        for seed in lock.iter() {
            if seed.namespace == namespace {
                continue;
            }
            info!(
                "Growing sprout of {} '{}/{}' to '{}/{}",
                seed.resource_type, seed.namespace, seed.name, namespace, seed.name
            );
            match seed.resource_type {
                SproutKind::ConfigMap => {
                    let cm_api: Api<ConfigMap> =
                        Api::namespaced(self.client.clone(), &seed.namespace);
                    let cm = cm_api.get(&seed.name).await?;
                    let mgr = KubeResourceManager::<ConfigMap>::new(self.client.clone());
                    let h = &cm.hash();
                    let sprout = crate::utils::create_sprout(cm, h);
                    mgr.create_in_namespace(namespace, &sprout).await?;
                }
                SproutKind::Secret => {
                    let sec_api: Api<Secret> =
                        Api::namespaced(self.client.clone(), &seed.namespace);
                    let sec = sec_api.get(&seed.name).await?;
                    let mgr = KubeResourceManager::<Secret>::new(self.client.clone());
                    let h = &sec.hash();
                    let sprout = crate::utils::create_sprout(sec, h);
                    mgr.create_in_namespace(namespace, &sprout).await?;
                }
            }
        }

        info!("All known seeds sprouted in new namespace '{}'", namespace);
        Ok(())
    }
}
