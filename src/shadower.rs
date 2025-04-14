use std::any::type_name_of_val;

use kube::{Api, Client, api::{ListParams, ResourceExt}};
use k8s_openapi::api::core::v1::{Secret, ConfigMap};
use anyhow::Result;
use crate::kubernetes::manager::{KubeResourceManager, ResourceManager};
use kube::Resource;

use tracing::{info, warn};

pub async fn shadow<K, M>(resource: K, manager: &M) -> Result<()>
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug + Send + Sync + 'static,
    M: ResourceManager<K> + Sync,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let namespaces = manager.list_namespaces().await?;

    let res = crate::utils::create_shadow(resource);
    for target_ns in namespaces {
        if target_ns == src_ns {
            continue;
        }

        // Check if resource already exists in the target namespace
        let pot_shadow = manager.get_in_namespace(&target_ns, &name).await?;
        match pot_shadow {
            Some(s) if s.annotations().contains_key(crate::utils::SHADOW_KEY) => {
                info!("Shadow of '{}/{}' already exists in namespace '{}'", src_ns, name, target_ns);
                manager.update_in_namespace(&target_ns, &res).await?;
            }
            Some(s) => {
                warn!("{} '{}' in namespace '{}' exists but is no shadow", type_name_of_val(&s), name, target_ns);
            }
            None => {
                info!("Creating shadow of '{}/{}' in namespace '{}'", src_ns, name, target_ns);
                manager.create_in_namespace(&target_ns, &res).await?;
            }
        }
    }
    Ok(())
}

pub async fn delete_shadows<K, M>(resource: K, manager: &M) -> Result<()>
where
    K: kube::Resource<Scope = kube::core::NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug + Send + Sync + 'static,
    M: ResourceManager<K> + Sync,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let namespaces = manager.list_namespaces().await?;
    for target_ns in namespaces {
        if target_ns == src_ns {
            continue;
        }

        // Check if the shadow exists in the target namespace
        let pot_shadow = manager.get_in_namespace(&target_ns, &name).await?;
        match pot_shadow {
            Some(shadow) if shadow.annotations().contains_key(crate::utils::SHADOW_KEY) => {
                info!("Deleting shadow of '{}/{}' in namespace '{}'", src_ns, name, target_ns);
                manager.delete_from_namespace(&target_ns, &name).await?;
            }
            Some(s) => {
                info!("{} '{}' in namespace '{}' does not have the shadow annotation", type_name_of_val(&s), name, target_ns);
            }
            _ => {
                continue;
            }
        }
    }
    Ok(())
}

pub async fn shadow_all_to_namespace(client: Client, ns: &str) -> Result<()> {
    let cm_api: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::all(client.clone());
    let cms = cm_api.list(&ListParams::default()).await?;
    for cm in cms {
        if crate::utils::shadow_enabled(cm.meta()) {
            let mgr = KubeResourceManager::<ConfigMap>::new(client.clone());

            let shadow_cm = crate::utils::create_shadow(cm);
            mgr.create_in_namespace(ns, &shadow_cm).await?;
        }
    }
    let sec_api: Api<k8s_openapi::api::core::v1::Secret> = Api::all(client.clone());
    let secs = sec_api.list(&ListParams::default()).await?;
    for sec in secs {
        if crate::utils::shadow_enabled(sec.meta()) {
            let mgr = KubeResourceManager::<Secret>::new(client.clone());
            let shadow_sec = crate::utils::create_shadow(sec);
            mgr.create_in_namespace(ns, &shadow_sec).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes::manager::MockResourceManager;
    use k8s_openapi::api::core::v1::ConfigMap;
    use std::collections::BTreeMap;

    fn make_annotated_configmap(name: &str, namespace: &str) -> ConfigMap {
        ConfigMap {
            metadata: kube::api::ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                annotations: Some(BTreeMap::from([(
                    crate::utils::ANNOTATION_KEY.to_string(), "true".to_string(),
                )])),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_shadow_creates_in_other_namespaces() {
        let mut mock = MockResourceManager::<ConfigMap>::new();
        let cm = make_annotated_configmap("test", "default");

        mock.expect_list_namespaces()
            .returning(|| Ok(vec!["default".to_string(), "dev".to_string(), "test".to_string()]));

        let name = cm.metadata.name.clone().unwrap();

        mock.expect_create_in_namespace()
            .times(2)
            .withf(move |ns, r| ns != "default" && r.name_any() == name)
            .returning(move |_, _| Ok(()));

        shadow(cm, &mock).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_shadows_removes_from_other_namespaces() {
        let mut mock = MockResourceManager::<ConfigMap>::new();
        let cm = make_annotated_configmap("to-delete", "ns-a");

        mock.expect_list_namespaces()
            .returning(|| Ok(vec!["ns-a".to_string(), "ns-b".to_string()]));

        mock.expect_delete_from_namespace()
            .withf(|ns, name| ns == "ns-b" && name == "to-delete")
            .returning(|_, _| Ok(()));

        delete_shadows(cm, &mock).await.unwrap();
    }
}
