// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::core::NamespaceResourceScope;
use kube::{
    Api, Client,
    api::{ListParams, PostParams, ResourceExt},
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ResourceManager<K>
where
    K: kube::Resource<Scope = NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + Send
        + Sync,
    <K as kube::Resource>::DynamicType: Default,
{
    async fn list_namespaces(&self) -> Result<Vec<String>>;
    async fn create_in_namespace(&self, ns: &str, resource: &K) -> Result<()>;
    async fn update_in_namespace(&self, ns: &str, resource: &K) -> Result<()>;
    async fn delete_from_namespace(&self, ns: &str, name: &str) -> Result<()>;
    async fn get_in_namespace(&self, ns: &str, name: &str) -> Result<Option<K>>;
}

pub struct KubeResourceManager<K>
where
    K: kube::Resource<Scope = NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    _marker: std::marker::PhantomData<K>,
    client: Client,
}

impl<K> KubeResourceManager<K>
where
    K: kube::Resource<Scope = NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    pub fn new(client: Client) -> Self {
        Self {
            _marker: std::marker::PhantomData,
            client,
        }
    }
}

#[async_trait::async_trait]
impl<K> ResourceManager<K> for KubeResourceManager<K>
where
    K: kube::Resource<Scope = NamespaceResourceScope>
        + Clone
        + serde::de::DeserializeOwned
        + serde::Serialize
        + std::fmt::Debug
        + Send
        + Sync
        + 'static,
    <K as kube::Resource>::DynamicType: Default,
{
    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let ns_api: Api<Namespace> = Api::all(self.client.clone());
        let namespaces = ns_api.list(&ListParams::default()).await?;
        Ok(namespaces.iter().map(|n| n.name_any()).collect())
    }

    async fn create_in_namespace(&self, ns: &str, resource: &K) -> Result<()> {
        let api: Api<K> = Api::namespaced(self.client.clone(), ns);
        let mut res = resource.clone();
        res.meta_mut().namespace = Some(ns.to_string());
        res.meta_mut().resource_version = None;
        res.meta_mut().uid = None;

        let _ = api.create(&PostParams::default(), &res).await.or_else(|e| {
            if let kube::Error::Api(err) = &e {
                if err.code == 409 {
                    return Ok(res);
                }
            }
            Err(e)
        });
        Ok(())
    }

    async fn update_in_namespace(&self, ns: &str, resource: &K) -> Result<()> {
        let api: Api<K> = Api::namespaced(self.client.clone(), ns);
        let mut res = resource.clone();
        res.meta_mut().namespace = Some(ns.to_string());
        res.meta_mut().resource_version = None;
        res.meta_mut().uid = None;

        api.replace(&res.name_any(), &PostParams::default(), &res)
            .await?;
        Ok(())
    }

    async fn delete_from_namespace(&self, ns: &str, name: &str) -> Result<()> {
        let api: Api<K> = Api::namespaced(self.client.clone(), ns);
        let _ = api.delete(name, &Default::default()).await;
        Ok(())
    }

    async fn get_in_namespace(&self, ns: &str, name: &str) -> Result<Option<K>> {
        let api: Api<K> = Api::namespaced(self.client.clone(), ns);
        let res = api.get_opt(name).await?;
        Ok(res)
    }
}
