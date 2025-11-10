//! Session management example - creating tabs and navigating

use browser::session::{BrowserSession, SessionConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create session config
    let config = SessionConfig {
        id: "example-session".to_string(),
        cdp_url: "ws://localhost:9222".to_string(),
        headless: true,
        user_data_dir: None,
    };

    println!("Creating browser session: {}", config.id);
    let session = BrowserSession::new(config);

    // Subscribe to events before starting
    let mut event_rx = session.event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            println!("📢 Event: {:?}", event);
        }
    });

    // Start the session
    session.start().await?;
    println!("✅ Session started");

    // Create first tab
    let tab1 = session
        .new_tab(Some("https://www.rust-lang.org".to_string()))
        .await?;
    println!("📄 Created tab 1: {}", tab1);

    // Wait for navigation
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Create second tab
    let tab2 = session
        .new_tab(Some("https://github.com".to_string()))
        .await?;
    println!("📄 Created tab 2: {}", tab2);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Switch back to first tab
    session.switch_tab(tab1.clone()).await?;
    println!("🔄 Switched to tab 1");

    // Navigate current tab
    session.navigate("https://crates.io").await?;
    println!("🧭 Navigated to crates.io");

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Check current session
    if let Some(current) = session.current_session().await {
        let info = current.get_target_info().await?;
        println!("📍 Current page: {} - {}", info.title, info.url);
    }

    // Evaluate JavaScript
    if let Some(current) = session.current_session().await {
        let result = current.evaluate("document.title").await?;
        println!("🔍 Page title via JS: {}", result);
    }

    // Stop the session
    session.stop().await?;
    println!("🛑 Session stopped");

    Ok(())
}
