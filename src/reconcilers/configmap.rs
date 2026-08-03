// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0

//! ConfigMap seed reconciler - watches ConfigMaps and propagates those marked as seeds.

use crate::sprout::manager::SproutManager;
use crate::utils::{has_finalizer, is_being_deleted, is_seed};
use async_trait::async_trait;
use futures::StreamExt;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::runtime::{controller::Action, Controller};
use kube::{Api, Client, ResourceExt};
use kube_runtime::watcher::Config as WatcherConfig;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
pub enum ConfigMapReconcileError {
    #[error("Kubernetes error: {0}")]
    Kube(#[from] kube::error::Error),
    #[error("Error: {0}")]
    Other(#[from] anyhow::Error),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
trait ConfigMapSeedLifecycle: Send + Sync {
    async fn add_seed(&self, resource: ConfigMap) -> anyhow::Result<()>;
    async fn delete_seed(&self, resource: ConfigMap) -> anyhow::Result<()>;
}

#[async_trait]
impl ConfigMapSeedLifecycle for SproutManager {
    async fn add_seed(&self, resource: ConfigMap) -> anyhow::Result<()> {
        SproutManager::add_seed(self, resource).await
    }

    async fn delete_seed(&self, resource: ConfigMap) -> anyhow::Result<()> {
        SproutManager::delete_seed(self, resource).await
    }
}

pub struct ConfigMapSeedReconciler {
    client: Client,
    sprout_manager: Arc<dyn ConfigMapSeedLifecycle>,
}

impl ConfigMapSeedReconciler {
    pub fn new(client: Client, sprout_manager: Arc<SproutManager>) -> Self {
        Self {
            client,
            sprout_manager,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let configmaps: Api<ConfigMap> = Api::all(self.client.clone());
        let context = Arc::new(self);

        Controller::new(configmaps, WatcherConfig::default())
            .run(reconcile, error_policy, context)
            .for_each(|res| async move {
                match res {
                    Ok(o) => debug!("Reconciled configmap: {:?}", o),
                    Err(e) => warn!("Reconciliation error: {:?}", e),
                }
            })
            .await;

        Ok(())
    }
}

async fn reconcile(
    cm: Arc<ConfigMap>,
    ctx: Arc<ConfigMapSeedReconciler>,
) -> Result<Action, ConfigMapReconcileError> {
    reconcile_with_lifecycle(cm, ctx.sprout_manager.as_ref()).await
}

async fn reconcile_with_lifecycle(
    cm: Arc<ConfigMap>,
    seed_lifecycle: &dyn ConfigMapSeedLifecycle,
) -> Result<Action, ConfigMapReconcileError> {
    let name = cm.name_any();
    let namespace = cm.namespace().unwrap_or_default();
    let meta = &(*cm).metadata;

    debug!("Reconciling configmap: {}/{}", namespace, name);

    // If being deleted, clean up sprouts and remove finalizer
    if is_being_deleted(meta) && has_finalizer(meta) {
        debug!("ConfigMap {}/{} is being deleted", namespace, name);
        seed_lifecycle.delete_seed((*cm).clone()).await?;
        return Ok(Action::await_change());
    }

    // Check if this is a seed resource
    if !is_seed(meta) {
        debug!(
            "ConfigMap {}/{} does not have seed annotation, skipping",
            namespace, name
        );
        return Ok(Action::await_change());
    }

    // If not being deleted but doesn't have finalizer, add it
    if !is_being_deleted(meta) && !has_finalizer(meta) {
        debug!("Adding seed configmap {}/{}", namespace, name);
    }

    // Keep SproutManager seed index in sync and handle finalizer/sprout propagation.
    seed_lifecycle.add_seed((*cm).clone()).await?;

    Ok(Action::await_change())
}

fn error_policy(
    cm: Arc<ConfigMap>,
    error: &ConfigMapReconcileError,
    _ctx: Arc<ConfigMapSeedReconciler>,
) -> Action {
    error!(
        "Reconciliation error for configmap {}/{}: {}",
        cm.namespace().unwrap_or_default(),
        cm.name_any(),
        error
    );
    Action::requeue(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::ConfigMap;
    use serde_json::json;

    fn make_configmap(seed: bool, deleting: bool, with_finalizer: bool) -> Arc<ConfigMap> {
        let mut metadata = json!({
            "name": "seed-cm",
            "namespace": "default"
        });

        if seed {
            metadata["annotations"] = json!({
                "sprouter.geeko.me/enabled": "true"
            });
        }

        if deleting {
            metadata["deletionTimestamp"] = json!("2026-01-01T00:00:00Z");
        }

        if with_finalizer {
            metadata["finalizers"] = json!(["sprouter.geeko.me/finalizer"]);
        }

        Arc::new(
            serde_json::from_value(json!({
                "apiVersion": "v1",
                "kind": "ConfigMap",
                "metadata": metadata,
                "data": {}
            }))
            .expect("valid ConfigMap JSON"),
        )
    }

    #[tokio::test]
    async fn reconcile_seed_calls_add_seed() {
        let mut lifecycle = MockConfigMapSeedLifecycle::new();
        lifecycle.expect_add_seed().times(1).returning(|_| Ok(()));
        lifecycle.expect_delete_seed().times(0);

        let cm = make_configmap(true, false, false);
        let result = reconcile_with_lifecycle(cm, &lifecycle).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn reconcile_terminating_seed_calls_delete_seed() {
        let mut lifecycle = MockConfigMapSeedLifecycle::new();
        lifecycle.expect_delete_seed().times(1).returning(|_| Ok(()));
        lifecycle.expect_add_seed().times(0);

        let cm = make_configmap(true, true, true);
        let result = reconcile_with_lifecycle(cm, &lifecycle).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn reconcile_non_seed_does_not_call_seed_lifecycle() {
        let mut lifecycle = MockConfigMapSeedLifecycle::new();
        lifecycle.expect_add_seed().times(0);
        lifecycle.expect_delete_seed().times(0);

        let cm = make_configmap(false, false, false);
        let result = reconcile_with_lifecycle(cm, &lifecycle).await;

        assert!(result.is_ok());
    }
}
