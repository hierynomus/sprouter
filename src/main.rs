mod controller;
mod propagate;
mod utils;
mod types;

use controller::{configmap, namespace, secret};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let client = kube::Client::try_default().await?;

    tokio::try_join!(
        configmap::run(client.clone()),
        secret::run(client.clone()),
        namespace::run(client.clone()),
    )?;

    Ok(())
}
