// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use tracing::info;

use sprouter::reconcilers::{ConfigMapSeedReconciler, SecretSeedReconciler, NamespaceReconciler};
use sprouter::sprout::manager::SproutManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting the Sprouter controller...");

    let client = kube::Client::try_default().await?;

    // Initialize the SproutManager
    let sprout_manager = Arc::new(SproutManager::new(client.clone()));
    sprout_manager.init().await?;
    info!("SproutManager initialized.");

    // Create and run reconcilers
    let configmap_reconciler = ConfigMapSeedReconciler::new(client.clone(), sprout_manager.clone());
    let secret_reconciler = SecretSeedReconciler::new(client.clone(), sprout_manager.clone());
    let namespace_reconciler = NamespaceReconciler::new(client, sprout_manager);

    tokio::try_join!(
        configmap_reconciler.run(),
        secret_reconciler.run(),
        namespace_reconciler.run(),
    )?;

    Ok(())
}
