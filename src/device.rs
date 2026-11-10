//! Device discovery and selection

use anyhow::{Context, Result};
use hidapi::HidApi;

use crate::controller::DeviceController;
use crate::device_db::is_lamzu_vendor;

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
        if after_mi.len() >= 2
            && let Ok(num) = u8::from_str_radix(&after_mi[..2], 16)
        {
            return Some(num);
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
                interface_number: device
                    .interface_number()
                    .try_into()
                    .ok()
                    .or_else(|| extract_interface_number(&path)),
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
    // The dongle returns a success marker even when the mouse isn't connected,
    // but with profile 0. `connect_path` also takes the cross-process lock.
    DeviceController::new()
        .and_then(|mut c| {
            c.connect_path(&device.path)?;
            c.is_mouse_responding()
        })
        .unwrap_or(false)
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
                return idx
                    .checked_sub(1)
                    .and_then(|i| devices.get(i))
                    .ok_or_else(|| {
                        anyhow::anyhow!("Device index {} out of range (1-{})", idx, devices.len())
                    });
            }
            // Try PID (hex)
            let sel_clean = sel.trim_start_matches("0x").trim_start_matches("0X");
            if let Ok(pid) = u16::from_str_radix(sel_clean, 16)
                && let Some(d) = devices.iter().find(|d| d.product_id == pid)
            {
                return Ok(d);
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
            // With multiple devices (e.g. wired + dongle), prefer the one that
            // actually responds; otherwise fall back to the first.
            if devices.len() > 1
                && let Some(d) = devices.iter().find(|d| probe_device(d))
            {
                return Ok(d);
            }
            Ok(&devices[0])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(pid: u16, name: &str) -> DeviceInfo {
        DeviceInfo {
            vendor_id: 0x373E,
            product_id: pid,
            product_string: Some(name.to_string()),
            manufacturer_string: None,
            serial_number: None,
            path: String::new(),
            interface_number: Some(2),
            usage_page: 0,
            usage: 0,
        }
    }

    #[test]
    fn interface_number_from_path() {
        let path = r"\?\HID#VID_373E&PID_001E&MI_02#9&189fb621&0&0000#{4d1e55b2}";
        assert_eq!(extract_interface_number(path), Some(2));
        assert_eq!(
            extract_interface_number(r"\?\HID#VID_373E&PID_001E#x"),
            None
        );
    }

    #[test]
    fn select_by_index_name_and_pid() {
        let devices = [dev(0x001C, "Maya X Wired"), dev(0x001E, "Maya X 8K Dongle")];
        assert_eq!(
            select_device(&devices, Some("2")).unwrap().product_id,
            0x001E
        );
        assert!(select_device(&devices, Some("0")).is_err());
        assert!(select_device(&devices, Some("3")).is_err());
        assert_eq!(
            select_device(&devices, Some("dongle")).unwrap().product_id,
            0x001E
        );
        assert_eq!(
            select_device(&devices, Some("0x001c")).unwrap().product_id,
            0x001C
        );
        assert!(select_device(&devices, Some("nothing")).is_err());
        assert!(select_device(&[], None).is_err());
    }
}
