//! Command handlers for the CLI

use anyhow::Result;
use colored::Colorize;
use comfy_table::{presets::NOTHING, Attribute, Cell, Color, Table};
use lamzuctl::DeviceController;

/// Connect to a device and return the controller
pub fn connect_to_device(selector: Option<&str>) -> Result<DeviceController> {
    let devices = lamzuctl::list_devices()?;
    let device = lamzuctl::select_device(&devices, selector)?;

    let mut controller = DeviceController::new()?;
    controller.connect_path(&device.path)?;

    Ok(controller)
}

/// Helper to create a key-value table with no borders
fn kv_table() -> Table {
    let mut table = Table::new();
    table.load_style(NOTHING);
    table.set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
    table
}

/// List all connected Lamzu devices
pub fn list() -> Result<()> {
    let devices = lamzuctl::list_devices()?;

    if devices.is_empty() {
        println!("No Lamzu devices found.");
        println!("\nMake sure your device is:");
        println!("  - Connected via USB or wireless dongle");
        println!("  - Powered on");
        println!("  - Not exclusively opened by another application");
        return Ok(());
    }

    println!("Found {} Lamzu device(s):\n", devices.len());

    for (idx, device) in devices.iter().enumerate() {
        let known_device = lamzuctl::lookup_device(device.vendor_id, device.product_id);

        println!("Device {} (--device {}):", idx + 1, idx + 1);

        let mut table = kv_table();

        if let Some(known) = known_device {
            table.add_row(vec!["Model", &known.name]);
        }

        table.add_row(vec![
            "VID:PID",
            &format!("{:04x}:{:04x}", device.vendor_id, device.product_id),
        ]);

        if let Some(manufacturer) = &device.manufacturer_string {
            table.add_row(vec!["Manufacturer", manufacturer]);
        }

        if let Some(product) = &device.product_string {
            table.add_row(vec!["Product", product]);
        }

        if let Some(serial) = &device.serial_number {
            table.add_row(vec!["Serial", serial]);
        }

        if let Some(iface) = device.interface_number {
            table.add_row(vec!["Interface", &iface.to_string()]);
        }

        table.add_row(vec![
            "Usage",
            &format!("{:04x}:{:04x}", device.usage_page, device.usage),
        ]);
        table.add_row(vec!["Path", &device.path]);

        if known_device.is_none() {
            table.add_row(vec![
                "Status",
                &format!("{} (not in database, may still work)", "Unknown".yellow()),
            ]);
        }

        println!("{table}");
        println!();
    }

    Ok(())
}

/// Format DPI value as string
fn format_dpi(stage: &lamzuctl::DpiStage) -> String {
    if stage.x == stage.y {
        format!("{}", stage.x)
    } else {
        format!("{}x{}", stage.x, stage.y)
    }
}

