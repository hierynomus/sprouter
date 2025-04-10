use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Resource;

use crate::{propagate, utils::is_shadowed};

pub async fn run(client: Client) -> anyhow::Result<()> {
    let api: Api<ConfigMap> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(cm) if is_shadowed(cm.meta()) => {
                propagate::propagate(cm, client.clone()).await?;
            }
            Event::Delete(cm) if is_shadowed(cm.meta()) => {
                propagate::delete_shadows(cm, client.clone()).await?;
            }
            _ => {}
        }
    }

    Ok(())
}
