// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0

//! ConfigMap seed reconciler - watches ConfigMaps and propagates those marked as seeds.

use crate::grower::{delete_sprouts, grow_sprouts};
use crate::kubernetes::manager::{KubeResourceManager, ResourceManager};
use crate::sprout::manager::SproutManager;
use crate::utils::{has_finalizer, is_being_deleted, is_seed};
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

pub struct ConfigMapSeedReconciler {
    client: Client,
    sprout_manager: Arc<SproutManager>,
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
    let name = cm.name_any();
    let namespace = cm.namespace().unwrap_or_default();
    let meta = &(*cm).metadata;

    debug!("Reconciling configmap: {}/{}", namespace, name);

    // If being deleted, clean up sprouts and remove finalizer
    if is_being_deleted(meta) && has_finalizer(meta) {
        debug!("ConfigMap {}/{} is being deleted", namespace, name);
        let mgr = KubeResourceManager::<ConfigMap>::new(ctx.client.clone());
        let _ = delete_sprouts((*cm).clone(), &mgr).await;
        mgr.remove_finalizer(&namespace, &name).await?;
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
        debug!("Adding finalizer to seed configmap {}/{}", namespace, name);
        let mgr = KubeResourceManager::<ConfigMap>::new(ctx.client.clone());
        mgr.add_finalizer(&namespace, &name).await?;
    }

    // This is a seed resource - grow sprouts
    debug!("Growing sprouts for seed configmap {}/{}", namespace, name);
    let mgr = KubeResourceManager::<ConfigMap>::new(ctx.client.clone());
    let _ = grow_sprouts((*cm).clone(), &mgr).await;

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
