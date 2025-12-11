//! Device Manager for Lamzu HID Communication
//!
//! Handles persistent HID connection with reconnection logic.

use anyhow::{Context, Result};
use lamzuctl::{BatteryStatus, DeviceController, DeviceInfo, DpiStage};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

/// Events emitted by the device manager
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected { device_name: String },
    Disconnected,
    StateUpdated(DeviceState),
    Error(String),
}

/// Cached device state
#[derive(Debug, Clone)]
pub struct DeviceState {
    pub connected: bool,
    pub device_name: String,
    pub current_profile: u8,
    pub current_dpi_stage: u8,
    pub dpi_stages: Vec<DpiStage>,
    pub battery: BatteryStatus,
    pub last_update: Option<Instant>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            connected: false,
            device_name: String::new(),
            current_profile: 1,
            current_dpi_stage: 1,
            dpi_stages: Vec::new(),
            battery: BatteryStatus {
                percentage: 0,
                charging: false,
            },
            last_update: None,
        }
    }
}

/// Manages the connection to a Lamzu device
pub struct DeviceManager {
    state: Arc<RwLock<DeviceState>>,
    device_selector: Option<String>,
    event_tx: mpsc::Sender<DeviceEvent>,
}

impl DeviceManager {
    pub fn new(event_tx: mpsc::Sender<DeviceEvent>) -> Self {
        Self {
            state: Arc::new(RwLock::new(DeviceState::default())),
            device_selector: None,
            event_tx,
        }
    }

    pub fn set_device_selector(&mut self, selector: Option<String>) {
        self.device_selector = selector;
    }

    pub fn state(&self) -> Arc<RwLock<DeviceState>> {
        Arc::clone(&self.state)
    }

    /// Get the current cached state
    pub async fn get_state(&self) -> DeviceState {
        self.state.read().await.clone()
    }

