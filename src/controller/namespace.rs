// Copyright 2025, Jeroen van Erp <jeroen@geeko.me>
// SPDX-License-Identifier: Apache-2.0
use std::collections::HashSet;

use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Namespace;
use kube::ResourceExt;
use kube::{Api, Client};
use kube_runtime::watcher::{Config as WatcherConfig, Event, watcher};
use tracing::info;

use crate::sprout::manager::SproutManager;

pub async fn run(client: Client, sprout_manager: &SproutManager) -> anyhow::Result<()> {
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let mut watcher = watcher(ns_api, WatcherConfig::default()).boxed();
    let mut seen = HashSet::new();

    info!("Starting Namespace watcher...");
    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(ref ns) => {
                let ns_name = ns.name_any();
                match &ns.status {
                    Some(status) if status.phase == Some("Active".to_string()) => {
                        // Ensure the namespace is not already seen
                        if seen.insert(ns_name.clone()) {
                            info!("Namespace '{}' created or updated", ns_name);
                            sprout_manager.new_namespace(&ns_name).await?;
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
            Event::Delete(ns) => {
                info!("Namespace '{}' deleted", ns.name_any());
                seen.remove(&ns.name_any());
            }
            _ => {}
        }
    }

    info!("Namespace watcher stopped.");
    Ok(())
}
