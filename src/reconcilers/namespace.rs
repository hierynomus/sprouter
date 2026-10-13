// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0

//! Namespace reconciler - watches Namespace creation events and fans out existing seeds.

use crate::sprout::manager::SproutManager;
use async_trait::async_trait;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::runtime::{controller::Action, Controller};
use kube::{Api, Client, ResourceExt};
use kube_runtime::watcher::Config as WatcherConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
pub enum NamespaceReconcileError {
    #[error("Kubernetes error: {0}")]
    Kube(#[from] kube::error::Error),
    #[error("Sprouter error: {0}")]
    Sprouter(#[from] anyhow::Error),
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
trait NamespaceLifecycle: Send + Sync {
    async fn new_namespace(&self, namespace: &str) -> anyhow::Result<()>;
}

#[async_trait]
impl NamespaceLifecycle for SproutManager {
    async fn new_namespace(&self, namespace: &str) -> anyhow::Result<()> {
        SproutManager::new_namespace(self, namespace).await
    }
}

pub struct NamespaceReconciler {
    client: Client,
    sprout_manager: Arc<dyn NamespaceLifecycle>,
    seen_namespaces: Mutex<HashMap<String, String>>,
    started_at: Time,
}

impl NamespaceReconciler {
    pub fn new(client: Client, sprout_manager: Arc<SproutManager>, started_at: Time) -> Self {
        Self {
            client,
            sprout_manager,
            seen_namespaces: Mutex::new(HashMap::new()),
            started_at,
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
    reconcile_with_lifecycle(
        ns,
        ctx.sprout_manager.as_ref(),
        &ctx.seen_namespaces,
        &ctx.started_at,
    )
    .await
}

async fn reconcile_with_lifecycle(
    ns: Arc<Namespace>,
    namespace_lifecycle: &dyn NamespaceLifecycle,
    seen_namespaces: &Mutex<HashMap<String, String>>,
    started_at: &Time,
) -> Result<Action, NamespaceReconcileError> {
    let ns_name = ns.name_any();
    let uid = ns.uid().ok_or_else(|| {
        NamespaceReconcileError::Sprouter(anyhow::anyhow!("Namespace '{}' has no UID", ns_name))
    })?;

    debug!("Reconciling namespace: {}", ns_name);

    let mut seen = seen_namespaces.lock().await;
    if seen.get(&ns_name) == Some(&uid) {
        debug!("Namespace '{}' already observed, skipping update", ns_name);
        return Ok(Action::await_change());
    }

    if ns.metadata.creation_timestamp.as_ref() <= Some(started_at) {
        debug!("Namespace '{}' existed before startup, skipping", ns_name);
        seen.insert(ns_name, uid);
        return Ok(Action::await_change());
    }

    drop(seen);

    namespace_lifecycle.new_namespace(&ns_name).await?;

    seen_namespaces.lock().await.insert(ns_name, uid);

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use k8s_openapi::api::core::v1::Namespace;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn time(value: &str) -> Time {
        Time(
            DateTime::parse_from_rfc3339(value)
                .expect("valid RFC3339 timestamp")
                .with_timezone(&Utc),
        )
    }

    fn namespace_with_uid(name: &str, uid: &str, created_at: Time) -> Arc<Namespace> {
        let mut namespace = Namespace::default();
        namespace.metadata.name = Some(name.to_string());
        namespace.metadata.uid = Some(uid.to_string());
        namespace.metadata.creation_timestamp = Some(created_at);
        Arc::new(namespace)
    }

    fn namespace(name: &str, created_at: Time) -> Arc<Namespace> {
        namespace_with_uid(name, name, created_at)
    }

    #[tokio::test]
    async fn reconcile_new_namespace_created_after_startup_calls_lifecycle_once() {
        let started_at = time("2026-01-01T00:00:00Z");
        let seen_namespaces = Mutex::new(HashMap::new());
        let ns = namespace("new-ns", time("2026-01-01T00:00:01Z"));
        let mut lifecycle = MockNamespaceLifecycle::new();
        lifecycle
            .expect_new_namespace()
            .withf(|namespace| namespace == "new-ns")
            .times(1)
            .returning(|_| Ok(()));

        reconcile_with_lifecycle(ns.clone(), &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
        reconcile_with_lifecycle(ns, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reconcile_namespace_created_before_startup_skips_lifecycle() {
        let started_at = time("2026-01-01T00:00:01Z");
        let seen_namespaces = Mutex::new(HashMap::new());
        let ns = namespace("existing-ns", time("2026-01-01T00:00:00Z"));
        let mut lifecycle = MockNamespaceLifecycle::new();
        lifecycle.expect_new_namespace().times(0);

        reconcile_with_lifecycle(ns, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reconcile_failed_new_namespace_is_not_marked_seen() {
        let started_at = time("2026-01-01T00:00:00Z");
        let seen_namespaces = Mutex::new(HashMap::new());
        let ns = namespace("retry-ns", time("2026-01-01T00:00:01Z"));
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut lifecycle = MockNamespaceLifecycle::new();
        lifecycle.expect_new_namespace().times(2).returning({
            let attempts = attempts.clone();
            move |_| {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    anyhow::bail!("temporary failure")
                }
                Ok(())
            }
        });

        assert!(
            reconcile_with_lifecycle(ns.clone(), &lifecycle, &seen_namespaces, &started_at)
                .await
                .is_err()
        );
        reconcile_with_lifecycle(ns, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reconcile_recreated_namespace_with_same_name_calls_lifecycle_again() {
        let started_at = time("2026-01-01T00:00:00Z");
        let seen_namespaces = Mutex::new(HashMap::new());
        let first = namespace_with_uid("reused", "uid-1", time("2026-01-01T00:00:01Z"));
        let replacement = namespace_with_uid("reused", "uid-2", time("2026-01-01T00:00:02Z"));
        let mut lifecycle = MockNamespaceLifecycle::new();
        lifecycle
            .expect_new_namespace()
            .withf(|name| name == "reused")
            .times(2)
            .returning(|_| Ok(()));

        reconcile_with_lifecycle(first, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
        reconcile_with_lifecycle(
            replacement.clone(),
            &lifecycle,
            &seen_namespaces,
            &started_at,
        )
        .await
        .unwrap();
        reconcile_with_lifecycle(replacement, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reconcile_recreated_preexisting_namespace_calls_lifecycle() {
        let started_at = time("2026-01-01T00:00:01Z");
        let seen_namespaces = Mutex::new(HashMap::new());
        let original = namespace_with_uid("reused", "uid-1", time("2026-01-01T00:00:00Z"));
        let replacement = namespace_with_uid("reused", "uid-2", time("2026-01-01T00:00:02Z"));
        let mut lifecycle = MockNamespaceLifecycle::new();
        lifecycle
            .expect_new_namespace()
            .withf(|name| name == "reused")
            .times(1)
            .returning(|_| Ok(()));

        reconcile_with_lifecycle(original, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
        reconcile_with_lifecycle(replacement, &lifecycle, &seen_namespaces, &started_at)
            .await
            .unwrap();
    }
}
