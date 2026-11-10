//! lamzuctl - Control utility for Lamzu gaming mice
//!
//! This library provides functionality for communicating with Lamzu gaming mice
//! over HID, allowing you to read and modify device settings.

mod controller;
mod device;
mod device_db;
mod protocol;
mod types;

// Re-export public API
pub use controller::DeviceController;
pub use device::{DeviceInfo, list_devices, select_device};
pub use device_db::*;
pub use types::*;