    /// Connect to the device and read initial state
    pub async fn connect(&self) -> Result<()> {
        // Device operations are blocking, run in spawn_blocking
        let selector = self.device_selector.clone();
        let (device_name, profile, dpi_stage, dpi_stages, battery) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let devices = lamzuctl::list_devices()?;

                if devices.is_empty() {
                    anyhow::bail!("No Lamzu devices found");
                }

                let device = lamzuctl::select_device(&devices, selector.as_deref())?;
                let device_name = device
                    .product_string
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                let mut controller = DeviceController::new()?;
                controller.connect_path(&device.path)?;

                let profile = controller.get_profile()?;
                let dpi_stage = controller.get_active_dpi_stage(profile)?;
                let dpi_stages = controller.get_dpi_stages(profile, 6)?;
                let battery = controller.get_battery()?;

                Ok((device_name, profile, dpi_stage, dpi_stages, battery))
            })
            .await
            .context("Device task panicked")??;

        // Update state
        {
            let mut state = self.state.write().await;
            state.connected = true;
            state.device_name = device_name.clone();
            state.current_profile = profile;
            state.current_dpi_stage = dpi_stage;
            state.dpi_stages = dpi_stages;
            state.battery = battery;
            state.last_update = Some(Instant::now());
        }

        // Notify listeners
        let _ = self
            .event_tx
            .send(DeviceEvent::Connected { device_name })
            .await;

        Ok(())
    }

    /// Execute a device operation with automatic reconnection
    async fn with_device<F, T>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&DeviceController) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let selector = self.device_selector.clone();

        let result = tokio::task::spawn_blocking(move || -> Result<T> {
            let devices = lamzuctl::list_devices()?;

            if devices.is_empty() {
                anyhow::bail!("No Lamzu devices found");
            }

            let device = lamzuctl::select_device(&devices, selector.as_deref())?;
            let mut controller = DeviceController::new()?;
            controller.connect_path(&device.path)?;

            op(&controller)
        })
        .await
        .context("Device task panicked")??;

        Ok(result)
    }

    /// Set the active profile
    pub async fn set_profile(&self, profile_id: u8) -> Result<()> {
        self.with_device(move |controller| controller.set_profile(profile_id))
            .await?;

        // Update cached state
        {
            let mut state = self.state.write().await;
            state.current_profile = profile_id;
        }

        let _ = self
            .event_tx
            .send(DeviceEvent::StateUpdated(self.get_state().await))
            .await;

        Ok(())
    }

    /// Set the active DPI stage
    pub async fn set_dpi_stage(&self, profile_id: u8, stage: u8) -> Result<()> {
        self.with_device(move |controller| controller.set_dpi_stage(profile_id, stage))
            .await?;

        // Update cached state and refresh DPI values
        let selector = self.device_selector.clone();
        let (dpi_stage, dpi_stages) = tokio::task::spawn_blocking(move || -> Result<_> {
            let devices = lamzuctl::list_devices()?;
            let device = lamzuctl::select_device(&devices, selector.as_deref())?;
            let mut controller = DeviceController::new()?;
            controller.connect_path(&device.path)?;

            let dpi_stage = controller.get_active_dpi_stage(profile_id)?;
            let dpi_stages = controller.get_dpi_stages(profile_id, 6)?;

            Ok((dpi_stage, dpi_stages))
        })
        .await
        .context("Device task panicked")??;

        {
            let mut state = self.state.write().await;
            state.current_dpi_stage = dpi_stage;
            state.dpi_stages = dpi_stages;
        }

        let _ = self
            .event_tx
            .send(DeviceEvent::StateUpdated(self.get_state().await))
            .await;

        Ok(())
    }

    /// Refresh battery status
    pub async fn refresh_battery(&self) -> Result<BatteryStatus> {
        let battery = self
            .with_device(|controller| controller.get_battery())
            .await?;

        {
            let mut state = self.state.write().await;
            state.battery = battery;
            state.last_update = Some(Instant::now());
        }

        let _ = self
            .event_tx
            .send(DeviceEvent::StateUpdated(self.get_state().await))
            .await;

        Ok(battery)
    }

    /// Refresh all device state
    pub async fn refresh_state(&self) -> Result<DeviceState> {
        let selector = self.device_selector.clone();

        let (device_name, profile, dpi_stage, dpi_stages, battery) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let devices = lamzuctl::list_devices()?;

                if devices.is_empty() {
                    anyhow::bail!("No Lamzu devices found");
                }

                let device = lamzuctl::select_device(&devices, selector.as_deref())?;
                let device_name = device
                    .product_string
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string());

                let mut controller = DeviceController::new()?;
                controller.connect_path(&device.path)?;

                let profile = controller.get_profile()?;
                let dpi_stage = controller.get_active_dpi_stage(profile)?;
                let dpi_stages = controller.get_dpi_stages(profile, 6)?;
                let battery = controller.get_battery()?;

                Ok((device_name, profile, dpi_stage, dpi_stages, battery))
            })
            .await
            .context("Device task panicked")??;

        let new_state = {
            let mut state = self.state.write().await;
            state.connected = true;
            state.device_name = device_name;
            state.current_profile = profile;
            state.current_dpi_stage = dpi_stage;
            state.dpi_stages = dpi_stages;
            state.battery = battery;
            state.last_update = Some(Instant::now());
            state.clone()
        };

        let _ = self
            .event_tx
            .send(DeviceEvent::StateUpdated(new_state.clone()))
            .await;

        Ok(new_state)
    }

    /// List available devices
    pub async fn list_devices() -> Result<Vec<DeviceInfo>> {
        tokio::task::spawn_blocking(|| lamzuctl::list_devices())
            .await
            .context("Device task panicked")?
    }
}

/// Battery polling task
pub async fn battery_poll_task(
    device_manager: Arc<DeviceManager>,
    mut stop_rx: mpsc::Receiver<()>,
    poll_interval: Duration,
) {
    let mut interval = tokio::time::interval(poll_interval);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = device_manager.refresh_battery().await {
                    eprintln!("Battery poll failed: {}", e);
                }
            }
            _ = stop_rx.recv() => {
                break;
            }
        }
    }
}
