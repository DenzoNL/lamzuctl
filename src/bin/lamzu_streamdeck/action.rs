//! What each action button shows and does.
//!
//! Everything here is pure (no I/O), so it's unit-testable.

use crate::device::{DeviceState, Status};
use crate::image::battery_bar_image;
use crate::protocol::{ActionMode, ActionSettings};

/// Everything Stream Deck needs to draw a button
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Render {
    pub title: String,
    /// Data URI, or empty to use the manifest default image
    pub image: String,
    /// 0 = default, 1 = active/highlighted
    pub state: u8,
}

/// A visible action instance on the Stream Deck
#[derive(Debug, Clone)]
pub struct ActionInstance {
    pub settings: ActionSettings,
    /// What we last sent to Stream Deck, so unchanged buttons aren't redrawn
    pub last_render: Option<Render>,
}

impl ActionInstance {
    pub fn new(settings: ActionSettings) -> Self {
        Self {
            settings,
            last_render: None,
        }
    }

    /// Short value text for the button, e.g. "P2", "1600", "81%+"
    fn value_text(&self, state: &DeviceState) -> String {
        match self.settings.mode {
            ActionMode::Profile => format!("P{}", state.profile),
            ActionMode::Dpi => lamzuctl::stage_at(&state.dpi_stages, state.dpi_stage)
                .map(|s| s.x.to_string())
                .unwrap_or_else(|| format!("DPI{}", state.dpi_stage)),
            ActionMode::Battery => {
                let charging = if state.battery.charging { "+" } else { "" };
                format!("{}%{}", state.battery.percentage, charging)
            }
        }
    }

    pub fn render(&self, state: &DeviceState) -> Render {
        let status_text = match state.status {
            Status::Connected => None,
            Status::MouseAsleep => Some("asleep"),
            Status::Disconnected => Some("no mouse"),
        };

        let show_bar = self.settings.show_battery && self.settings.mode != ActionMode::Battery;

        match (show_bar, status_text) {
            (true, None) => Render {
                title: String::new(),
                image: battery_bar_image(&self.value_text(state), Some(state.battery), ""),
                state: self.is_active(state).into(),
            },
            (true, Some(status)) => Render {
                title: String::new(),
                image: battery_bar_image("--", None, status),
                state: 0,
            },
            (false, None) => Render {
                title: self.value_text(state),
                image: String::new(),
                state: self.is_active(state).into(),
            },
            (false, Some(status)) => Render {
                title: format!("--\n{status}"),
                image: String::new(),
                state: 0,
            },
        }
    }

    /// Whether the button should be highlighted: the current profile is one
    /// of the selected ones (profile mode only; the Property Inspector never
    /// sets selected values for the other modes).
    fn is_active(&self, state: &DeviceState) -> bool {
        match self.settings.mode {
            ActionMode::Profile => self.settings.selected_values.contains(&state.profile),
            ActionMode::Dpi => self.settings.selected_values.contains(&state.dpi_stage),
            ActionMode::Battery => false,
        }
    }

    /// The value a key press should switch to, or `None` if there's nothing to cycle.
    ///
    /// - Profile mode cycles through the selected profiles (sorted, wrapping).
    /// - DPI mode cycles through every stage configured on the current profile.
    pub fn next_cycle_value(&self, state: &DeviceState) -> Option<u8> {
        match self.settings.mode {
            ActionMode::Profile => {
                let mut values = self.settings.selected_values.clone();
                values.sort_unstable();
                values.dedup();
                values
                    .iter()
                    .copied()
                    .find(|&v| v > state.profile)
                    .or_else(|| values.first().copied())
            }
            ActionMode::Dpi => {
                let count = u8::try_from(state.dpi_stages.len())
                    .ok()
                    .filter(|&n| n > 0)?;
                Some(if state.dpi_stage >= count {
                    1
                } else {
                    state.dpi_stage + 1
                })
            }
            ActionMode::Battery => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lamzuctl::DpiStage;

    fn instance(mode: ActionMode, selected: &[u8], show_battery: bool) -> ActionInstance {
        ActionInstance::new(ActionSettings {
            mode,
            selected_values: selected.to_vec(),
            device_selector: None,
            show_battery,
        })
    }

    fn connected(profile: u8, dpi_stage: u8) -> DeviceState {
        DeviceState {
            status: Status::Connected,
            profile,
            dpi_stage,
            dpi_stages: vec![DpiStage { x: 800, y: 800 }, DpiStage { x: 1600, y: 1600 }],
            ..DeviceState::default()
        }
    }

    #[test]
    fn profile_cycle_wraps_and_ignores_duplicates() {
        let a = instance(ActionMode::Profile, &[3, 1, 3], false);
        assert_eq!(a.next_cycle_value(&connected(1, 1)), Some(3));
        assert_eq!(a.next_cycle_value(&connected(3, 1)), Some(1));
        assert_eq!(a.next_cycle_value(&connected(2, 1)), Some(3));
        assert_eq!(
            instance(ActionMode::Profile, &[], false).next_cycle_value(&connected(1, 1)),
            None
        );
    }

    #[test]
    fn dpi_cycle_wraps_and_handles_zero() {
        let a = instance(ActionMode::Dpi, &[], false);
        assert_eq!(a.next_cycle_value(&connected(1, 1)), Some(2));
        assert_eq!(a.next_cycle_value(&connected(1, 2)), Some(1));
        assert_eq!(a.next_cycle_value(&connected(1, 0)), Some(1));
        assert_eq!(a.next_cycle_value(&DeviceState::default()), None);
    }

    /// Regression: a sleeping mouse reports DPI stage 0, which used to underflow and crash.
    #[test]
    fn renders_without_panicking_on_zeroed_state() {
        for mode in [ActionMode::Profile, ActionMode::Dpi, ActionMode::Battery] {
            for show_battery in [false, true] {
                for status in [Status::Connected, Status::MouseAsleep, Status::Disconnected] {
                    let state = DeviceState {
                        status,
                        ..DeviceState::default()
                    };
                    let _ = instance(mode, &[1], show_battery).render(&state);
                }
            }
        }
    }

    #[test]
    fn render_titles() {
        let dpi = instance(ActionMode::Dpi, &[], false);
        assert_eq!(dpi.render(&connected(1, 2)).title, "1600");

        let profile = instance(ActionMode::Profile, &[2], false);
        let r = profile.render(&connected(2, 1));
        assert_eq!((r.title.as_str(), r.state), ("P2", 1));

        let asleep = DeviceState {
            status: Status::MouseAsleep,
            ..connected(2, 1)
        };
        let r = profile.render(&asleep);
        assert_eq!((r.title.as_str(), r.state), ("--\nasleep", 0));
    }
}
