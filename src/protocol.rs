//! Lamzu HID protocol constants and helpers

use anyhow::Result;

// Device IDs
pub(crate) const DEVICE_ID_DONGLE: u8 = 0x00;
pub(crate) const DEVICE_ID_MOUSE: u8 = 0x02;

// Categories
pub(crate) const CATEGORY_GENERAL: u8 = 0x00;
pub(crate) const CATEGORY_CONFIG: u8 = 0x01;
pub(crate) const CATEGORY_LIGHTING: u8 = 0x02;

// Read opcodes (bit 7 set = read)
pub(crate) const OP_GET_POLLING_RATE: u8 = 0x80;
pub(crate) const OP_GET_DPI_STAGES: u8 = 0x81;
pub(crate) const OP_GET_ACTIVE_DPI: u8 = 0x82;
pub(crate) const OP_GET_BATTERY: u8 = 0x83;
pub(crate) const OP_GET_ANGLE_SNAP: u8 = 0x84; // Category 0x01
pub(crate) const OP_GET_PROFILE: u8 = 0x85;
pub(crate) const OP_GET_DEBOUNCE: u8 = 0x88; // Category 0x00 (no category)
pub(crate) const OP_GET_LOD: u8 = 0x88; // Category 0x01 - same opcode as debounce, different category
pub(crate) const OP_GET_MOTION_SYNC: u8 = 0x89; // Category 0x01
pub(crate) const OP_GET_RIPPLE_CONTROL: u8 = 0x8A; // Category 0x01
pub(crate) const OP_GET_HYPER_MODE: u8 = 0x8B; // Category 0x01 (high speed mode)
pub(crate) const OP_GET_TRACKING_MODE: u8 = 0x93; // Category 0x01 (competition mode)
pub(crate) const OP_GET_ANGLE_TUNE: u8 = 0x94; // Category 0x01
pub(crate) const OP_GET_DPI_COLORS: u8 = 0x81; // Same opcode, different category (lighting)
pub(crate) const OP_GET_FIRMWARE_VERSION: u8 = 0x81; // Category 0x00 (no category)

// Write opcodes (bit 7 clear = write)
pub(crate) const OP_SET_PROFILE: u8 = 0x05;
pub(crate) const OP_SET_ACTIVE_DPI: u8 = 0x02;

/// Build a 64-byte command packet for the Lamzu protocol
pub(crate) fn build_command(device_id: u8, payload_len: u8, category: u8, opcode: u8, params: &[u8]) -> [u8; 64] {
    let mut cmd = [0u8; 64];
    cmd[2] = device_id;
    cmd[3] = payload_len;
    cmd[4] = category;
    cmd[5] = opcode;
    if !params.is_empty() {
        cmd[6..6 + params.len()].copy_from_slice(params);
    }
    cmd
}

/// Validate response has success marker
pub(crate) fn validate_response(response: &[u8]) -> Result<()> {
    // Response format with Report ID: [0]=RptID, [1]=Marker, ...
    let marker = response.get(1).copied().unwrap_or(0);

    // Accept markers in range 0xA0-0xAF (160-175) as valid responses
    if !(0xA0..=0xAF).contains(&marker) {
        anyhow::bail!(
            "Device returned error response (expected 0xA0-0xAF, got 0x{:02X})",
            marker
        );
    }

    Ok(())
}

/// Get a byte from a response with bounds checking
pub(crate) fn get_response_byte(response: &[u8], index: usize) -> Result<u8> {
    response.get(index).copied().ok_or_else(|| {
        anyhow::anyhow!("Response too short: expected at least {} bytes, got {}", index + 1, response.len())
    })
}

/// Convert raw polling rate value to Hz
pub(crate) fn polling_rate_to_hz(raw: u8) -> u16 {
    match raw {
        0x01 | 0x10 => 1000,
        0x02 => 500,
        0x04 => 250,
        0x08 => 125,
        // Extended polling rates for 2K/4K/8K devices. 0x40 -> 4000 is
        // verified on a Maya X 8K; 0x20 -> 2000 follows the same pattern.
        0x20 => 2000,
        0x40 => 4000,
        0x80 => 8000,
        _ => raw as u16,
    }
}
