use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Resource;
use kube::ResourceExt;

use tracing::info;

use crate::kubernetes::manager::KubeResourceManager;
use crate::shadow::manager::ShadowManager;
use crate::{shadower, utils::shadow_enabled};

pub async fn run(client: Client, shadow_manager: &ShadowManager) -> anyhow::Result<()> {
    let api: Api<ConfigMap> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    info!("Starting ConfigMap watcher...");
    while let Some(event) = watcher.try_next().await? {
        let mgr = KubeResourceManager::<ConfigMap>::new(client.clone());
        match event {
            Event::Apply(cm) if shadow_manager.is_known_shadow(cm.clone()).await => {
                if shadow_enabled(cm.meta()) {
                    info!("ConfigMap '{}/{}' updated", cm.namespace().unwrap_or_default(), cm.name_any());
                    shadow_manager.add_shadow(cm.clone()).await?;
                } else {
                    info!("ConfigMap '{}/{}' is known, but no longer shadow, deleting", cm.namespace().unwrap_or_default(), cm.name_any());
                    shadow_manager.delete_shadow(cm.clone()).await?;
                }
            }
            Event::Apply(cm) if shadow_enabled(cm.meta()) => {
                info!("ConfigMap '{}/{}' created, casting shadows", cm.namespace().unwrap_or_default(), cm.name_any());
                shadow_manager.add_shadow(cm.clone()).await?;
            }
            Event::Delete(cm) if shadow_enabled(cm.meta()) => {
                info!("ConfigMap {} deleted", cm.name_any());
                shadow_manager.delete_shadow(cm.clone()).await?;
            }
            _ => {}
        }
    }
    info!("ConfigMap watcher stopped.");

    Ok(())
}
