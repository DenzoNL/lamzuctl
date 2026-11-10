//! The plugin's event loop.
//!
//! A single task owns all state: it reads Stream Deck events, polls the
//! device, and writes commands back. There are no background tasks or
//! internal channels, so there is nothing to deadlock or race.

use crate::action::ActionInstance;
use crate::device::{self, DeviceManager, Status};
use crate::log::log;
use crate::protocol::{
    ActionMode, ActionSettings, ImagePayload, StatePayload, StreamDeckCommand, StreamDeckEvent,
    TitlePayload,
};
use anyhow::Result;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type WsWriter = SplitSink<Ws, Message>;
pub type WsReader = SplitStream<Ws>;

/// How often to poll the device while any action is visible
const POLL_INTERVAL: Duration = Duration::from_secs(15);
/// Reuse a reading this fresh when a button appears, instead of re-polling
/// once per button when a page with many of them is shown
const APPEAR_MAX_AGE: Duration = Duration::from_secs(2);

struct Plugin {
    writer: WsWriter,
    device: DeviceManager,
    /// Visible actions keyed by context (ordered, so selector choice is stable)
    actions: BTreeMap<String, ActionInstance>,
    /// Last logged status, to log transitions rather than every poll
    logged_status: Option<Status>,
}

pub async fn run(mut reader: WsReader, writer: WsWriter) -> Result<()> {
    let mut plugin = Plugin {
        writer,
        device: DeviceManager::new(),
        actions: BTreeMap::new(),
        logged_status: None,
    };

    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = poll.tick() => {
                if !plugin.actions.is_empty() {
                    plugin.refresh().await;
                    plugin.render_all().await?;
                }
            }
            msg = reader.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<StreamDeckEvent>(&text) {
                        Ok(event) => plugin.handle(event).await?,
                        Err(e) => log(format!("Ignoring unparseable message ({e}): {text}")),
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    log("Stream Deck closed the connection");
                    return Ok(());
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
            }
        }
    }
}

impl Plugin {
    /// Send a command. An error here means the WebSocket is gone, which is fatal.
    async fn send(&mut self, command: StreamDeckCommand) -> Result<()> {
        let json = serde_json::to_string(&command)?;
        self.writer.send(Message::Text(json.into())).await?;
        Ok(())
    }

    /// The device selector used for polling: the first one set on any visible action
    fn poll_selector(&self) -> Option<String> {
        self.actions
            .values()
            .find_map(|a| a.settings.selector())
            .map(str::to_owned)
    }

    async fn refresh(&mut self) {
        let result = self.device.refresh(self.poll_selector()).await;
        self.log_status(result.err());
    }

    /// Log device status transitions (and the reason for failures)
    fn log_status(&mut self, error: Option<anyhow::Error>) {
        let state = self.device.state();
        if self.logged_status == Some(state.status) {
            return;
        }
        self.logged_status = Some(state.status);
        match (state.status, error) {
            (Status::Connected, _) => log(format!(
                "Mouse connected: {} (profile {}, DPI stage {}, battery {}%)",
                state.device_name, state.profile, state.dpi_stage, state.battery.percentage
            )),
            (status, Some(e)) => log(format!("Device status {status:?}: {e:#}")),
            (status, None) => log(format!("Device status {status:?}")),
        }
    }

    async fn render_all(&mut self) -> Result<()> {
        let contexts: Vec<String> = self.actions.keys().cloned().collect();
        for context in contexts {
            self.render(&context).await?;
        }
        Ok(())
    }

    /// Redraw one button, sending only what changed since the last draw
    async fn render(&mut self, context: &str) -> Result<()> {
        let Some(action) = self.actions.get(context) else {
            return Ok(());
        };
        let new = action.render(self.device.state());
        let old = action.last_render.clone();
        if old.as_ref() == Some(&new) {
            return Ok(());
        }

        let context = context.to_owned();
        if old.as_ref().is_none_or(|o| o.image != new.image) {
            self.send(StreamDeckCommand::SetImage {
                context: context.clone(),
                payload: ImagePayload {
                    image: new.image.clone(),
                },
            })
            .await?;
        }
        if old.as_ref().is_none_or(|o| o.title != new.title) {
            self.send(StreamDeckCommand::SetTitle {
                context: context.clone(),
                payload: TitlePayload {
                    title: new.title.clone(),
                },
            })
            .await?;
        }
        if old.as_ref().is_none_or(|o| o.state != new.state) {
            self.send(StreamDeckCommand::SetState {
                context: context.clone(),
                payload: StatePayload { state: new.state },
            })
            .await?;
        }

        if let Some(action) = self.actions.get_mut(&context) {
            action.last_render = Some(new);
        }
        Ok(())
    }

