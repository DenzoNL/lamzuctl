//! Lamzu Stream Deck plugin.
//!
//! Supports profile cycling, DPI stage cycling, and battery monitoring.
//! Launched by the Stream Deck app with `-port`, `-pluginUUID`,
//! `-registerEvent` and `-info` arguments.

mod action;
mod device;
mod events;
mod image;
mod log;
mod protocol;

use crate::log::log;
use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use protocol::RegistrationMessage;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Command line arguments passed by Stream Deck
struct Args {
    port: u16,
    plugin_uuid: String,
    register_event: String,
    info: serde_json::Value,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut port = None;
        let mut plugin_uuid = None;
        let mut register_event = None;
        let mut info = None;

        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .with_context(|| format!("Missing value for {flag}"))
            };
            match flag.as_str() {
                "-port" => port = Some(value()?.parse().context("Invalid -port")?),
                "-pluginUUID" => plugin_uuid = Some(value()?),
                "-registerEvent" => register_event = Some(value()?),
                "-info" => {
                    info = Some(serde_json::from_str(&value()?).context("Invalid -info JSON")?)
                }
                _ => {}
            }
        }

        Ok(Self {
            port: port.context("Missing -port argument")?,
            plugin_uuid: plugin_uuid.context("Missing -pluginUUID argument")?,
            register_event: register_event.context("Missing -registerEvent argument")?,
            info: info.unwrap_or_default(),
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    log::install_panic_hook();
    log(format!(
        "=== Plugin {} starting ===",
        env!("CARGO_PKG_VERSION")
    ));

    let result = run().await;
    match &result {
        Ok(()) => log("Plugin shutting down"),
        Err(e) => {
            log(format!("Fatal error: {e:#}"));
            eprintln!("Error: {e:#}");
            eprintln!("This plugin is meant to be launched by the Stream Deck app.");
        }
    }
    result
}

async fn run() -> Result<()> {
    let args = Args::parse()?;
    log(format!(
        "Stream Deck {} on {} {}",
        args.info["application"]["version"].as_str().unwrap_or("?"),
        args.info["application"]["platform"].as_str().unwrap_or("?"),
        args.info["application"]["platformVersion"]
            .as_str()
            .unwrap_or("?"),
    ));

    let (ws, _) = connect_async(format!("ws://127.0.0.1:{}", args.port))
        .await
        .context("Failed to connect to Stream Deck")?;
    let (mut writer, reader) = ws.split();

    let registration = serde_json::to_string(&RegistrationMessage {
        event: &args.register_event,
        uuid: &args.plugin_uuid,
    })?;
    writer
        .send(Message::Text(registration.into()))
        .await
        .context("Failed to register with Stream Deck")?;
    log("Registered with Stream Deck");

    events::run(reader, writer).await
}
