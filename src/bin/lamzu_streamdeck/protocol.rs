//! Stream Deck WebSocket Protocol Types
//!
//! Defines all events received from Stream Deck and commands sent to it.
//! Based on: https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Events received from Stream Deck
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum StreamDeckEvent {
    /// An action instance is now visible on the Stream Deck
    WillAppear {
        action: String,
        context: String,
        device: String,
        payload: AppearPayload,
    },
    /// An action instance is no longer visible
    WillDisappear {
        action: String,
        context: String,
        device: String,
        payload: AppearPayload,
    },
    /// User pressed a key
    KeyDown {
        action: String,
        context: String,
        device: String,
        payload: KeyPayload,
    },
    /// User released a key
    KeyUp {
        action: String,
        context: String,
        device: String,
        payload: KeyPayload,
    },
    /// Settings updated via Property Inspector or setSettings
    DidReceiveSettings {
        action: String,
        context: String,
        device: String,
        payload: SettingsPayload,
    },
    /// Global settings updated
    DidReceiveGlobalSettings {
        payload: GlobalSettingsPayload,
    },
    /// Property Inspector became visible
    PropertyInspectorDidAppear {
        action: String,
        context: String,
        device: String,
    },
    /// Property Inspector was closed
    PropertyInspectorDidDisappear {
        action: String,
        context: String,
        device: String,
    },
    /// Message from Property Inspector
    SendToPlugin {
        action: String,
        context: String,
        payload: Value,
    },
    /// System woke from sleep
    SystemDidWakeUp,
    /// A device was connected
    DeviceDidConnect {
        device: String,
        #[serde(rename = "deviceInfo")]
        device_info: DeviceInfo,
    },
    /// A device was disconnected
    DeviceDidDisconnect { device: String },
    /// Application launched (monitored app)
    ApplicationDidLaunch {
        payload: ApplicationPayload,
    },
    /// Application terminated (monitored app)
    ApplicationDidTerminate {
        payload: ApplicationPayload,
    },
    /// Dial rotated (Stream Deck +)
    DialRotate {
        action: String,
        context: String,
        device: String,
        payload: DialPayload,
    },
    /// Dial pressed (Stream Deck +)
    DialPress {
        action: String,
        context: String,
        device: String,
        payload: DialPayload,
    },
    /// Touchpad touched (Stream Deck +)
    TouchTap {
        action: String,
        context: String,
        device: String,
        payload: TouchPayload,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub state: u8,
    #[serde(default)]
    pub is_in_multi_action: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub state: u8,
    #[serde(default)]
    pub is_in_multi_action: bool,
    #[serde(default)]
    pub user_desired_state: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub is_in_multi_action: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettingsPayload {
    pub settings: Value,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Coordinates {
    pub column: u8,
    pub row: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: u8,
    pub size: DeviceSize,
}

#[derive(Debug, Deserialize)]
pub struct DeviceSize {
    pub columns: u8,
    pub rows: u8,
}

#[derive(Debug, Deserialize)]
pub struct ApplicationPayload {
    pub application: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub ticks: i32,
    #[serde(default)]
    pub pressed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchPayload {
    pub settings: Value,
    pub coordinates: Coordinates,
    #[serde(default)]
    pub tap_pos: Vec<i32>,
    #[serde(default)]
    pub hold: bool,
}

/// Commands to send to Stream Deck
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum StreamDeckCommand {
    /// Set the title of a button
    SetTitle {
        context: String,
        payload: SetTitlePayload,
    },
    /// Set the image of a button (base64 encoded)
    SetImage {
        context: String,
        payload: SetImagePayload,
    },
    /// Set the state of a multi-state action
    SetState {
        context: String,
        payload: SetStatePayload,
    },
    /// Show an alert triangle on a button
    ShowAlert { context: String },
    /// Show a checkmark on a button
    ShowOk { context: String },
    /// Save settings for an action instance
    SetSettings {
        context: String,
        payload: Value,
    },
    /// Get settings for an action instance
    GetSettings { context: String },
    /// Save global settings
    SetGlobalSettings {
        context: String,
        payload: Value,
    },
    /// Get global settings
    GetGlobalSettings { context: String },
    /// Open a URL in the default browser
    OpenUrl {
        payload: OpenUrlPayload,
    },
    /// Write to the plugin log
    LogMessage {
        payload: LogMessagePayload,
    },
    /// Send message to Property Inspector
    SendToPropertyInspector {
        action: String,
        context: String,
        payload: Value,
    },
    /// Switch to a profile
    SwitchToProfile {
        context: String,
        device: String,
        payload: SwitchProfilePayload,
    },
    /// Set feedback for encoder (Stream Deck +)
    SetFeedback {
        context: String,
        payload: Value,
    },
    /// Set feedback layout for encoder (Stream Deck +)
    SetFeedbackLayout {
        context: String,
        payload: SetFeedbackLayoutPayload,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetTitlePayload {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetImagePayload {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct SetStatePayload {
    pub state: u8,
}

#[derive(Debug, Serialize)]
pub struct OpenUrlPayload {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct LogMessagePayload {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SwitchProfilePayload {
    pub profile: String,
}

#[derive(Debug, Serialize)]
pub struct SetFeedbackLayoutPayload {
    pub layout: String,
}

/// Registration message sent when connecting
#[derive(Debug, Serialize)]
pub struct RegistrationMessage {
    pub event: String,
    pub uuid: String,
}

/// Information passed to plugin on startup
/// Using Value for fields we don't need to avoid parsing issues with new SDK versions
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub application: ApplicationInfo,
    #[serde(default)]
    pub plugin: Option<Value>,
    #[serde(default)]
    pub device_pixel_ratio: f32,
    #[serde(default)]
    pub colors: Option<Value>,
    #[serde(default)]
    pub devices: Vec<ConnectedDevice>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationInfo {
    #[serde(default)]
    pub font: String,
    #[serde(default)]
    pub language: String,
    pub platform: String,
    #[serde(default, rename = "platformVersion")]
    pub platform_version: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectedDevice {
    pub id: String,
    pub name: String,
    pub size: DeviceSize,
    #[serde(rename = "type")]
    pub device_type: u8,
}

/// Our plugin's action settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSettings {
    /// Action mode: "profile", "dpi", or "battery"
    #[serde(default)]
    pub mode: ActionMode,
    /// For profile mode: list of profiles to cycle through (e.g., [1, 2, 3])
    /// For dpi mode: list of DPI stages to cycle through
    #[serde(default)]
    pub selected_values: Vec<u8>,
    /// Device selector for multi-device setups
    #[serde(default)]
    pub device_selector: Option<String>,
    /// Show battery level as background bar (for profile/dpi modes)
    #[serde(default)]
    pub show_battery: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    Profile,
    Dpi,
    #[default]
    Battery,
}

impl ActionSettings {
    pub fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }
}
