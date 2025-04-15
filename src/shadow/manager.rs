use std::collections::HashSet;

use anyhow::Result;
use k8s_openapi::{api::core::v1::{ConfigMap, Secret}, NamespaceResourceScope};
use kube::{api::ListParams, Api, Client};
use tracing::info;
use kube::ResourceExt;
use crate::{kubernetes::manager::{KubeResourceManager, ResourceManager}, shadow::kind::{kind_of, AsShadowKind, ShadowKind}, shadower::{cast_shadow, delete_shadows}};
use crate::utils::shadow_enabled;
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Eq, Hash, PartialEq, Clone)]
pub struct Shadow {
    name: String,
    namespace: String,
    resource_type: ShadowKind,
}

pub struct ShadowManager {
    client: Client,
    shadows: Arc<RwLock<HashSet<Shadow>>>,
}

impl ShadowManager {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            shadows: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn init(&self) -> Result<()> {
        self.init_shadows::<ConfigMap>().await?;
        self.init_shadows::<Secret>().await?;

        let lock = self.shadows.read().await;
        info!("ShadowManager initialized with {} shadows", lock.len());

        Ok(())
    }

    async fn init_shadows<K>(&self) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + Sync + Send + std::fmt::Debug + 'static + AsShadowKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        let api: Api<K> = Api::all(self.client.clone());
        let shadows = api.list(&ListParams::default()).await?;
        for shadow in shadows {
            if shadow_enabled(shadow.meta()) {
                self.add_shadow(shadow).await?;
            }
        }
        Ok(())
    }

    pub async fn add_shadow<K>(&self, resource: K) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + Sync + Send + std::fmt::Debug + 'static + AsShadowKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        info!("Casting shadows for '{}/{}'", resource.namespace().unwrap_or_default(), resource.name_any());
        let mut lock = self.shadows.write().await;
        lock.insert(
            Shadow {
                name: resource.name_any(),
                namespace: resource.namespace().unwrap_or_default(),
                resource_type: kind_of(&resource),
            }
        );
        let mgr = KubeResourceManager::<K>::new(self.client.clone());
        cast_shadow(resource.clone(), &mgr).await?;
        Ok(())
    }

    pub async fn delete_shadow<K>(&self, resource: K) -> Result<()>
    where
        K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + Sync + Send + std::fmt::Debug + 'static + AsShadowKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        info!("Deleting shadows for '{}/{}'", resource.namespace().unwrap_or_default(), resource.name_any());
        let mut lock = self.shadows.write().await;
        lock.remove(
            &Shadow {
                name: resource.name_any(),
                namespace: resource.namespace().unwrap_or_default(),
                resource_type: kind_of(&resource),
            }
        );
        let mgr = KubeResourceManager::<K>::new(self.client.clone());
        delete_shadows(resource.clone(), &mgr).await?;
        Ok(())
    }

    pub async fn is_known_shadow<K>(&self, resource: K) -> bool
    where
        K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + Sync + Send + std::fmt::Debug + 'static + AsShadowKind,
        <K as kube::Resource>::DynamicType: Default,
    {
        let lock = self.shadows.read().await;
        lock.contains(
            &Shadow {
                name: resource.name_any(),
                namespace: resource.namespace().unwrap_or_default(),
                resource_type: kind_of(&resource),
            }
        )
    }

    pub async fn new_namespace(&self, namespace: &str) -> Result<()> {
        let lock = self.shadows.read().await;

        for shadow in lock.iter() {
            if shadow.namespace == namespace {
                continue;
            }
            info!("Casting shadow of {} '{}/{}' to '{}/{}", shadow.resource_type, shadow.namespace, shadow.name, namespace, shadow.name);
            match shadow.resource_type {
                ShadowKind::ConfigMap => {
                    let cm_api: Api<ConfigMap> = Api::namespaced(self.client.clone(), &shadow.namespace);
                    let cm = cm_api.get(&shadow.name).await?;
                    let mgr = KubeResourceManager::<ConfigMap>::new(self.client.clone());
                    let shadow_cm = crate::utils::create_shadow(cm);
                    mgr.create_in_namespace(namespace, &shadow_cm).await?;
                }
                ShadowKind::Secret => {
                    let sec_api: Api<Secret> = Api::namespaced(self.client.clone(), &shadow.namespace);
                    let sec = sec_api.get(&shadow.name).await?;
                    let mgr = KubeResourceManager::<Secret>::new(self.client.clone());
                    let shadow_sec = crate::utils::create_shadow(sec);
                    mgr.create_in_namespace(namespace, &shadow_sec).await?;
                }
            }
        }

        info!("All known shadows cast to new namespace '{}'", namespace);
        Ok(())
    }
}
