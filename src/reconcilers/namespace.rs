// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0

//! Namespace reconciler - watches Namespace creation events and fans out existing seeds.

use crate::sprout::manager::SproutManager;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Namespace;
use kube::runtime::{controller::Action, Controller};
use kube::{Api, Client, ResourceExt};
use kube_runtime::watcher::Config as WatcherConfig;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
pub enum NamespaceReconcileError {
    #[error("Kubernetes error: {0}")]
    Kube(#[from] kube::error::Error),
    #[error("Sprouter error: {0}")]
    Sprouter(#[from] anyhow::Error),
}

pub struct NamespaceReconciler {
    client: Client,
    sprout_manager: Arc<SproutManager>,
}

impl NamespaceReconciler {
    pub fn new(client: Client, sprout_manager: Arc<SproutManager>) -> Self {
        Self {
            client,
            sprout_manager,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let namespaces: Api<Namespace> = Api::all(self.client.clone());
        let context = Arc::new(self);

        Controller::new(namespaces, WatcherConfig::default())
            .run(reconcile, error_policy, context)
            .for_each(|res| async move {
                match res {
                    Ok(o) => debug!("Reconciled namespace: {:?}", o),
                    Err(e) => warn!("Reconciliation error: {:?}", e),
                }
            })
            .await;

        Ok(())
    }
}

async fn reconcile(
    ns: Arc<Namespace>,
    ctx: Arc<NamespaceReconciler>,
) -> Result<Action, NamespaceReconcileError> {
    let ns_name = ns.name_any();

    debug!("Reconciling namespace: {}", ns_name);

    // When a new namespace is created, fan out all existing seeds into it
    ctx.sprout_manager.new_namespace(&ns_name).await?;

    Ok(Action::await_change())
}

fn error_policy(
    ns: Arc<Namespace>,
    error: &NamespaceReconcileError,
    _ctx: Arc<NamespaceReconciler>,
) -> Action {
    error!(
        "Reconciliation error for namespace {}: {}",
        ns.name_any(),
        error
    );
    Action::requeue(Duration::from_secs(60))
}
