use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::Resource;
use kube::ResourceExt;

use tracing::info;

use crate::sprout;
use crate::utils::is_seed;

pub async fn run(client: Client, sprout_manager: &sprout::manager::SproutManager) -> anyhow::Result<()> {
    let api: Api<Secret> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    info!("Starting Secret watcher...");
    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(sec) if sprout_manager.is_known_seed(sec.clone()).await => {
                if is_seed(sec.meta()) {
                    info!("Secret '{}/{}' updated", sec.namespace().unwrap_or_default(), sec.name_any());
                    sprout_manager.add_seed(sec.clone()).await?;
                } else {
                    info!("Secret '{}/{}' is known, but no longer seed, deleting", sec.namespace().unwrap_or_default(), sec.name_any());
                    sprout_manager.delete_seed(sec.clone()).await?;
                }
            }
            Event::Apply(sec) if is_seed(sec.meta()) => {
                info!("Secret '{}/{}' created, growing sprouts", sec.namespace().unwrap_or_default(), sec.name_any());
                sprout_manager.add_seed(sec.clone()).await?;
            }
            Event::Delete(sec) if is_seed(sec.meta()) => {
                info!("Secret {} deleted", sec.name_any());
                sprout_manager.delete_seed(sec.clone()).await?;
            }
            _ => {}
        }
    }

    info!("Secret watcher stopped.");

    Ok(())
}
