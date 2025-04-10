use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::Resource;

use crate::{propagate, utils::is_shadowed};

pub async fn run(client: Client) -> anyhow::Result<()> {
    let api: Api<Secret> = Api::all(client.clone());
    let mut watcher = watcher(api, WatcherConfig::default()).boxed();

    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(secret) if is_shadowed(secret.meta()) => {
                propagate::propagate(secret, client.clone()).await?;
            }
            Event::Delete(secret) if is_shadowed(secret.meta()) => {
                propagate::delete_shadows(secret, client.clone()).await?;
            }
            _ => {}
        }
    }

    Ok(())
}
