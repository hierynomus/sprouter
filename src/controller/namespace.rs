use std::collections::HashSet;

use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Namespace;
use kube::ResourceExt;
use tracing::info;

use crate::shadower::shadow_all_to_namespace;

pub async fn run(client: Client) -> anyhow::Result<()> {
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
                            shadow_all_to_namespace(client.clone(), &ns_name).await?;
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
