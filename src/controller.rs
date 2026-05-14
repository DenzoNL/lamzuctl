//! Device controller for communicating with Lamzu mice

use anyhow::{Context, Result};
use hidapi::HidApi;
use std::ffi::CString;

use crate::device::list_devices;
use crate::protocol::*;
use crate::types::*;

/// Main device controller
pub struct DeviceController {
    api: HidApi,
    device: Option<hidapi::HidDevice>,
}

impl DeviceController {
    /// Create a new device controller
    pub fn new() -> Result<Self> {
        let api = HidApi::new().context("Failed to initialize HID API")?;
        Ok(Self {
            api,
            device: None,
        })
    }

    /// Connect to the first available Lamzu device
    ///
    /// Tries to connect to any Lamzu device by using the deduplicated device list.
    /// This ensures we connect to the control interface (MI_02).
    pub fn connect(&mut self) -> Result<()> {
        let devices = list_devices()?;

        if devices.is_empty() {
            anyhow::bail!("No Lamzu devices found. Is your mouse connected?");
        }

        // Connect to the first available device
        self.connect_path(&devices[0].path)
    }

    /// Connect to a specific device by path
    pub fn connect_path(&mut self, path: &str) -> Result<()> {
        let c_path = CString::new(path)
            .context("Invalid path: contains null byte")?;

        let device = self.api
            .open_path(&c_path)
            .context("Failed to open device at specified path")?;

        self.device = Some(device);
        Ok(())
    }

    /// Send a command and get the response using feature reports
    fn send_and_receive(&self, command: &[u8; 64]) -> Result<Vec<u8>> {
        let device = self.device.as_ref()
            .context("Device not connected")?;

        // Send the command via Set Feature Report
        // First byte must be Report ID (0x00)
        let mut send_buf = [0u8; 65];
        send_buf[0] = 0x00; // Report ID
        send_buf[1..65].copy_from_slice(command);

        device.send_feature_report(&send_buf)
            .context("Failed to send feature report")?;

        // Give device time to process
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Get the response via Get Feature Report
        let mut recv_buf = [0u8; 65];
        recv_buf[0] = 0x00; // Report ID

        let len = device.get_feature_report(&mut recv_buf)
            .context("Failed to get feature report")?;

        Ok(recv_buf[..len].to_vec())
    }

    // ========================================================================
    // High-level read commands
    // ========================================================================

    /// Get the current profile ID (1-based, typically 1-5)
    pub fn get_profile(&self) -> Result<u8> {
        let cmd = build_command(DEVICE_ID_MOUSE, 0x01, CATEGORY_GENERAL, OP_GET_PROFILE, &[]);
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Profile
        // Device returns 1-based profile ID
        get_response_byte(&response, 7)
    }

    /// Get battery status (percentage and charging state)
    pub fn get_battery(&self) -> Result<BatteryStatus> {
        let cmd = build_command(DEVICE_ID_MOUSE, 0x02, CATEGORY_GENERAL, OP_GET_BATTERY, &[]);
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Charging, [8]=Percentage
        Ok(BatteryStatus {
            percentage: get_response_byte(&response, 8)?,
            charging: get_response_byte(&response, 7)? != 0,
        })
    }

