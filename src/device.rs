//! Device discovery and selection

use anyhow::{Context, Result};
use hidapi::HidApi;
use std::ffi::CString;

use crate::device_db::is_lamzu_vendor;
use crate::protocol::{CATEGORY_GENERAL, DEVICE_ID_MOUSE, OP_GET_PROFILE};

/// Information about a detected device
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Vendor ID
    pub vendor_id: u16,
    /// Product ID
    pub product_id: u16,
    /// Product string
    pub product_string: Option<String>,
    /// Manufacturer string
    pub manufacturer_string: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// Device path (the control interface path for HID communication)
    pub path: String,
    /// Interface number extracted from path
    pub interface_number: Option<u8>,
    /// Usage page (if available)
    pub usage_page: u16,
    /// Usage (if available)
    pub usage: u16,
}

/// Extract interface number from Windows HID path
/// e.g., "\\?\HID#VID_373E&PID_001E&MI_02#..." -> Some(2)
fn extract_interface_number(path: &str) -> Option<u8> {
    // Look for MI_XX pattern in the path
    let path_upper = path.to_uppercase();
    if let Some(mi_pos) = path_upper.find("MI_") {
        let after_mi = &path_upper[mi_pos + 3..];
        // Extract the two hex digits after MI_
        if after_mi.len() >= 2 {
            if let Ok(num) = u8::from_str_radix(&after_mi[..2], 16) {
                return Some(num);
            }
        }
    }
    None
}

/// Enumerate all connected Lamzu devices (raw, unfiltered)
pub(crate) fn list_devices_raw() -> Result<Vec<DeviceInfo>> {
    let api = HidApi::new().context("Failed to initialize HID API")?;

    let mut devices = Vec::new();

    for device in api.device_list() {
        // Filter for Lamzu devices (check all known vendor IDs)
        if is_lamzu_vendor(device.vendor_id()) {
            let path = device.path().to_string_lossy().to_string();
            devices.push(DeviceInfo {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                product_string: device.product_string().map(|s| s.to_string()),
                manufacturer_string: device.manufacturer_string().map(|s| s.to_string()),
                serial_number: device.serial_number().map(|s| s.to_string()),
                interface_number: extract_interface_number(&path),
                usage_page: device.usage_page(),
                usage: device.usage(),
                path,
            });
        }
    }

    Ok(devices)
}

/// Enumerate all connected Lamzu devices, deduplicated to show only unique physical devices
///
/// Each physical device (identified by serial number) may expose multiple HID interfaces.
/// This function returns only the control interface (MI_02 / interface 2) which is used
/// for configuration commands.
pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let all_devices = list_devices_raw()?;

    // Group devices by (vendor_id, product_id, serial_number) to identify unique physical devices
    let mut seen: std::collections::HashSet<(u16, u16, String)> = std::collections::HashSet::new();
    let mut unique_devices = Vec::new();

    // First pass: collect the control interface (MI_02) for each unique device
    for device in &all_devices {
        let serial = device.serial_number.clone().unwrap_or_default();
        let key = (device.vendor_id, device.product_id, serial.clone());

        // Prefer interface 2 (MI_02) as the control interface
        if device.interface_number == Some(2) && !seen.contains(&key) {
            seen.insert(key);
            unique_devices.push(device.clone());
        }
    }

    // Second pass: for any devices without MI_02, fall back to any available interface
    for device in &all_devices {
        let serial = device.serial_number.clone().unwrap_or_default();
        let key = (device.vendor_id, device.product_id, serial);

        if !seen.contains(&key) {
            seen.insert(key);
            unique_devices.push(device.clone());
        }
    }

    Ok(unique_devices)
}

/// Probe a device to check if it responds with valid data
///
/// This is used when multiple devices are found (e.g., both wired and dongle
/// connections) to auto-select the one that's actually active. A device that's
/// connected but not in use (e.g., dongle when mouse is in wired mode) will
/// return all zeros, failing this check.
fn probe_device(device: &DeviceInfo) -> bool {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(_) => return false,
    };

    let c_path = match CString::new(device.path.as_str()) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let hid_device = match api.open_path(&c_path) {
        Ok(d) => d,
        Err(_) => return false,
    };

    // Build get_profile command (same as build_command but with report ID prefix)
    // Command format: [0]=ReportID, [1..65]=command data
    // Command data: [2]=DeviceID, [3]=PayloadLen, [4]=Category, [5]=Opcode
    let mut send_buf = [0u8; 65];
    send_buf[0] = 0x00; // Report ID
    // Offsets +1 because of report ID prefix
    send_buf[1 + 2] = DEVICE_ID_MOUSE; // [3] in send_buf = device_id
    send_buf[1 + 3] = 0x01;            // [4] = payload_len
    send_buf[1 + 4] = CATEGORY_GENERAL; // [5] = category
    send_buf[1 + 5] = OP_GET_PROFILE;  // [6] = opcode

    if hid_device.send_feature_report(&send_buf).is_err() {
        return false;
    }

    std::thread::sleep(std::time::Duration::from_millis(10));

    let mut recv_buf = [0u8; 65];
    recv_buf[0] = 0x00;

    let len = match hid_device.get_feature_report(&mut recv_buf) {
        Ok(l) => l,
        Err(_) => return false,
    };

    if len < 8 {
        return false;
    }

    // Response: [0]=RptID, [1]=Marker (0xA0-0xAF = success), [7]=Profile
    // The dongle returns success marker (0xA0) even when mouse isn't connected,
    // but the profile will be 0. A valid profile is 1-5.
    let marker = recv_buf[1];
    let profile = recv_buf[7];
    (0xA0..=0xAF).contains(&marker) && profile > 0
}

/// Select a device from the list based on a selector string
///
/// Selector can be:
/// - Index (1-based, e.g., "1", "2")
/// - PID in hex (e.g., "001c", "0x001E")
/// - Name substring (case-insensitive, e.g., "wired", "dongle")
///
/// If no selector is provided and multiple devices exist, probes each device
/// to find one that responds with valid data.
pub fn select_device<'a>(
    devices: &'a [DeviceInfo],
    selector: Option<&str>,
) -> Result<&'a DeviceInfo> {
    if devices.is_empty() {
        anyhow::bail!("No Lamzu devices found");
    }

    match selector {
        Some(sel) => {
            // Try index (1-based)
            if let Ok(idx) = sel.parse::<usize>() {
                return devices.get(idx - 1).ok_or_else(|| {
                    anyhow::anyhow!("Device index {} out of range (1-{})", idx, devices.len())
                });
            }
            // Try PID (hex)
            let sel_clean = sel.trim_start_matches("0x").trim_start_matches("0X");
            if let Ok(pid) = u16::from_str_radix(sel_clean, 16) {
                if let Some(d) = devices.iter().find(|d| d.product_id == pid) {
                    return Ok(d);
                }
            }
            // Try name substring (case-insensitive)
            let sel_lower = sel.to_lowercase();
            if let Some(d) = devices.iter().find(|d| {
                d.product_string
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&sel_lower))
                    .unwrap_or(false)
            }) {
                return Ok(d);
            }
            anyhow::bail!("No device matching '{}' found", sel);
        }
        None => {
            if devices.len() > 1 {
                // Multiple devices - probe to find one that responds
                let mut selected_idx = 0;
                for (i, device) in devices.iter().enumerate() {
                    if probe_device(device) {
                        selected_idx = i;
                        break;
                    }
                }
                return Ok(&devices[selected_idx]);
            }
            Ok(&devices[0])
        }
    }
}