/// Convert RGB to comfy_table Color
fn rgb_to_color(color: &lamzuctl::RgbColor) -> Color {
    Color::Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

/// Show device info summary
pub fn info(device_selector: Option<&str>) -> Result<()> {
    let devices = lamzuctl::list_devices()?;
    let device = lamzuctl::select_device(&devices, device_selector)?;
    let device_name = device.product_string.as_deref().unwrap_or("Unknown");

    let mut controller = DeviceController::new()?;
    controller.connect_path(&device.path)?;

    let profile = controller.get_profile()?;
    let configured_profiles = controller.get_configured_profiles()?;
    let polling_rate = controller.get_polling_rate(profile)?;
    let battery = controller.get_battery()?;
    let active_stage = controller.get_active_dpi_stage(profile)?;
    let dpi_stages = controller.get_dpi_stages(profile, 6)?;
    let colors = controller.get_dpi_colors(profile)?;
    let mouse_fw = controller.get_firmware_version()?;
    let dongle_fw = controller.get_dongle_firmware_version()?;
    let sensor = controller.get_sensor_settings(profile)?;

    // Device info table
    let mut table = kv_table();
    table.add_row(vec!["Device", device_name]);
    table.add_row(vec![
        "Firmware",
        &format!("{} (dongle {})", mouse_fw, dongle_fw),
    ]);
    table.add_row(vec![
        "Profile",
        &format!("{} of {}", profile, configured_profiles.len()),
    ]);
    table.add_row(vec!["Polling Rate", &format!("{} Hz", polling_rate)]);

    let battery_text = if battery.charging {
        format!("{}% (charging)", battery.percentage)
    } else {
        format!("{}%", battery.percentage)
    };
    table.add_row(vec!["Battery", &battery_text]);
    println!("{table}\n");

    // DPI stages table
    let mut dpi_table = Table::new();
    dpi_table.load_style(NOTHING);
    dpi_table.set_header(vec![
        Cell::new("Stage").add_attribute(Attribute::Bold),
        Cell::new("DPI").add_attribute(Attribute::Bold),
        Cell::new("").add_attribute(Attribute::Bold),
    ]);

    for (i, stage) in dpi_stages.iter().enumerate() {
        let stage_num = i + 1;
        let color = colors.get(i).copied().unwrap_or_default();
        let active_marker = if stage_num == active_stage as usize {
            "[active]"
        } else {
            ""
        };

        dpi_table.add_row(vec![
            Cell::new(stage_num),
            Cell::new(format_dpi(stage)).fg(rgb_to_color(&color)),
            Cell::new(active_marker),
        ]);
    }
    println!("{dpi_table}\n");

    // Sensor settings table
    let mut sensor_table = kv_table();
    sensor_table.add_row(vec!["LOD", &format!("{} mm", sensor.lod_mm)]);
    sensor_table.add_row(vec![
        "Motion Sync",
        if sensor.motion_sync { "On" } else { "Off" },
    ]);
    sensor_table.add_row(vec![
        "Angle Snap",
        if sensor.angle_snap { "On" } else { "Off" },
    ]);
    if sensor.angle_tune != 0 {
        sensor_table.add_row(vec!["Angle Tune", &format!("{}°", sensor.angle_tune)]);
    }
    sensor_table.add_row(vec!["Performance", &sensor.performance_mode.to_string()]);
    if sensor.extreme_20k_fps {
        sensor_table.add_row(vec!["20K FPS", "On"]);
    }
    sensor_table.add_row(vec!["Debounce", &format!("{} ms", sensor.debounce_ms)]);
    println!("{sensor_table}");

    Ok(())
}

/// List all profiles
pub fn profiles(device_selector: Option<&str>) -> Result<()> {
    let devices = lamzuctl::list_devices()?;
    let device = lamzuctl::select_device(&devices, device_selector)?;

    let mut controller = DeviceController::new()?;
    controller.connect_path(&device.path)?;

    let current_profile = controller.get_profile()?;
    let profiles = controller.get_configured_profiles()?;

    println!("Profiles ({} configured):\n", profiles.len());

    let mut table = Table::new();
    table.load_style(NOTHING);
    table.set_header(vec![
        Cell::new("Profile").add_attribute(Attribute::Bold),
        Cell::new("Polling").add_attribute(Attribute::Bold),
        Cell::new("DPI Stage").add_attribute(Attribute::Bold),
        Cell::new("DPI").add_attribute(Attribute::Bold),
        Cell::new("").add_attribute(Attribute::Bold),
    ]);

    for profile in &profiles {
        let active_marker = if profile.id == current_profile {
            "[active]"
        } else {
            ""
        };

        let active_dpi = profile.dpi_stages.get(profile.active_dpi_stage as usize - 1);
        let dpi_text = active_dpi.map(format_dpi).unwrap_or_default();

        table.add_row(vec![
            Cell::new(profile.id),
            Cell::new(format!("{} Hz", profile.polling_rate)),
            Cell::new(format!(
                "{} of {}",
                profile.active_dpi_stage,
                profile.dpi_stages.len()
            )),
            Cell::new(dpi_text),
            Cell::new(active_marker),
        ]);
    }

    println!("{table}");

    Ok(())
}

/// Get subcommands
pub mod get {
    use super::*;

    pub fn profile(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        println!("Profile: {}", profile);
        Ok(())
    }

    pub fn polling_rate(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let polling_rate = controller.get_polling_rate(profile)?;
        println!("Polling Rate: {} Hz", polling_rate);
        Ok(())
    }

    pub fn dpi(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let active_stage = controller.get_active_dpi_stage(profile)?;
        let dpi_stages = controller.get_dpi_stages(profile, 6)?;
        let colors = controller.get_dpi_colors(profile)?;

        println!("DPI Configuration (Profile {}):\n", profile);

        let mut table = Table::new();
        table.load_style(NOTHING);
        table.set_header(vec![
            Cell::new("Stage").add_attribute(Attribute::Bold),
            Cell::new("DPI").add_attribute(Attribute::Bold),
            Cell::new("").add_attribute(Attribute::Bold),
        ]);

        for (i, stage) in dpi_stages.iter().enumerate() {
            let stage_num = i + 1;
            let color = colors.get(i).copied().unwrap_or_default();
            let active_marker = if stage_num == active_stage as usize {
                "[active]"
            } else {
                ""
            };

            table.add_row(vec![
                Cell::new(stage_num),
                Cell::new(format_dpi(stage)).fg(rgb_to_color(&color)),
                Cell::new(active_marker),
            ]);
        }

        println!("{table}");

        Ok(())
    }

    pub fn battery(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let battery = controller.get_battery()?;
        if battery.charging {
            println!("Battery: {}% (charging)", battery.percentage);
        } else {
            println!("Battery: {}%", battery.percentage);
        }
        Ok(())
    }

    pub fn firmware(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let mouse_version = controller.get_firmware_version()?;
        let dongle_version = controller.get_dongle_firmware_version()?;
        println!("Mouse Firmware:  {}", mouse_version);
        println!("Dongle Firmware: {}", dongle_version);
        Ok(())
    }

    pub fn sensor(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let settings = controller.get_sensor_settings(profile)?;

        println!("Sensor Settings (Profile {}):\n", profile);

        let mut table = kv_table();
        table.add_row(vec![
            "Motion Sync",
            if settings.motion_sync { "On" } else { "Off" },
        ]);
        table.add_row(vec![
            "Angle Snap",
            if settings.angle_snap { "On" } else { "Off" },
        ]);
        table.add_row(vec!["Angle Tune", &format!("{}°", settings.angle_tune)]);
        table.add_row(vec![
            "Ripple Control",
            if settings.ripple_control { "On" } else { "Off" },
        ]);
        table.add_row(vec!["LOD", &format!("{} mm", settings.lod_mm)]);
        table.add_row(vec!["Performance", &settings.performance_mode.to_string()]);
        table.add_row(vec![
            "20K FPS",
            if settings.extreme_20k_fps { "On" } else { "Off" },
        ]);
        table.add_row(vec!["Debounce", &format!("{} ms", settings.debounce_ms)]);

        println!("{table}");

        Ok(())
    }

    pub fn debounce(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let debounce = controller.get_debounce_time(profile)?;
        println!("Debounce: {} ms", debounce);
        Ok(())
    }

    pub fn lod(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let lod = controller.get_lod(profile)?;
        println!("LOD: {} mm", lod);
        Ok(())
    }

    pub fn motion_sync(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let enabled = controller.get_motion_sync(profile)?;
        println!("Motion Sync: {}", if enabled { "On" } else { "Off" });
        Ok(())
    }

    pub fn angle_snap(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let enabled = controller.get_angle_snap(profile)?;
        println!("Angle Snap: {}", if enabled { "On" } else { "Off" });
        Ok(())
    }

    pub fn angle_tune(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let value = controller.get_angle_tune(profile)?;
        println!("Angle Tune: {}°", value);
        Ok(())
    }

    pub fn performance_mode(device_selector: Option<&str>) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let mode = controller.get_performance_mode(profile)?;
        println!("Performance Mode: {}", mode);
        Ok(())
    }
}

