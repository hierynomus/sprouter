use kube::{Api, Client};
use kube_runtime::watcher::{watcher, Event, Config as WatcherConfig};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Namespace;
use kube::ResourceExt;

use crate::propagate::propagate_all_to_namespace;

pub async fn run(client: Client) -> anyhow::Result<()> {
    let ns_api: Api<Namespace> = Api::all(client.clone());
    let mut watcher = watcher(ns_api, WatcherConfig::default()).boxed();

    while let Some(event) = watcher.try_next().await? {
        match event {
            Event::Apply(ns) => {
                let ns_name = ns.name_any();
                propagate_all_to_namespace(&ns_name, client.clone()).await?;
            }
            _ => {}
        }
    }

    Ok(())
}
