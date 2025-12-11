//! Action Instance Management
//!
//! Manages the state and behavior of individual Stream Deck action instances.

use crate::device::{DeviceManager, DeviceState};
use crate::protocol::{ActionMode, ActionSettings};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Represents a single action instance on the Stream Deck
#[derive(Debug, Clone)]
pub struct ActionInstance {
    pub context: String,
    pub action: String,
    pub device: String,
    pub settings: ActionSettings,
}

impl ActionInstance {
    pub fn new(context: String, action: String, device: String, settings: ActionSettings) -> Self {
        Self {
            context,
            action,
            device,
            settings,
        }
    }

    /// Generate the display title for this action
    pub fn display_title(&self, state: &DeviceState) -> String {
        match self.settings.mode {
            ActionMode::Profile => {
                // Show current profile from device state
                format!("P{}", state.current_profile)
            }
            ActionMode::Dpi => {
                // Show current DPI value from device state
                if let Some(dpi_stage) = state.dpi_stages.get(state.current_dpi_stage as usize - 1) {
                    format!("{}", dpi_stage.x)
                } else {
                    format!("DPI{}", state.current_dpi_stage)
                }
            }
            ActionMode::Battery => {
                let icon = if state.battery.charging { "+" } else { "" };
                format!("{}%{}", state.battery.percentage, icon)
            }
        }
    }

    /// Determine if this action is currently "active" (highlighted)
    /// For cycling modes, active means the current value is in the selected list
    pub fn is_active(&self, state: &DeviceState) -> bool {
        match self.settings.mode {
            ActionMode::Profile => {
                self.settings.selected_values.contains(&state.current_profile)
            }
            ActionMode::Dpi => {
                self.settings.selected_values.contains(&state.current_dpi_stage)
            }
            ActionMode::Battery => false,
        }
    }

    /// Get the next value in the cycle based on current state
    /// For Profile mode: cycles through selected_values
    /// For DPI mode: cycles through all available stages (1 to dpi_stages.len())
    /// Returns None if cycling isn't applicable
    pub fn next_cycle_value(&self, state: &DeviceState) -> Option<u8> {
        match self.settings.mode {
            ActionMode::Profile => {
                if self.settings.selected_values.is_empty() {
                    return None;
                }

                let current = state.current_profile;

                // Find current position in the cycle
                let mut sorted_values = self.settings.selected_values.clone();
                sorted_values.sort();

                // Find the next value after current
                for &val in &sorted_values {
                    if val > current {
                        return Some(val);
                    }
                }

                // Wrap around to the first value
                sorted_values.first().copied()
            }
            ActionMode::Dpi => {
                // Cycle through all available DPI stages for the current profile
                let num_stages = state.dpi_stages.len() as u8;
                if num_stages == 0 {
                    return None;
                }

                let current = state.current_dpi_stage;
                let next = if current >= num_stages { 1 } else { current + 1 };
                Some(next)
            }
            ActionMode::Battery => None,
        }
    }
}

/// Manages all active action instances
pub struct ActionManager {
    instances: HashMap<String, ActionInstance>,
    battery_poll_handles: HashMap<String, mpsc::Sender<()>>,
}

impl ActionManager {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            battery_poll_handles: HashMap::new(),
        }
    }

    /// Register a new action instance
    pub fn add_instance(&mut self, instance: ActionInstance) {
        self.instances.insert(instance.context.clone(), instance);
    }

    /// Remove an action instance
    pub fn remove_instance(&mut self, context: &str) -> Option<ActionInstance> {
        // Stop any battery polling for this instance
        if let Some(tx) = self.battery_poll_handles.remove(context) {
            let _ = tx.try_send(());
        }
        self.instances.remove(context)
    }

    /// Get an action instance
    pub fn get_instance(&self, context: &str) -> Option<&ActionInstance> {
        self.instances.get(context)
    }

    /// Get a mutable reference to an action instance
    pub fn get_instance_mut(&mut self, context: &str) -> Option<&mut ActionInstance> {
        self.instances.get_mut(context)
    }

    /// Update settings for an instance
    pub fn update_settings(&mut self, context: &str, settings: ActionSettings) {
        if let Some(instance) = self.instances.get_mut(context) {
            instance.settings = settings;
        }
    }

    /// Get all instances
    pub fn instances(&self) -> impl Iterator<Item = &ActionInstance> {
        self.instances.values()
    }

    /// Get the number of active instances
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Get all instances with a specific mode
    pub fn instances_with_mode(&self, mode: ActionMode) -> Vec<&ActionInstance> {
        self.instances
            .values()
            .filter(|i| i.settings.mode == mode)
            .collect()
    }

    /// Check if any battery instances are active (for polling decisions)
    pub fn has_battery_instances(&self) -> bool {
        self.instances
            .values()
            .any(|i| i.settings.mode == ActionMode::Battery)
    }

    /// Register a battery poll stop handle
    pub fn register_battery_poll(&mut self, context: &str, stop_tx: mpsc::Sender<()>) {
        self.battery_poll_handles.insert(context.to_string(), stop_tx);
    }

    /// Check if battery polling is active for a context
    pub fn has_battery_poll(&self, context: &str) -> bool {
        self.battery_poll_handles.contains_key(context)
    }
}