/// Set subcommands
pub mod set {
    use super::*;

    pub fn profile(device_selector: Option<&str>, id: u8) -> Result<()> {
        let controller = connect_to_device(device_selector)?;

        // Validate just the requested profile rather than enumerating every
        // profile, which costs a HID round trip per setting per profile.
        if id < 1 || id > lamzuctl::DEFAULT_PROFILE_COUNT {
            anyhow::bail!(
                "Invalid profile ID: {}. Valid range is 1-{}",
                id,
                lamzuctl::DEFAULT_PROFILE_COUNT
            );
        }

        if !controller.get_profile_info(id)?.is_configured() {
            anyhow::bail!(
                "Profile {} is not configured on this device. Run `lamzuctl profiles` \
                 to see the configured profiles.",
                id
            );
        }

        controller.set_profile(id)?;
        println!("Profile set to {}", id);

        Ok(())
    }

    pub fn dpi(device_selector: Option<&str>, value: u16) -> Result<()> {
        let controller = connect_to_device(device_selector)?;
        let profile = controller.get_profile()?;
        let dpi_stages = controller.get_dpi_stages(profile, 6)?;
        let max_stage = dpi_stages.len() as u16;

        // A small number is a stage index; anything larger is a DPI value, which
        // we resolve to the stage configured with it. The device only switches
        // between preset stages, so a DPI value has to already be on a stage.
        let stage = if value >= 1 && value <= max_stage {
            value as u8
        } else {
            match dpi_stages.iter().position(|s| s.x == value && s.y == value) {
                Some(idx) => (idx + 1) as u8,
                None => {
                    let available: Vec<String> =
                        dpi_stages.iter().map(format_dpi).collect();
                    anyhow::bail!(
                        "No DPI stage set to {} in profile {}. Valid stages are 1-{}; \
                         configured DPI values are: {}",
                        value,
                        profile,
                        max_stage,
                        available.join(", ")
                    );
                }
            }
        };

        controller.set_dpi_stage(profile, stage)?;

        // Show confirmation with the DPI value
        if let Some(dpi) = dpi_stages.get(stage as usize - 1) {
            if dpi.x == dpi.y {
                println!("DPI stage set to {} ({} DPI)", stage, dpi.x);
            } else {
                println!("DPI stage set to {} ({}x{} DPI)", stage, dpi.x, dpi.y);
            }
        } else {
            println!("DPI stage set to {}", stage);
        }

        Ok(())
    }
}
