//! Device access for the plugin.
//!
//! All HID I/O is blocking, so it runs on tokio's blocking pool. Each
//! operation:
//! - re-enumerates and opens the device, so unplug/replug and sleep/wake are
//!   handled without any reconnect state machine;
//! - is serialized through a lock, so two requests can never interleave
//!   their feature reports;
//! - is bounded by [`IO_TIMEOUT`], so a wedged HID call can't freeze the
//!   plugin's event loop.

use anyhow::{Context, Result, anyhow};
use lamzuctl::{
    BatteryStatus, DEFAULT_DPI_STAGE_COUNT, DeviceController, DpiStage, MouseNotResponding,
};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether we can talk to the mouse
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No Lamzu device found, or it failed to open/respond
    Disconnected,
    /// Receiver present, but the mouse is asleep / off / out of range
    MouseAsleep,
    /// Mouse is responding; the readings in [`DeviceState`] are live
    Connected,
}

/// Last known device state
#[derive(Debug, Clone)]
pub struct DeviceState {
    pub status: Status,
    pub device_name: String,
    pub profile: u8,
    pub dpi_stage: u8,
    pub dpi_stages: Vec<DpiStage>,
    pub battery: BatteryStatus,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            status: Status::Disconnected,
            device_name: String::new(),
            profile: 0,
            dpi_stage: 0,
            dpi_stages: Vec::new(),
            battery: BatteryStatus {
                percentage: 0,
                charging: false,
            },
        }
    }
}

/// Live readings from one round-trip to the device
struct Reading {
    device_name: String,
    profile: u8,
    dpi_stage: u8,
    dpi_stages: Vec<DpiStage>,
    battery: BatteryStatus,
}

pub struct DeviceManager {
    state: DeviceState,
    last_refresh: Option<Instant>,
    io_lock: Arc<Mutex<()>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            state: DeviceState::default(),
            last_refresh: None,
            io_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    /// True if the cached state is older than `max_age` (or was never read)
    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.last_refresh.is_none_or(|t| t.elapsed() > max_age)
    }

    /// Re-read everything from the device. Failures are folded into
    /// [`DeviceState::status`]; the returned error is for logging only.
    pub async fn refresh(&mut self, selector: Option<String>) -> Result<()> {
        let result = self.run(selector, |_| Ok(())).await;
        self.absorb(result)
    }

    /// Switch profile, then re-read state (DPI stages differ per profile)
    pub async fn set_profile(&mut self, selector: Option<String>, profile: u8) -> Result<()> {
        let result = self.run(selector, move |c| c.set_profile(profile)).await;
        self.absorb(result)
    }

    /// Switch DPI stage on the current profile, then re-read state
    pub async fn set_dpi_stage(
        &mut self,
        selector: Option<String>,
        profile: u8,
        stage: u8,
    ) -> Result<()> {
        let result = self
            .run(selector, move |c| c.set_dpi_stage(profile, stage))
            .await;
        self.absorb(result)
    }

    /// Fold an operation's result into the cached state
    fn absorb(&mut self, result: Result<Reading>) -> Result<()> {
        self.last_refresh = Some(Instant::now());
        match result {
            Ok(r) => {
                self.state = DeviceState {
                    status: Status::Connected,
                    device_name: r.device_name,
                    profile: r.profile,
                    dpi_stage: r.dpi_stage,
                    dpi_stages: r.dpi_stages,
                    battery: r.battery,
                };
                Ok(())
            }
            Err(e) => {
                self.state.status = if e.downcast_ref::<MouseNotResponding>().is_some() {
                    Status::MouseAsleep
                } else {
                    Status::Disconnected
                };
                Err(e)
            }
        }
    }

    /// Open the device, run `op`, then read back the full state, all on the
    /// blocking pool under the I/O lock and with a timeout.
    async fn run<F>(&self, selector: Option<String>, op: F) -> Result<Reading>
    where
        F: FnOnce(&DeviceController) -> Result<()> + Send + 'static,
    {
        let io_lock = Arc::clone(&self.io_lock);
        let task = tokio::task::spawn_blocking(move || {
            // If a previous call is still stuck in the driver, fail fast
            // instead of queueing up more blocked threads behind it.
            let _guard = match io_lock.try_lock() {
                Ok(g) => g,
                Err(TryLockError::Poisoned(p)) => p.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    return Err(anyhow!("device busy: a previous request has not finished"));
                }
            };

            let devices = lamzuctl::list_devices()?;
            let device = lamzuctl::select_device(&devices, selector.as_deref())?;
            let mut controller = DeviceController::new()?;
            controller.connect_path(&device.path)?;

            op(&controller)?;

            let profile = controller.get_profile()?;
            Ok(Reading {
                device_name: device
                    .product_string
                    .clone()
                    .unwrap_or_else(|| format!("Lamzu {:04x}", device.product_id)),
                profile,
                dpi_stage: controller.get_active_dpi_stage(profile)?,
                dpi_stages: controller.get_dpi_stages(profile, DEFAULT_DPI_STAGE_COUNT)?,
                battery: controller.get_battery()?,
            })
        });

        tokio::time::timeout(IO_TIMEOUT, task)
            .await
            .map_err(|_| anyhow!("device did not respond within {}s", IO_TIMEOUT.as_secs()))?
            .context("device task panicked")?
    }
}

/// List devices for the Property Inspector's device dropdown
pub async fn list_devices() -> Result<Vec<lamzuctl::DeviceInfo>> {
    tokio::task::spawn_blocking(lamzuctl::list_devices)
        .await
        .context("device task panicked")?
}
