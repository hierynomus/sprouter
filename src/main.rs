use tracing::info;

use sprouter::controller::{configmap, namespace, secret};
use sprouter::sprout::manager::SproutManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting the Sprouter controller...");

    let client = kube::Client::try_default().await?;

    // Initialize the SproutManager
    let sprout_manager = SproutManager::new(client.clone());
    sprout_manager.init().await?;
    info!("SproutManager initialized.");

    tokio::try_join!(
        configmap::run(client.clone(), &sprout_manager),
        secret::run(client.clone(), &sprout_manager),
        namespace::run(client.clone(), &sprout_manager),
    )?;

    Ok(())
}
