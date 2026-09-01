//! Data structures for lamzuctl

/// Default number of profiles on Lamzu mice
pub const DEFAULT_PROFILE_COUNT: u8 = 5;

/// Default number of DPI stages per profile on Lamzu mice
pub const DEFAULT_DPI_STAGE_COUNT: u8 = 6;

/// Error for when the wireless mouse is not answering the dongle.
///
/// The dongle keeps acknowledging HID commands while the mouse is asleep,
/// powered off, or out of range — but every mouse-sourced value reads zero
/// (battery 0%, profile 0, firmware 0.0.0.0). Rather than passing those
/// zeros off as real data, commands fail with this error so callers can tell
/// "mouse asleep" apart from "battery genuinely empty". The CLI maps it to
/// exit code 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseNotResponding;

impl std::fmt::Display for MouseNotResponding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mouse is not responding (asleep, powered off, or out of range); move it to wake it"
        )
    }
}

impl std::error::Error for MouseNotResponding {}

/// Battery status information
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BatteryStatus {
    /// Battery percentage (0-100)
    pub percentage: u8,
    /// Whether the device is currently charging
    pub charging: bool,
}

/// DPI stage configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DpiStage {
    /// X-axis DPI value
    pub x: u16,
    /// Y-axis DPI value
    pub y: u16,
}

/// RGB color value
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RgbColor {
    /// Red component (0-255)
    pub r: u8,
    /// Green component (0-255)
    pub g: u8,
    /// Blue component (0-255)
    pub b: u8,
}

impl RgbColor {
    /// Create a new RGB color
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert to hex string (e.g., "#FF0000")
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl std::fmt::Display for RgbColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// Performance mode setting (High-Speed vs Competition)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PerformanceMode {
    /// High-Speed Mode - standard performance, lower power consumption
    /// Cannot be used with 2K/4K/8K polling rates
    HighSpeed,
    /// Competition Mode - maximum performance, higher power consumption
    Competition,
}

impl std::fmt::Display for PerformanceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerformanceMode::HighSpeed => write!(f, "High-Speed"),
            PerformanceMode::Competition => write!(f, "Competition"),
        }
    }
}

/// Firmware version information
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

impl std::fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.major, self.minor, self.patch, self.build)
    }
}

/// Sensor settings that can be queried from the device
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SensorSettings {
    /// Motion sync enabled (synchronizes sensor polling with USB polling)
    pub motion_sync: bool,
    /// Angle snapping enabled (straightens diagonal movements)
    pub angle_snap: bool,
    /// Angle tuning value (-128 to 127, adjusts sensor angle)
    pub angle_tune: i8,
    /// Ripple control enabled (reduces jitter at low speeds)
    pub ripple_control: bool,
    /// Lift-off distance in mm (how high mouse can be lifted before tracking stops)
    pub lod_mm: f32,
    /// 20K FPS extreme performance mode (requires Competition mode)
    pub extreme_20k_fps: bool,
    /// Performance mode (High-Speed or Competition)
    pub performance_mode: PerformanceMode,
    /// Debounce time in milliseconds
    pub debounce_ms: u8,
}

/// Profile information
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileInfo {
    /// Profile ID (1-based)
    pub id: u8,
    /// Polling rate in Hz
    pub polling_rate: u16,
    /// Active DPI stage (1-based)
    pub active_dpi_stage: u8,
    /// DPI stages configured for this profile
    pub dpi_stages: Vec<DpiStage>,
}

impl ProfileInfo {
    /// Check if this profile has been configured
    ///
    /// A profile is considered unconfigured if it has 0 Hz polling rate
    /// or no DPI stages defined.
    pub fn is_configured(&self) -> bool {
        self.polling_rate > 0 && !self.dpi_stages.is_empty()
    }
}
