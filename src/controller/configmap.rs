// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Resource;
use kube::ResourceExt;
use kube::{Api, Client};
use kube_runtime::watcher::{Config as WatcherConfig, Event, watcher};

use tracing::info;

use crate::sprout::manager::SproutManager;
use crate::utils::is_seed;

pub async fn run(client: Client, sprout_manager: &SproutManager) -> anyhow::Result<()> {
    let api: Api<ConfigMap> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    info!("Starting ConfigMap watcher...");
    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(cm) if sprout_manager.is_known_seed(cm.clone()).await => {
                if is_seed(cm.meta()) {
                    info!(
                        "ConfigMap '{}/{}' updated",
                        cm.namespace().unwrap_or_default(),
                        cm.name_any()
                    );
                    sprout_manager.add_seed(cm.clone()).await?;
                } else {
                    info!(
                        "ConfigMap '{}/{}' is known, but no longer seed, deleting",
                        cm.namespace().unwrap_or_default(),
                        cm.name_any()
                    );
                    sprout_manager.delete_seed(cm.clone()).await?;
                }
            }
            Event::Apply(cm) if is_seed(cm.meta()) => {
                info!(
                    "ConfigMap '{}/{}' created, growing sprouts",
                    cm.namespace().unwrap_or_default(),
                    cm.name_any()
                );
                sprout_manager.add_seed(cm.clone()).await?;
            }
            Event::Delete(cm) if is_seed(cm.meta()) => {
                info!("ConfigMap {} deleted", cm.name_any());
                sprout_manager.delete_seed(cm.clone()).await?;
            }
            _ => {}
        }
    }
    info!("ConfigMap watcher stopped.");

    Ok(())
}
