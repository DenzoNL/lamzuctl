//! CLI for lamzuctl

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lamzuctl")]
#[command(about = "Control utility for Lamzu gaming mice", long_about = None)]
struct Cli {
    /// Device to use (index from `list`, name substring, or PID like "001c")
    #[arg(short, long, global = true)]
    device: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List connected devices
    List,

    /// Show device info (profile, polling rate, DPI) - default command
    Info,

    /// List all profiles with their settings
    Profiles,

    /// Get device settings
    Get {
        #[command(subcommand)]
        setting: GetCommands,
    },

    /// Set device settings
    Set {
        #[command(subcommand)]
        setting: SetCommands,
    },
}

#[derive(Subcommand)]
enum GetCommands {
    /// Get current profile number
    Profile,

    /// Get polling rate in Hz
    PollingRate,

    /// Get DPI configuration
    Dpi,

    /// Get battery level and charging status
    Battery,

    /// Get firmware version
    Firmware,

    /// Get sensor settings (motion sync, LOD, angle snap, etc.)
    Sensor,

    /// Get debounce time in milliseconds
    Debounce,

    /// Get lift-off distance (LOD) in millimeters
    Lod,

    /// Get motion sync setting
    MotionSync,

    /// Get angle snap setting
    AngleSnap,

    /// Get angle tuning value
    AngleTune,

    /// Get performance mode (High-Speed or Competition)
    PerformanceMode,
}

#[derive(Subcommand)]
enum SetCommands {
    /// Set active profile
    Profile {
        /// Profile number (1 to number of configured profiles)
        #[arg(value_name = "ID")]
        id: u8,
    },

    /// Set active DPI stage, by stage number or by DPI value
    Dpi {
        /// DPI stage number (1-6), or a DPI value configured on a stage (e.g. 3200)
        #[arg(value_name = "STAGE_OR_DPI")]
        stage: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let device = cli.device.as_deref();

    // Default to Info command when no subcommand is given
    let command = cli.command.unwrap_or(Commands::Info);

    match command {
        Commands::List => commands::list(),
        Commands::Info => commands::info(device),
        Commands::Profiles => commands::profiles(device),
        Commands::Get { setting } => match setting {
            GetCommands::Profile => commands::get::profile(device),
            GetCommands::PollingRate => commands::get::polling_rate(device),
            GetCommands::Dpi => commands::get::dpi(device),
            GetCommands::Battery => commands::get::battery(device),
            GetCommands::Firmware => commands::get::firmware(device),
            GetCommands::Sensor => commands::get::sensor(device),
            GetCommands::Debounce => commands::get::debounce(device),
            GetCommands::Lod => commands::get::lod(device),
            GetCommands::MotionSync => commands::get::motion_sync(device),
            GetCommands::AngleSnap => commands::get::angle_snap(device),
            GetCommands::AngleTune => commands::get::angle_tune(device),
            GetCommands::PerformanceMode => commands::get::performance_mode(device),
        },
        Commands::Set { setting } => match setting {
            SetCommands::Profile { id } => commands::set::profile(device, id),
            SetCommands::Dpi { stage } => commands::set::dpi(device, stage),
        },
    }
}
