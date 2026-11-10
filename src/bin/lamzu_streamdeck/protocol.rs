//! Stream Deck WebSocket protocol types.
//!
//! Only the events and commands this plugin actually uses are modelled.
//! Anything else Stream Deck sends (e.g. `titleParametersDidChange`,
//! `keyUp`, `deviceDidConnect`) deserializes to [`StreamDeckEvent::Other`]
//! and is ignored, so new SDK events can never break event parsing.
//!
//! Reference: <https://docs.elgato.com/streamdeck/sdk/references/websocket/plugin/>

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Events received from Stream Deck
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum StreamDeckEvent {
    /// An action instance became visible
    WillAppear {
        context: String,
        payload: SettingsPayload,
    },
    /// An action instance is no longer visible
    WillDisappear { context: String },
    /// User pressed a key
    KeyDown {
        context: String,
        payload: SettingsPayload,
    },
    /// Settings changed via the Property Inspector
    DidReceiveSettings {
        context: String,
        payload: SettingsPayload,
    },
    /// Message from the Property Inspector
    SendToPlugin {
        action: String,
        context: String,
        payload: Value,
    },
    /// Computer woke from sleep
    SystemDidWakeUp,
    /// Any event this plugin doesn't handle
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct SettingsPayload {
    #[serde(default)]
    pub settings: Value,
}

/// Commands sent to Stream Deck
#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum StreamDeckCommand {
    SetTitle {
        context: String,
        payload: TitlePayload,
    },
    SetImage {
        context: String,
        payload: ImagePayload,
    },
    SetState {
        context: String,
        payload: StatePayload,
    },
    ShowAlert {
        context: String,
    },
    SendToPropertyInspector {
        action: String,
        context: String,
        payload: Value,
    },
}

#[derive(Debug, Serialize)]
pub struct TitlePayload {
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ImagePayload {
    /// Data URI, or an empty string to restore the manifest default image
    pub image: String,
}

#[derive(Debug, Serialize)]
pub struct StatePayload {
    pub state: u8,
}

/// Registration message sent right after connecting
#[derive(Debug, Serialize)]
pub struct RegistrationMessage<'a> {
    pub event: &'a str,
    pub uuid: &'a str,
}

/// Per-action settings, as stored by the Property Inspector
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActionSettings {
    /// Action mode: "profile", "dpi", or "battery"
    pub mode: ActionMode,
    /// Profile mode: profiles to cycle through (e.g. `[1, 2, 3]`)
    pub selected_values: Vec<u8>,
    /// Device selector for multi-device setups (index, PID, or name substring)
    pub device_selector: Option<String>,
    /// Show battery level as a background bar (profile/dpi modes)
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
    /// Parse settings, falling back to defaults for missing or malformed data
    pub fn from_value(value: &Value) -> Self {
        Self::deserialize(value).unwrap_or_default()
    }

    /// The device selector, ignoring empty strings
    pub fn selector(&self) -> Option<&str> {
        self.device_selector.as_deref().filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_events_are_ignored() {
        let raw = r#"{"action":"x","context":"c","device":"d","event":"titleParametersDidChange","payload":{}}"#;
        assert!(matches!(
            serde_json::from_str::<StreamDeckEvent>(raw).unwrap(),
            StreamDeckEvent::Other
        ));
    }

    #[test]
    fn will_appear_parses_with_extra_fields() {
        let raw = json!({
            "action": "io.github.denzonl.lamzuctl.action",
            "context": "abc",
            "device": "dev",
            "event": "willAppear",
            "payload": {
                "controller": "Keypad",
                "coordinates": {"column": 6, "row": 0},
                "isInMultiAction": false,
                "settings": {"mode": "profile", "selectedValues": [1, 2], "showBattery": true}
            }
        });
        let StreamDeckEvent::WillAppear { context, payload } = serde_json::from_value(raw).unwrap()
        else {
            panic!("wrong variant");
        };
        assert_eq!(context, "abc");
        let settings = ActionSettings::from_value(&payload.settings);
        assert_eq!(settings.mode, ActionMode::Profile);
        assert_eq!(settings.selected_values, vec![1, 2]);
        assert!(settings.show_battery);
    }

    #[test]
    fn settings_tolerate_garbage() {
        assert_eq!(
            ActionSettings::from_value(&json!({"mode": "nonsense"})),
            ActionSettings::default()
        );
        let s = ActionSettings::from_value(&json!({"mode": "dpi", "deviceSelector": ""}));
        assert_eq!(s.mode, ActionMode::Dpi);
        assert_eq!(s.selector(), None);
    }

    #[test]
    fn commands_serialize_in_sdk_shape() {
        let cmd = StreamDeckCommand::SetTitle {
            context: "c".into(),
            payload: TitlePayload { title: "P1".into() },
        };
        assert_eq!(
            serde_json::to_value(cmd).unwrap(),
            json!({"event": "setTitle", "context": "c", "payload": {"title": "P1"}})
        );
    }
}
