use tracing::info;

use shadower::{controller::{configmap, namespace, secret}, shadow};
use shadower::shadow::manager::ShadowManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting the shadower controller...");

    let client = kube::Client::try_default().await?;

    // Initialize the ShadowManager
    let shadow_manager = ShadowManager::new(client.clone());
    shadow_manager.init().await?;
    info!("ShadowManager initialized.");

    tokio::try_join!(
        configmap::run(client.clone(), &shadow_manager),
        secret::run(client.clone()),
        namespace::run(client.clone(), &shadow_manager),
    )?;

    info!("Shadower controller started successfully.");
    // This is a blocking call, it will run until the program is terminated
    tokio::signal::ctrl_c().await?;

    Ok(())
}