    /// Get the polling rate in Hz for the given profile
    pub fn get_polling_rate(&self, profile_id: u8) -> Result<u16> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_POLLING_RATE,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Profile, [8]=Rate
        let raw = get_response_byte(&response, 8)?;
        Ok(polling_rate_to_hz(raw))
    }

    /// Get the active DPI stage index (1-based)
    pub fn get_active_dpi_stage(&self, profile_id: u8) -> Result<u8> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_ACTIVE_DPI,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Profile, [8]=Stage
        get_response_byte(&response, 8)
    }

    /// Get all DPI stages for the given profile
    ///
    /// `stage_count` is the number of stages to request (typically 6)
    pub fn get_dpi_stages(&self, profile_id: u8, stage_count: u8) -> Result<Vec<DpiStage>> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x0A, // payload length for DPI query
            CATEGORY_CONFIG,
            OP_GET_DPI_STAGES,
            &[profile_id, stage_count],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Profile, [8]=NumStages
        // DPI data starts at [9]: X_hi, X_lo, Y_hi, Y_lo for each stage
        let num_stages = get_response_byte(&response, 8)? as usize;
        let mut stages = Vec::with_capacity(num_stages);
        let data_start = 9;

        for i in 0..num_stages {
            let offset = data_start + i * 4;
            // Ensure all 4 bytes are available for this stage
            if offset + 3 >= response.len() {
                break;
            }
            let x = u16::from_be_bytes([response[offset], response[offset + 1]]);
            let y = u16::from_be_bytes([response[offset + 2], response[offset + 3]]);
            stages.push(DpiStage { x, y });
        }

        Ok(stages)
    }

    /// Get RGB colors for each DPI stage
    pub fn get_dpi_colors(&self, profile_id: u8) -> Result<Vec<RgbColor>> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x13, // payload length for color query
            CATEGORY_LIGHTING,
            OP_GET_DPI_COLORS,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Response contains RGB triplets after header
        // Header: [0]=RptID, [1]=Marker, [2]=?, [3]=DevID, [4]=Len, [5]=Cat, [6]=Opcode, [7]=Profile
        // Colors start at [8]: R, G, B for each stage
        let mut colors = Vec::new();
        let data_start = 8;

        for i in 0..6 {
            let offset = data_start + i * 3;
            if offset + 2 < response.len() {
                colors.push(RgbColor {
                    r: response[offset],
                    g: response[offset + 1],
                    b: response[offset + 2],
                });
            }
        }

        Ok(colors)
    }

    /// Get detailed information for a specific profile
    pub fn get_profile_info(&self, profile_id: u8) -> Result<ProfileInfo> {
        let polling_rate = self.get_polling_rate(profile_id)?;
        let active_dpi_stage = self.get_active_dpi_stage(profile_id)?;
        let dpi_stages = self.get_dpi_stages(profile_id, DEFAULT_DPI_STAGE_COUNT)?;

        Ok(ProfileInfo {
            id: profile_id,
            polling_rate,
            active_dpi_stage,
            dpi_stages,
        })
    }

    /// Get information for all profiles
    pub fn get_all_profiles(&self) -> Result<Vec<ProfileInfo>> {
        let mut profiles = Vec::with_capacity(DEFAULT_PROFILE_COUNT as usize);
        for profile_id in 1..=DEFAULT_PROFILE_COUNT {
            profiles.push(self.get_profile_info(profile_id)?);
        }
        Ok(profiles)
    }

    /// Get information for all configured profiles (filters out unconfigured ones)
    pub fn get_configured_profiles(&self) -> Result<Vec<ProfileInfo>> {
        let all = self.get_all_profiles()?;
        Ok(all.into_iter().filter(|p| p.is_configured()).collect())
    }

    /// Get the mouse firmware version
    pub fn get_firmware_version(&self) -> Result<FirmwareVersion> {
        self.get_firmware_version_for_device(DEVICE_ID_MOUSE)
    }

    /// Get the dongle/receiver firmware version
    pub fn get_dongle_firmware_version(&self) -> Result<FirmwareVersion> {
        self.get_firmware_version_for_device(DEVICE_ID_DONGLE)
    }

    /// Get firmware version for a specific device ID
    fn get_firmware_version_for_device(&self, device_id: u8) -> Result<FirmwareVersion> {
        // Based on Lamzu source: [2]=device_id, [3]=16, [5]=129 (0x81)
        // No category byte used
        let mut cmd = [0u8; 64];
        cmd[2] = device_id;
        cmd[3] = 0x10; // payload length = 16
        cmd[5] = OP_GET_FIRMWARE_VERSION;

        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Protocol detection from Lamzu source:
        // - response[6] == 0x81 -> new protocol (data at byte 7+)
        // - response[5] == 0x81 -> old protocol (data at byte 6+)
        // We only support new protocol, so response should have opcode echo at [6]
        let (major, minor, patch, build) = if get_response_byte(&response, 6)? == OP_GET_FIRMWARE_VERSION {
            // New protocol: [7]=major, [8]=minor, [9]=patch, [10]=build
            (
                get_response_byte(&response, 7)?,
                get_response_byte(&response, 8)?,
                get_response_byte(&response, 9)?,
                get_response_byte(&response, 10)?,
            )
        } else if get_response_byte(&response, 5)? == OP_GET_FIRMWARE_VERSION {
            // Old protocol detected - still parse it but warn
            (
                get_response_byte(&response, 6)?,
                get_response_byte(&response, 7)?,
                get_response_byte(&response, 8)?,
                get_response_byte(&response, 9)?,
            )
        } else {
            // Unknown format, try new protocol positions
            (
                get_response_byte(&response, 7)?,
                get_response_byte(&response, 8)?,
                get_response_byte(&response, 9)?,
                get_response_byte(&response, 10)?,
            )
        };

        Ok(FirmwareVersion {
            major,
            minor,
            patch,
            build,
        })
    }

    /// Get debounce time in milliseconds for the given profile
    pub fn get_debounce_time(&self, profile_id: u8) -> Result<u8> {
        // Based on Lamzu source: [2]=2, [3]=2, [5]=136 (0x88), [6]=profile_id
        // No category byte (differs from LOD which uses category 0x01)
        let mut cmd = [0u8; 64];
        cmd[2] = DEVICE_ID_MOUSE;
        cmd[3] = 0x02;
        cmd[5] = OP_GET_DEBOUNCE;
        cmd[6] = profile_id;

        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        get_response_byte(&response, 8)
    }

    /// Get lift-off distance (LOD) for the given profile
    ///
    /// Returns the LOD in millimeters. Common values are 1mm or 2mm.
    pub fn get_lod(&self, profile_id: u8) -> Result<f32> {
        // Based on Lamzu source: [2]=2, [3]=2, [4]=1 (category), [5]=136 (0x88), [6]=profile_id
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_LOD,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Decode LOD value - if bit 7 is set, value is in 0.1mm units
        let raw = get_response_byte(&response, 8)?;
        let lod = if raw & 0x80 != 0 {
            // Value is (raw & 0x7F) * 0.1 mm
            ((raw & 0x7F) as f32) * 0.1
        } else {
            raw as f32
        };
        Ok(lod)
    }

    /// Get motion sync setting for the given profile
    ///
    /// Motion sync synchronizes sensor polling with USB polling rate.
    pub fn get_motion_sync(&self, profile_id: u8) -> Result<bool> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_MOTION_SYNC,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(get_response_byte(&response, 8)? == 1)
    }

    /// Get angle snap setting for the given profile
    ///
    /// Angle snap (also called line correction) straightens diagonal mouse movements.
    pub fn get_angle_snap(&self, profile_id: u8) -> Result<bool> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_ANGLE_SNAP,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(get_response_byte(&response, 8)? == 1)
    }

    /// Get angle tuning value for the given profile
    ///
    /// Returns the angle adjustment value (-128 to 127 degrees).
    pub fn get_angle_tune(&self, profile_id: u8) -> Result<i8> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_ANGLE_TUNE,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        // Value is stored as unsigned but represents signed
        Ok(get_response_byte(&response, 8)? as i8)
    }

    /// Get 20K FPS extreme mode for the given profile
    ///
    /// When enabled, uses 20K scanning rate for extreme performance.
    /// Higher power consumption than Competition Mode.
    /// Requires Competition Mode to be enabled first.
    pub fn get_extreme_20k_fps(&self, profile_id: u8) -> Result<bool> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_TRACKING_MODE,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(get_response_byte(&response, 8)? == 1)
    }

    /// Get ripple control setting for the given profile
    ///
    /// Ripple control reduces cursor jitter at low movement speeds.
    pub fn get_ripple_control(&self, profile_id: u8) -> Result<bool> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_RIPPLE_CONTROL,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(get_response_byte(&response, 8)? == 1)
    }

    /// Get performance mode for the given profile
    ///
    /// Returns High-Speed mode or Competition mode.
    /// High-Speed: standard performance, lower power, incompatible with 2K/4K/8K polling
    /// Competition: maximum performance, higher power consumption
    pub fn get_performance_mode(&self, profile_id: u8) -> Result<PerformanceMode> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_GET_HYPER_MODE,
            &[profile_id],
        );
        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(if get_response_byte(&response, 8)? == 1 {
            PerformanceMode::Competition
        } else {
            PerformanceMode::HighSpeed
        })
    }

    /// Get all sensor settings for the given profile
    pub fn get_sensor_settings(&self, profile_id: u8) -> Result<SensorSettings> {
        Ok(SensorSettings {
            motion_sync: self.get_motion_sync(profile_id)?,
            angle_snap: self.get_angle_snap(profile_id)?,
            angle_tune: self.get_angle_tune(profile_id)?,
            ripple_control: self.get_ripple_control(profile_id)?,
            lod_mm: self.get_lod(profile_id)?,
            extreme_20k_fps: self.get_extreme_20k_fps(profile_id)?,
            performance_mode: self.get_performance_mode(profile_id)?,
            debounce_ms: self.get_debounce_time(profile_id)?,
        })
    }

    // ========================================================================
    // High-level write commands
    // ========================================================================

    /// Set the active profile
    ///
    /// # Arguments
    /// * `profile_id` - Profile number (1-based, typically 1-5)
    ///
    /// # Errors
    /// Returns an error if the device rejects the command
    pub fn set_profile(&self, profile_id: u8) -> Result<()> {
        // Based on official Lamzu software:
        // [2] = 0x02 (device ID)
        // [3] = 0x01 (payload length)
        // [4] = 0x00 (no category - left as zero)
        // [5] = 0x05 (set profile opcode)
        // [6] = profile_id (1-based, passed directly)
        let mut cmd = [0u8; 64];
        cmd[2] = DEVICE_ID_MOUSE;
        cmd[3] = 0x01;
        // cmd[4] = 0 (no category for profile commands)
        cmd[5] = OP_SET_PROFILE;
        cmd[6] = profile_id;

        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(())
    }

    /// Set the active DPI stage for the given profile
    ///
    /// # Arguments
    /// * `profile_id` - Profile number (1-based, typically 1-5)
    /// * `stage` - DPI stage number (1-based, typically 1-6)
    ///
    /// # Errors
    /// Returns an error if the device rejects the command
    pub fn set_dpi_stage(&self, profile_id: u8, stage: u8) -> Result<()> {
        let cmd = build_command(
            DEVICE_ID_MOUSE,
            0x02,
            CATEGORY_CONFIG,
            OP_SET_ACTIVE_DPI,
            &[profile_id, stage],
        );

        let response = self.send_and_receive(&cmd)?;
        validate_response(&response)?;

        Ok(())
    }

    // ========================================================================
    // Convenience methods
    // ========================================================================

    /// Get the current DPI value (from active profile and stage)
    ///
    /// This is a convenience method that combines getting the active profile,
    /// active DPI stage, and DPI stages to return the current DPI value.
    pub fn get_current_dpi(&self) -> Result<DpiStage> {
        let profile = self.get_profile()?;
        let active_stage = self.get_active_dpi_stage(profile)?;
        let stages = self.get_dpi_stages(profile, DEFAULT_DPI_STAGE_COUNT)?;

        stages
            .get(active_stage as usize - 1)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Active DPI stage {} not found in stages list", active_stage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_creation() {
        assert!(DeviceController::new().is_ok());
    }
}
