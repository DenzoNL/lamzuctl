//! Lamzu Stream Deck Plugin
//!
//! A Stream Deck plugin for controlling Lamzu gaming mice.
//! Supports profile switching, DPI stage changing, and battery monitoring.

mod action;
mod device;
mod events;
mod image;
mod protocol;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use protocol::{PluginInfo, RegistrationMessage};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Log to a file for debugging (Stream Deck plugins have no console)
fn log(msg: &str) {
    let log_path = std::env::temp_dir().join("lamzu-streamdeck.log");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}

/// Command line arguments passed by Stream Deck
struct Args {
    port: u16,
    plugin_uuid: String,
    register_event: String,
    info: PluginInfo,
}

impl Args {
    fn parse() -> Result<Self> {
        let args: Vec<String> = env::args().collect();

        let mut port = None;
        let mut plugin_uuid = None;
        let mut register_event = None;
        let mut info = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-port" => {
                    i += 1;
                    port = Some(args.get(i).context("Missing port value")?.parse()?);
                }
                "-pluginUUID" => {
                    i += 1;
                    plugin_uuid = Some(args.get(i).context("Missing pluginUUID value")?.clone());
                }
                "-registerEvent" => {
                    i += 1;
                    register_event =
                        Some(args.get(i).context("Missing registerEvent value")?.clone());
                }
                "-info" => {
                    i += 1;
                    let info_str = args.get(i).context("Missing info value")?;
                    info = Some(serde_json::from_str(info_str).context("Invalid info JSON")?);
                }
                _ => {}
            }
            i += 1;
        }

        Ok(Self {
            port: port.context("Missing -port argument")?,
            plugin_uuid: plugin_uuid.context("Missing -pluginUUID argument")?,
            register_event: register_event.context("Missing -registerEvent argument")?,
            info: info.context("Missing -info argument")?,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    log("=== Plugin starting ===");
    log(&format!("Args: {:?}", std::env::args().collect::<Vec<_>>()));

    // Parse command line arguments
    let args = match Args::parse() {
        Ok(args) => args,
        Err(e) => {
            log(&format!("Failed to parse arguments: {}", e));
            eprintln!("Failed to parse arguments: {}", e);
            eprintln!("This plugin should be launched by Stream Deck, not directly.");
            eprintln!("\nExpected arguments:");
            eprintln!("  -port <port>");
            eprintln!("  -pluginUUID <uuid>");
            eprintln!("  -registerEvent <event>");
            eprintln!("  -info <json>");
            return Err(e);
        }
    };

    log(&format!("Parsed args - port: {}, uuid: {}", args.port, args.plugin_uuid));
    log(&format!("Platform: {} {}", args.info.application.platform, args.info.application.platform_version));

    // Connect to Stream Deck WebSocket
    let url = format!("ws://127.0.0.1:{}", args.port);
    log(&format!("Connecting to WebSocket: {}", url));

    let (ws_stream, _) = connect_async(&url)
        .await
        .context("Failed to connect to Stream Deck")?;

    log("Connected to Stream Deck WebSocket");

    let (writer, reader) = ws_stream.split();
    let writer = Arc::new(RwLock::new(writer));

    // Send registration message
    let registration = RegistrationMessage {
        event: args.register_event,
        uuid: args.plugin_uuid,
    };
    let registration_json = serde_json::to_string(&registration)?;
    log(&format!("Sending registration: {}", registration_json));

    {
        let mut w = writer.write().await;
        w.send(Message::Text(registration_json))
            .await
            .context("Failed to send registration")?;
    }

    log("Registered with Stream Deck");

    // Test device connection
    log("Testing device connection...");
    match lamzuctl::list_devices() {
        Ok(devices) => {
            log(&format!("Found {} devices", devices.len()));
            for d in &devices {
                log(&format!("  - {:?} at {}", d.product_string, d.path));
            }
        }
        Err(e) => {
            log(&format!("Failed to list devices: {}", e));
        }
    }

    // Create device event channel (not used directly in main, but needed for event loop)
    let (_device_tx, device_rx) = tokio::sync::mpsc::channel(32);

    // Run event loop
    log("Starting event loop...");
    events::run_event_loop(reader, writer, device_rx).await?;

    log("Plugin shutting down");
    Ok(())
}
