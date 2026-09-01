//! Stream Deck Event Handling
//!
//! Processes events from Stream Deck and dispatches actions.

use crate::action::{ActionInstance, ActionManager};
use crate::device::{DeviceEvent, DeviceManager, DeviceState};
use crate::image::generate_battery_bar_image;

/// Log to file (reuse from main)
fn log(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
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
use crate::protocol::{
    ActionMode, ActionSettings, SetImagePayload, SetStatePayload, SetTitlePayload,
    StreamDeckCommand, StreamDeckEvent,
};
use anyhow::Result;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsReader = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// Send a command to Stream Deck
async fn send_command(writer: &Arc<RwLock<WsWriter>>, command: StreamDeckCommand) -> Result<()> {
    let json = serde_json::to_string(&command)?;
    let mut writer = writer.write().await;
    writer.send(Message::Text(json.into())).await?;
    Ok(())
}

/// Update the display of an action instance
async fn update_action_display(
    writer: &Arc<RwLock<WsWriter>>,
    instance: &ActionInstance,
    state: &DeviceState,
) -> Result<()> {
    let title = instance.display_title(state);
    let is_active = instance.is_active(state);

    // Check if we should show battery indicator (for profile/dpi modes with showBattery enabled)
    let show_battery_bar = instance.settings.show_battery
        && instance.settings.mode != ActionMode::Battery;

    if show_battery_bar {
        // Generate image with battery bar background
        let image = generate_battery_bar_image(
            &title,
            state.battery.percentage,
            state.battery.charging,
        );

        // Clear the built-in title (we render it in the image)
        send_command(
            writer,
            StreamDeckCommand::SetTitle {
                context: instance.context.clone(),
                payload: SetTitlePayload {
                    title: String::new(),
                    target: None,
                    state: None,
                },
            },
        )
        .await?;

        // Set the custom image
        send_command(
            writer,
            StreamDeckCommand::SetImage {
                context: instance.context.clone(),
                payload: SetImagePayload {
                    image,
                    target: None,
                    state: None,
                },
            },
        )
        .await?;
    } else {
        // Clear any custom image (reset to default)
        send_command(
            writer,
            StreamDeckCommand::SetImage {
                context: instance.context.clone(),
                payload: SetImagePayload {
                    image: String::new(),
                    target: None,
                    state: None,
                },
            },
        )
        .await?;

        // Set the title text
        send_command(
            writer,
            StreamDeckCommand::SetTitle {
                context: instance.context.clone(),
                payload: SetTitlePayload {
                    title,
                    target: None,
                    state: None,
                },
            },
        )
        .await?;
    }

    // Set state (0 = default, 1 = active/highlighted)
    send_command(
        writer,
        StreamDeckCommand::SetState {
            context: instance.context.clone(),
            payload: SetStatePayload {
                state: if is_active { 1 } else { 0 },
            },
        },
    )
    .await?;

    Ok(())
}

/// Handle willAppear event
async fn handle_will_appear(
    writer: &Arc<RwLock<WsWriter>>,
    action_manager: &mut ActionManager,
    device_manager: &Arc<DeviceManager>,
    action: String,
    context: String,
    device: String,
    settings: Value,
) -> Result<()> {
    log(&format!("handle_will_appear: context={}, settings={:?}", context, settings));

    let settings = ActionSettings::from_value(&settings);
    log(&format!("Parsed settings: mode={:?}, selected={:?}", settings.mode, settings.selected_values));

    // Create action instance
    let instance = ActionInstance::new(context.clone(), action, device, settings.clone());
    action_manager.add_instance(instance.clone());

    // Connect to device if not already
    log("Connecting to device...");
    if let Err(e) = device_manager.connect().await {
        log(&format!("Failed to connect to device: {}", e));
        // Still update display with default/error state
    } else {
        log("Device connected successfully");
    }

    // Update display
    let state = device_manager.get_state().await;
    log(&format!("Device state: connected={}, profile={}, dpi_stage={}, battery={}%",
        state.connected, state.current_profile, state.current_dpi_stage, state.battery.percentage));

    let title = instance.display_title(&state);
    log(&format!("Setting title to: {}", title));

    if let Err(e) = update_action_display(writer, &instance, &state).await {
        log(&format!("Failed to update display: {}", e));
    } else {
        log("Display updated successfully");
    }

    // Start battery polling if this is a battery action
    if settings.mode == ActionMode::Battery && !action_manager.has_battery_poll(&context) {
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let dm = Arc::clone(device_manager);
        let ctx = context.clone();
        let wr = Arc::clone(writer);
        let am_state = device_manager.state();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            let mut stop_rx = stop_rx;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Ok(battery) = dm.refresh_battery().await {
                            let state = am_state.read().await;
                            // Update display with new battery value
                            let icon = if battery.charging { "+" } else { "" };
                            let title = format!("{}%{}", battery.percentage, icon);

                            let _ = send_command(&wr, StreamDeckCommand::SetTitle {
                                context: ctx.clone(),
                                payload: SetTitlePayload {
                                    title,
                                    target: None,
                                    state: None,
                                },
                            }).await;
                        }
                    }
                    _ = stop_rx.recv() => {
                        break;
                    }
                }
            }
        });

        action_manager.register_battery_poll(&context, stop_tx);
    }

    Ok(())
}

