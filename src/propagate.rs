use kube::{Api, Client, api::{ListParams, PostParams, ResourceExt}};
use k8s_openapi::api::core::v1::Namespace;
use anyhow::Result;
use kube::core::NamespaceResourceScope;
use kube::Resource;

pub async fn propagate<K>(resource: K, client: Client) -> Result<()>
where
    K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let all_ns: Api<Namespace> = Api::all(client.clone());
    let namespaces = all_ns.list(&ListParams::default()).await?;

    for ns in namespaces.iter() {
        if ns.name_any() == src_ns {
            continue;
        }
        propagate_to_namespace(resource.clone(), &ns.name_any(), client.clone()).await?;
    }
    Ok(())
}

pub async fn propagate_to_namespace<K>(resource: K, ns: &str, client: Client) -> Result<()>
where
    K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + serde::Serialize + std::fmt::Debug + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    let target_api: Api<K> = Api::namespaced(client.clone(), ns);
    let mut new_res = resource.clone();
    new_res.meta_mut().namespace = Some(ns.to_string());
    new_res.meta_mut().resource_version = None;
    new_res.meta_mut().uid = None;

    let _ = target_api.create(&PostParams::default(), &new_res).await.or_else(|e| {
        if let kube::Error::Api(err) = &e {
            if err.code == 409 {
                return Ok(new_res);
            }
        }
        Err(e)
    });

    Ok(())
}

pub async fn delete_shadows<K>(resource: K, client: Client) -> Result<()>
where
    K: kube::Resource<Scope = NamespaceResourceScope> + Clone + serde::de::DeserializeOwned + std::fmt::Debug + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    let name = resource.name_any();
    let src_ns = resource.namespace().unwrap_or_default();
    let all_ns: Api<Namespace> = Api::all(client.clone());
    let namespaces = all_ns.list(&ListParams::default()).await?;
    for ns in namespaces.iter() {
        if ns.name_any() == src_ns {
            continue;
        }
        let api: Api<K> = Api::namespaced(client.clone(), &ns.name_any());
        let _ = api.delete(&name, &Default::default()).await;
    }
    Ok(())
}

pub async fn propagate_all_to_namespace(ns: &str, client: Client) -> Result<()> {
    let cm_api: Api<k8s_openapi::api::core::v1::ConfigMap> = Api::all(client.clone());
    let cms = cm_api.list(&ListParams::default()).await?;
    for cm in cms {
        if crate::utils::is_shadowed(cm.meta()) {
            propagate_to_namespace(cm, ns, client.clone()).await?;
        }
    }
    let sec_api: Api<k8s_openapi::api::core::v1::Secret> = Api::all(client.clone());
    let secs = sec_api.list(&ListParams::default()).await?;
    for sec in secs {
        if crate::utils::is_shadowed(sec.meta()) {
            propagate_to_namespace(sec, ns, client.clone()).await?;
        }
    }
    Ok(())
}
