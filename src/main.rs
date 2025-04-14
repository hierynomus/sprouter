use tracing::info;

use shadower::controller::{configmap, secret, namespace};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting the shadower controller...");

    let client = kube::Client::try_default().await?;

    tokio::try_join!(
        configmap::run(client.clone()),
        secret::run(client.clone()),
        namespace::run(client.clone()),
    )?;

    info!("Shadower controller started successfully.");
    // This is a blocking call, it will run until the program is terminated
    tokio::signal::ctrl_c().await?;

    Ok(())
}
