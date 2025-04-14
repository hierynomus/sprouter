use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Resource;
use kube::ResourceExt;

use tracing::info;

use crate::kubernetes::manager::KubeResourceManager;
use crate::{shadower, utils::shadow_enabled};

pub async fn run(client: Client) -> anyhow::Result<()> {
    let api: Api<ConfigMap> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    info!("Starting ConfigMap watcher...");
    while let Some(event) = watcher.try_next().await? {
        let mgr = KubeResourceManager::<ConfigMap>::new(client.clone());
        match event {
            Event::Apply(cm) if shadow_enabled(cm.meta()) => {
                info!("ConfigMap {} created or updated", cm.name_any());
                shadower::shadow(cm, &mgr).await?;
            }
            Event::Delete(cm) if shadow_enabled(cm.meta()) => {
                info!("ConfigMap {} deleted", cm.name_any());
                shadower::delete_shadows(cm, &mgr).await?;
            }
            _ => {}
        }
    }
    info!("ConfigMap watcher stopped.");

    Ok(())
}