    async fn handle(&mut self, event: StreamDeckEvent) -> Result<()> {
        match event {
            StreamDeckEvent::WillAppear { context, payload } => {
                let settings = ActionSettings::from_value(&payload.settings);
                self.actions
                    .insert(context.clone(), ActionInstance::new(settings));
                if self.device.is_stale(APPEAR_MAX_AGE) {
                    self.refresh().await;
                }
                self.render(&context).await?;
            }
            StreamDeckEvent::WillDisappear { context } => {
                self.actions.remove(&context);
            }
            StreamDeckEvent::DidReceiveSettings { context, payload } => {
                let settings = ActionSettings::from_value(&payload.settings);
                if let Some(action) = self.actions.get_mut(&context) {
                    action.settings = settings;
                }
                self.render(&context).await?;
            }
            StreamDeckEvent::KeyDown { context, payload } => {
                self.key_down(context, ActionSettings::from_value(&payload.settings))
                    .await?;
            }
            StreamDeckEvent::SendToPlugin {
                action,
                context,
                payload,
            } => {
                self.property_inspector_message(action, context, payload)
                    .await?;
            }
            StreamDeckEvent::SystemDidWakeUp => {
                log("System woke up, refreshing");
                self.refresh().await;
                self.render_all().await?;
            }
            StreamDeckEvent::Other => {}
        }
        Ok(())
    }

    async fn key_down(&mut self, context: String, settings: ActionSettings) -> Result<()> {
        // Prefer our tracked settings; fall back to the event's (e.g. inside a multi-action)
        let action = self
            .actions
            .get(&context)
            .cloned()
            .unwrap_or_else(|| ActionInstance::new(settings));
        let selector = action.settings.selector().map(str::to_owned);

        // Act on fresh state, so we don't cycle from a stale value after the
        // profile was changed on the mouse itself.
        let _ = self.device.refresh(selector.clone()).await;
        let state = self.device.state().clone();

        let result = if state.status != Status::Connected {
            Err(anyhow::anyhow!("mouse not available ({:?})", state.status))
        } else {
            match (action.settings.mode, action.next_cycle_value(&state)) {
                (ActionMode::Profile, Some(next)) => {
                    log(format!("Profile {} -> {next}", state.profile));
                    self.device.set_profile(selector, next).await
                }
                (ActionMode::Dpi, Some(next)) => {
                    log(format!("DPI stage {} -> {next}", state.dpi_stage));
                    self.device
                        .set_dpi_stage(selector, state.profile, next)
                        .await
                }
                (ActionMode::Battery, _) => Ok(()), // the refresh above was the point
                (mode, None) => Err(anyhow::anyhow!("nothing to cycle in {mode:?} mode")),
            }
        };

        if let Err(e) = &result {
            log(format!("Key press failed: {e:#}"));
            self.send(StreamDeckCommand::ShowAlert { context }).await?;
        }
        self.log_status(result.err());
        self.render_all().await
    }

    async fn property_inspector_message(
        &mut self,
        action: String,
        context: String,
        payload: Value,
    ) -> Result<()> {
        let reply = match payload.get("command").and_then(Value::as_str) {
            Some("getDevices") => {
                let devices = device::list_devices().await.unwrap_or_default();
                let list: Vec<Value> = devices
                    .iter()
                    .map(|d| {
                        json!({
                            "name": d.product_string,
                            "vid": d.vendor_id,
                            "pid": d.product_id,
                        })
                    })
                    .collect();
                json!({ "event": "deviceList", "devices": list })
            }
            Some("getState") => {
                self.refresh().await;
                self.render_all().await?;
                let s = self.device.state();
                json!({
                    "event": "deviceState",
                    "status": match s.status {
                        Status::Connected => "connected",
                        Status::MouseAsleep => "asleep",
                        Status::Disconnected => "disconnected",
                    },
                    "deviceName": s.device_name,
                    "profile": s.profile,
                    "dpiStage": s.dpi_stage,
                    "battery": s.battery.percentage,
                    "charging": s.battery.charging,
                })
            }
            _ => return Ok(()),
        };

        self.send(StreamDeckCommand::SendToPropertyInspector {
            action,
            context,
            payload: reply,
        })
        .await
    }
}