/// Handle willDisappear event
async fn handle_will_disappear(action_manager: &mut ActionManager, context: String) -> Result<()> {
    action_manager.remove_instance(&context);
    Ok(())
}

/// Handle keyDown event
async fn handle_key_down(
    writer: &Arc<RwLock<WsWriter>>,
    action_manager: &ActionManager,
    device_manager: &Arc<DeviceManager>,
    context: String,
) -> Result<()> {
    let instance = match action_manager.get_instance(&context) {
        Some(i) => i.clone(),
        None => return Ok(()),
    };

    let state = device_manager.get_state().await;
    log(&format!("keyDown: mode={:?}, selected_values={:?}",
        instance.settings.mode, instance.settings.selected_values));

    match instance.settings.mode {
        ActionMode::Profile => {
            // Get next profile in the cycle
            let next = match instance.next_cycle_value(&state) {
                Some(v) => v,
                None => {
                    log("No profiles selected for cycling");
                    send_command(writer, StreamDeckCommand::ShowAlert { context }).await?;
                    return Ok(());
                }
            };
            log(&format!("Cycling profile: {} -> {}", state.current_profile, next));

            if let Err(e) = device_manager.set_profile(next).await {
                log(&format!("Failed to set profile: {}", e));
                send_command(writer, StreamDeckCommand::ShowAlert { context }).await?;
                return Ok(());
            }
        }
        ActionMode::Dpi => {
            // Get next DPI stage in the cycle
            let next = match instance.next_cycle_value(&state) {
                Some(v) => v,
                None => {
                    log("No DPI stages selected for cycling");
                    send_command(writer, StreamDeckCommand::ShowAlert { context }).await?;
                    return Ok(());
                }
            };
            log(&format!("Cycling DPI: {} -> {}", state.current_dpi_stage, next));

            if let Err(e) = device_manager.set_dpi_stage(state.current_profile, next).await {
                log(&format!("Failed to set DPI stage: {}", e));
                send_command(writer, StreamDeckCommand::ShowAlert { context }).await?;
                return Ok(());
            }
        }
        ActionMode::Battery => {
            // Refresh battery on click
            if let Err(e) = device_manager.refresh_battery().await {
                log(&format!("Failed to refresh battery: {}", e));
                send_command(writer, StreamDeckCommand::ShowAlert { context }).await?;
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Handle didReceiveSettings event
async fn handle_did_receive_settings(
    writer: &Arc<RwLock<WsWriter>>,
    action_manager: &mut ActionManager,
    device_manager: &Arc<DeviceManager>,
    context: String,
    settings: Value,
) -> Result<()> {
    let new_settings = ActionSettings::from_value(&settings);
    action_manager.update_settings(&context, new_settings);

    // Update display with new settings
    if let Some(instance) = action_manager.get_instance(&context) {
        let state = device_manager.get_state().await;
        update_action_display(writer, instance, &state).await?;
    }

    Ok(())
}

/// Handle device state updates
async fn handle_device_event(
    writer: &Arc<RwLock<WsWriter>>,
    action_manager: &ActionManager,
    event: DeviceEvent,
) -> Result<()> {
    match event {
        DeviceEvent::StateUpdated(state) => {
            // Update all action displays
            for instance in action_manager.instances() {
                if let Err(e) = update_action_display(writer, instance, &state).await {
                    eprintln!("Failed to update display for {}: {}", instance.context, e);
                }
            }
        }
        DeviceEvent::Connected { device_name } => {
            eprintln!("Connected to: {}", device_name);
        }
        DeviceEvent::Disconnected => {
            eprintln!("Device disconnected");
        }
        DeviceEvent::Error(msg) => {
            eprintln!("Device error: {}", msg);
        }
    }
    Ok(())
}

/// Main event loop
pub async fn run_event_loop(
    mut reader: WsReader,
    writer: Arc<RwLock<WsWriter>>,
    _device_rx: mpsc::Receiver<DeviceEvent>,
) -> Result<()> {
    log("run_event_loop starting");

    let (device_tx, mut device_event_rx) = mpsc::channel(32);
    let device_manager = Arc::new(DeviceManager::new(device_tx));
    let mut action_manager = ActionManager::new();

    // Try initial connection
    log("Attempting initial device connection...");
    if let Err(e) = device_manager.connect().await {
        log(&format!("Initial device connection failed (will retry): {}", e));
    } else {
        log("Initial device connection successful");
    }

    // State polling interval (15 seconds)
    let mut poll_interval = tokio::time::interval(Duration::from_secs(15));
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    log("Entering main event loop");
    loop {
        tokio::select! {
            // Periodic state polling
            _ = poll_interval.tick() => {
                // Only poll if we have visible actions
                if action_manager.instance_count() > 0 {
                    log("Polling device state...");
                    match device_manager.refresh_state().await {
                        Ok(state) => {
                            log(&format!("Poll: profile={}, dpi={}, battery={}%{}",
                                state.current_profile, state.current_dpi_stage,
                                state.battery.percentage,
                                if state.battery.charging { " (charging)" } else { "" }));
                            // Update all action displays
                            for instance in action_manager.instances() {
                                if let Err(e) = update_action_display(&writer, instance, &state).await {
                                    log(&format!("Failed to update display: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            log(&format!("Poll failed: {}", e));
                        }
                    }
                }
            }
            // Handle WebSocket messages from Stream Deck
            msg = reader.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        log(&format!("Received WebSocket message: {}", &text[..text.len().min(200)]));
                        match serde_json::from_str::<StreamDeckEvent>(&text) {
                            Ok(event) => {
                                log(&format!("Parsed event: {:?}", std::mem::discriminant(&event)));
                                if let Err(e) = handle_stream_deck_event(
                                    &writer,
                                    &mut action_manager,
                                    &device_manager,
                                    event,
                                ).await {
                                    log(&format!("Error handling event: {}", e));
                                }
                            }
                            Err(e) => {
                                log(&format!("Failed to parse event: {} - {}", e, text));
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log("WebSocket closed");
                        break;
                    }
                    Some(Err(e)) => {
                        log(&format!("WebSocket error: {}", e));
                        break;
                    }
                    None => {
                        log("WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }
            // Handle device events
            Some(event) = device_event_rx.recv() => {
                log(&format!("Device event received: {:?}", event));
                if let Err(e) = handle_device_event(&writer, &action_manager, event).await {
                    log(&format!("Error handling device event: {}", e));
                }
            }
        }
    }

    Ok(())
}

/// Dispatch Stream Deck events to handlers
async fn handle_stream_deck_event(
    writer: &Arc<RwLock<WsWriter>>,
    action_manager: &mut ActionManager,
    device_manager: &Arc<DeviceManager>,
    event: StreamDeckEvent,
) -> Result<()> {
    match event {
        StreamDeckEvent::WillAppear {
            action,
            context,
            device,
            payload,
        } => {
            handle_will_appear(
                writer,
                action_manager,
                device_manager,
                action,
                context,
                device,
                payload.settings,
            )
            .await?;
        }
        StreamDeckEvent::WillDisappear { context, .. } => {
            handle_will_disappear(action_manager, context).await?;
        }
        StreamDeckEvent::KeyDown { context, .. } => {
            handle_key_down(writer, action_manager, device_manager, context).await?;
        }
        StreamDeckEvent::DidReceiveSettings {
            context, payload, ..
        } => {
            handle_did_receive_settings(
                writer,
                action_manager,
                device_manager,
                context,
                payload.settings,
            )
            .await?;
        }
        StreamDeckEvent::SystemDidWakeUp => {
            // Reconnect to device after system wake
            if let Err(e) = device_manager.connect().await {
                eprintln!("Failed to reconnect after wake: {}", e);
            }
        }
        StreamDeckEvent::SendToPlugin {
            context, payload, ..
        } => {
            // Handle messages from Property Inspector
            handle_property_inspector_message(writer, action_manager, device_manager, context, payload).await?;
        }
        _ => {
            // Ignore other events
        }
    }
    Ok(())
}

/// Handle messages from Property Inspector
async fn handle_property_inspector_message(
    writer: &Arc<RwLock<WsWriter>>,
    _action_manager: &mut ActionManager,
    device_manager: &Arc<DeviceManager>,
    context: String,
    payload: Value,
) -> Result<()> {
    // Check for specific PI commands
    if let Some(cmd) = payload.get("command").and_then(|v| v.as_str()) {
        match cmd {
            "getDevices" => {
                // Send list of available devices to PI
                let devices = DeviceManager::list_devices().await.unwrap_or_default();
                let device_list: Vec<Value> = devices
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "path": d.path,
                            "name": d.product_string,
                            "vid": d.vendor_id,
                            "pid": d.product_id,
                        })
                    })
                    .collect();

                send_command(
                    writer,
                    StreamDeckCommand::SendToPropertyInspector {
                        action: "io.github.denzonl.lamzuctl.action".to_string(),
                        context,
                        payload: serde_json::json!({
                            "event": "deviceList",
                            "devices": device_list,
                        }),
                    },
                )
                .await?;
            }
            "getState" => {
                // Send current device state to PI
                let state = device_manager.get_state().await;
                send_command(
                    writer,
                    StreamDeckCommand::SendToPropertyInspector {
                        action: "io.github.denzonl.lamzuctl.action".to_string(),
                        context,
                        payload: serde_json::json!({
                            "event": "deviceState",
                            "connected": state.connected,
                            "deviceName": state.device_name,
                            "profile": state.current_profile,
                            "dpiStage": state.current_dpi_stage,
                            "battery": state.battery.percentage,
                            "charging": state.battery.charging,
                        }),
                    },
                )
                .await?;
            }
            _ => {}
        }
    }
    Ok(())
}
