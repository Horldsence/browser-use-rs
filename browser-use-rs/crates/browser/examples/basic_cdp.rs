//! Basic CDP example - connecting and getting browser version

use browser::cdp::CDPClient;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Connect to Chrome
    let cdp_url = "ws://localhost:9222/devtools/browser";
    println!("Connecting to Chrome at: {}", cdp_url);

    let client = CDPClient::connect(cdp_url).await?;
    println!("Connected!");

    // Get browser version
    let version_result = client
        .send_request("Browser.getVersion", None, None)
        .await?;

    println!("Browser version: {}", version_result);

    // List targets
    let targets_result = client.send_request("Target.getTargets", None, None).await?;

    println!("Targets: {}", targets_result);

    // Subscribe to target events
    let client_clone = Arc::clone(&client);
    client.subscribe(
        "Target.targetCreated",
        Arc::new(move |event| {
            println!("Target created: {:?}", event);
        }),
    );

    // Keep alive for a bit to see events
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Clean shutdown
    client.close().await?;
    println!("Disconnected");

    Ok(())
}
