use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::Resource;
use kube::ResourceExt;

use tracing::info;

use crate::kubernetes::manager::KubeResourceManager;
use crate::{shadower, utils::shadow_enabled};

pub async fn run(client: Client) -> anyhow::Result<()> {
    let api: Api<Secret> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    info!("Starting Secret watcher...");
    while let Some(event) = watcher.try_next().await? {
        let mgr = KubeResourceManager::<Secret>::new(client.clone());
        match event {
            Event::Apply(secret) if shadow_enabled(secret.meta()) => {
                info!("Secret {} created or updated", secret.name_any());
                shadower::shadow(secret, &mgr).await?;
            }
            Event::Delete(secret) if shadow_enabled(secret.meta()) => {
                info!("Secret {} deleted", secret.name_any());
                shadower::delete_shadows(secret, &mgr).await?;
            }
            _ => {}
        }
    }

    info!("Secret watcher stopped.");

    Ok(())
}
