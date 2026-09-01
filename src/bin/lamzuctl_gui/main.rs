// lamzuctl-gui — egui frontend for the lamzuctl library.
//
// See gui-handoff/DESIGN.md and gui-handoff/mockup.png for the visual reference.
// The `// (n)` comments below map to the numbered pins on the mockup.
//
// v1 scope: profile switching, DPI stage selection / cycle, battery display.
// Knobs (polling rate, lift-off, motion sync, LED) are display-only — the
// library doesn't have setters for them yet.

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use eframe::{egui, NativeOptions};
use egui::{
    vec2, Align, Color32, ComboBox, Frame, Grid, Layout, RichText, Sense, SidePanel, Visuals,
};
use lamzuctl::{
    BatteryStatus, DeviceController, DpiStage, PerformanceMode, SensorSettings,
    DEFAULT_DPI_STAGE_COUNT,
};

const PROFILE_COUNT: usize = 3;
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
const ACCENT: Color32 = Color32::from_rgb(0xd4, 0x50, 0x2a);

// ─────────────────────────────────────────────────────────────────────────────
// Cached device state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ProfileSnapshot {
    polling_rate_hz: u16,
    active_dpi_stage: u8, // 1-based
    dpi_stages: Vec<DpiStage>,
    sensor: SensorSettings,
}

#[derive(Clone, Debug, Default)]
struct DeviceSnapshot {
    device_name: String,
    battery: Option<BatteryStatus>,
    current_profile: u8, // 1-based
    profiles: Vec<ProfileSnapshot>,
}

#[derive(Debug)]
enum DeviceEvt {
    Connected(DeviceSnapshot),
    Snapshot(DeviceSnapshot),
    Disconnected,
    Error(String),
}

#[derive(Debug)]
enum DeviceCmd {
    SetProfile(u8),
    SetDpiStage { profile: u8, stage: u8 },
}

// ─────────────────────────────────────────────────────────────────────────────
// Worker thread
// ─────────────────────────────────────────────────────────────────────────────

fn spawn_worker(
    ctx: egui::Context,
    evt_tx: mpsc::Sender<DeviceEvt>,
    cmd_rx: mpsc::Receiver<DeviceCmd>,
) {
    thread::spawn(move || {
        let mut controller: Option<DeviceController> = None;
        let mut device_name = String::new();

        loop {
            // Try to connect if we don't have a live controller.
            if controller.is_none() {
                match connect() {
                    Ok((c, name)) => {
                        controller = Some(c);
                        device_name = name;
                        match read_snapshot(controller.as_ref().unwrap(), &device_name) {
                            Ok(snap) => {
                                let _ = evt_tx.send(DeviceEvt::Connected(snap));
                                ctx.request_repaint();
                            }
                            Err(e) => {
                                let _ = evt_tx.send(DeviceEvt::Error(e.to_string()));
                                controller = None;
                            }
                        }
                    }
                    Err(_) => {
                        let _ = evt_tx.send(DeviceEvt::Disconnected);
                        ctx.request_repaint();
                        thread::sleep(RECONNECT_INTERVAL);
                        continue;
                    }
                }
            }

            // Drain inbound commands; abort the controller on hard errors.
            let mut force_refresh = false;
            let mut drop_controller = false;
            while let Ok(cmd) = cmd_rx.try_recv() {
                let c = controller.as_ref().unwrap();
                let result = match cmd {
                    DeviceCmd::SetProfile(id) => c.set_profile(id),
                    DeviceCmd::SetDpiStage { profile, stage } => c.set_dpi_stage(profile, stage),
                };
                if let Err(e) = result {
                    let _ = evt_tx.send(DeviceEvt::Error(e.to_string()));
                    drop_controller = true;
                    break;
                }
                force_refresh = true;
            }
            if drop_controller {
                controller = None;
                continue;
            }

            // Periodic poll (or immediate one if a command just ran).
            if let Some(c) = &controller {
                match read_snapshot(c, &device_name) {
                    Ok(snap) => {
                        let _ = evt_tx.send(DeviceEvt::Snapshot(snap));
                        ctx.request_repaint();
                    }
                    Err(e) => {
                        let _ = evt_tx.send(DeviceEvt::Error(e.to_string()));
                        controller = None;
                        continue;
                    }
                }
            }

            // Wait for the next tick, but wake early if a command arrives.
            // mpsc::Receiver doesn't expose a "peek-blocking" easily, so use
            // recv_timeout in a tight inner loop: if we got something, drain
            // it on the next outer iteration.
            if !force_refresh {
                if let Ok(cmd) = cmd_rx.recv_timeout(POLL_INTERVAL) {
                    // Put it back via a side channel: since mpsc doesn't have
                    // push-front, just handle it inline here.
                    let c = controller.as_ref().unwrap();
                    let result = match cmd {
                        DeviceCmd::SetProfile(id) => c.set_profile(id),
                        DeviceCmd::SetDpiStage { profile, stage } => {
                            c.set_dpi_stage(profile, stage)
                        }
                    };
                    if let Err(e) = result {
                        let _ = evt_tx.send(DeviceEvt::Error(e.to_string()));
                        controller = None;
                    }
                }
            }
        }
    });
}

fn connect() -> anyhow::Result<(DeviceController, String)> {
    let devices = lamzuctl::list_devices()?;
    if devices.is_empty() {
        anyhow::bail!("no devices");
    }
    let device = lamzuctl::select_device(&devices, None)?;
    let name = device
        .product_string
        .clone()
        .unwrap_or_else(|| format!("Lamzu {:04x}", device.product_id));
    let mut controller = DeviceController::new()?;
    controller.connect_path(&device.path)?;
    Ok((controller, name))
}

fn read_snapshot(c: &DeviceController, device_name: &str) -> anyhow::Result<DeviceSnapshot> {
    let battery = c.get_battery().ok();
    let current_profile = c.get_profile()?;

    let mut profiles = Vec::with_capacity(PROFILE_COUNT);
    for profile_id in 1..=(PROFILE_COUNT as u8) {
        let polling_rate_hz = c.get_polling_rate(profile_id).unwrap_or(0);
        let active_dpi_stage = c.get_active_dpi_stage(profile_id).unwrap_or(1);
        let dpi_stages = c
            .get_dpi_stages(profile_id, DEFAULT_DPI_STAGE_COUNT)
            .unwrap_or_default();
        let sensor = c
            .get_sensor_settings(profile_id)
            .unwrap_or(SensorSettings {
                motion_sync: false,
                angle_snap: false,
                angle_tune: 0,
                ripple_control: false,
                lod_mm: 1.0,
                extreme_20k_fps: false,
                performance_mode: PerformanceMode::HighSpeed,
                debounce_ms: 0,
            });

        profiles.push(ProfileSnapshot {
            polling_rate_hz,
            active_dpi_stage,
            dpi_stages,
            sensor,
        });
    }

    Ok(DeviceSnapshot {
        device_name: device_name.to_string(),
        battery,
        current_profile,
        profiles,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────────────────

struct App {
    state: DeviceSnapshot,
    connected: bool,
    last_error: Option<String>,
    rx: mpsc::Receiver<DeviceEvt>,
    tx: mpsc::Sender<DeviceCmd>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_native_font(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx);

        let (evt_tx, evt_rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        spawn_worker(cc.egui_ctx.clone(), evt_tx, cmd_rx);

        Self {
            state: DeviceSnapshot::default(),
            connected: false,
            last_error: None,
            rx: evt_rx,
            tx: cmd_tx,
        }
    }

    fn drain_events(&mut self) {
        while let Ok(evt) = self.rx.try_recv() {
            match evt {
                DeviceEvt::Connected(snap) | DeviceEvt::Snapshot(snap) => {
                    self.state = snap;
                    self.connected = true;
                    self.last_error = None;
                }
                DeviceEvt::Disconnected => {
                    self.connected = false;
                    self.state = DeviceSnapshot::default();
                }
                DeviceEvt::Error(e) => {
                    self.last_error = Some(e);
                }
            }
        }
    }

    fn send(&self, cmd: DeviceCmd) {
        let _ = self.tx.send(cmd);
    }

    fn active_profile_index(&self) -> Option<usize> {
        let id = self.state.current_profile;
        if id == 0 {
            return None;
        }
        let idx = (id as usize).saturating_sub(1);
        if idx < self.state.profiles.len() {
            Some(idx)
        } else {
            None
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();

        // (2) Profile sidebar
        SidePanel::left("profiles")
            .resizable(false)
            .exact_width(170.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("PROFILES").small().weak()); // (3)

                let mut clicked: Option<u8> = None;
                for slot in 0..PROFILE_COUNT {
                    let profile_id = (slot + 1) as u8;
                    let selected = self.state.current_profile == profile_id;
                    if profile_row(ui, profile_id, selected, self.connected) {
                        clicked = Some(profile_id);
                    }
                }
                if let Some(id) = clicked {
                    self.state.current_profile = id;
                    self.send(DeviceCmd::SetProfile(id));
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    // (5) Footer
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let pct = self.state.battery.map(|b| b.percentage).unwrap_or(0);
                        let charging = self.state.battery.map(|b| b.charging).unwrap_or(false);
                        battery_gauge(ui, pct, charging); // (6)
                        let label = if self.connected {
                            if charging {
                                format!("{}%  ⚡", pct)
                            } else {
                                format!("{}%", pct)
                            }
                        } else {
                            "—".to_string()
                        };
                        ui.label(label);
                    });
                    let model = if self.connected {
                        self.state.device_name.as_str()
                    } else {
                        "no device"
                    };
                    ui.label(model);
                    ui.separator();
                });
            });

        // (7) Main pane
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.connected {
                disconnected_view(ui, self.last_error.as_deref());
                return;
            }

            let Some(active_idx) = self.active_profile_index() else {
                ui.label("Waiting for device data…");
                return;
            };
            let profile_id = (active_idx + 1) as u8;
            let p = self.state.profiles[active_idx].clone();

            // (8) Heading row
            ui.heading(format!("Profile {}", profile_id));

            ui.add_space(6.0);
            ui.label(RichText::new("DPI STAGES").small().weak());

            // (9) DPI stages list — display + active selection
            Frame::group(ui.style()).show(ui, |ui| {
                if p.dpi_stages.is_empty() {
                    ui.weak("(no stages configured for this profile)");
                } else {
                    let active_idx_dpi = (p.active_dpi_stage as usize).saturating_sub(1);
                    let mut clicked_stage: Option<u8> = None;
                    for (i, stage) in p.dpi_stages.iter().enumerate() {
                        let selected = i == active_idx_dpi;
                        let label = if stage.x == stage.y {
                            format!("{:>5}", stage.x)
                        } else {
                            format!("{:>5}x{}", stage.x, stage.y)
                        };
                        if ui
                            .selectable_label(selected, RichText::new(label).monospace())
                            .clicked()
                        {
                            clicked_stage = Some((i + 1) as u8);
                        }
                    }
                    if let Some(stage) = clicked_stage {
                        self.send(DeviceCmd::SetDpiStage {
                            profile: profile_id,
                            stage,
                        });
                    }
                }
            });

            ui.add_space(10.0);

            // (11) Knob grid — display-only in v1
            Grid::new("knobs")
                .num_columns(2)
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    knob_combo(
                        // (12)
                        ui,
                        "polling rate",
                        polling_rate_label(p.polling_rate_hz),
                        &["125 Hz", "500 Hz", "1000 Hz", "4000 Hz", "8000 Hz"],
                    );
                    knob_combo(
                        ui,
                        "lift-off",
                        format!("{:.1} mm", p.sensor.lod_mm),
                        &["1.0 mm", "2.0 mm"],
                    );
                    ui.end_row();
                    knob_toggle(ui, "motion sync", p.sensor.motion_sync); // (13)
                    knob_text(
                        ui,
                        "performance",
                        match p.sensor.performance_mode {
                            PerformanceMode::HighSpeed => "High-Speed",
                            PerformanceMode::Competition => "Competition",
                            _ => "Unknown",
                        },
                    );
                    ui.end_row();
                });

            if let Some(err) = &self.last_error {
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.colored_label(ACCENT, format!("⚠ {}", truncate(err, 80)));
                });
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Widgets
// ─────────────────────────────────────────────────────────────────────────────

fn profile_row(ui: &mut egui::Ui, id: u8, selected: bool, enabled: bool) -> bool {
    let label = format!("P{}", id);
    let mut clicked = false;
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            // Badge box
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            let bg = if selected {
                ACCENT
            } else {
                Color32::TRANSPARENT
            };
            let fg = if selected {
                Color32::WHITE
            } else {
                ui.style().visuals.text_color()
            };
            ui.painter()
                .rect(rect, 4.0, bg, (1.25, ui.style().visuals.text_color()), egui::StrokeKind::Middle);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(12.0),
                fg,
            );
            // Name to the right of the badge — for v1 just "Profile N"
            let row_resp = ui.selectable_label(selected, format!("Profile {}", id));
            if row_resp.clicked() {
                clicked = true;
            }
        });
    });
    clicked
}

fn battery_gauge(ui: &mut egui::Ui, pct: u8, charging: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(46.0, 18.0), Sense::hover());
    let body = egui::Rect::from_min_size(rect.min, vec2(40.0, 18.0));
    let tip = egui::Rect::from_min_size(
        egui::pos2(body.max.x, rect.min.y + 5.0),
        vec2(4.0, 8.0),
    );
    let ink = ui.style().visuals.text_color();
    ui.painter().rect_stroke(body, 3.0, (1.25, ink), egui::StrokeKind::Middle);
    ui.painter().rect_filled(tip, 1.0, ink);
    let inset = body.shrink(3.0);
    let fill_w = inset.width() * (pct.min(100) as f32 / 100.0);
    let fill_rect = egui::Rect::from_min_size(inset.min, vec2(fill_w.max(0.0), inset.height()));
    let col = if charging {
        Color32::from_rgb(0x4c, 0xa5, 0x4a)
    } else if pct < 20 {
        Color32::from_rgb(0xd4, 0x50, 0x2a)
    } else {
        ink
    };
    if fill_w > 0.0 {
        ui.painter().rect_filled(fill_rect, 1.0, col);
    }
}

fn knob_combo(ui: &mut egui::Ui, label: &str, current: impl Into<String>, options: &[&str]) {
    let current = current.into();
    Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(160.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(label.to_uppercase()).small().weak());
            ui.add_enabled_ui(false, |ui| {
                ComboBox::from_id_salt(label)
                    .selected_text(&current)
                    .show_ui(ui, |_ui| {})
                    .response
                    .on_disabled_hover_text("read-only in v1");
            });
            ui.label(
                RichText::new(options.join(" · "))
                    .small()
                    .weak(),
            );
        });
    });
}

fn knob_toggle(ui: &mut egui::Ui, label: &str, value: bool) {
    Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(160.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label.to_uppercase()).small().weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut v = value;
                    ui.add_enabled_ui(false, |ui| {
                        ui.checkbox(&mut v, "")
                            .on_disabled_hover_text("read-only in v1");
                    });
                });
            });
            ui.label(if value { "on" } else { "off" });
        });
    });
}

fn knob_text(ui: &mut egui::Ui, label: &str, value: &str) {
    Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(160.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(label.to_uppercase()).small().weak());
            ui.label(RichText::new(value).strong());
        });
    });
}

fn disconnected_view(ui: &mut egui::Ui, last_error: Option<&str>) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.heading("No Lamzu mouse connected");
        ui.add_space(8.0);
        ui.label("Plug in your mouse or its dongle. lamzuctl will reconnect automatically.");
        if let Some(err) = last_error {
            ui.add_space(12.0);
            ui.colored_label(
                Color32::from_rgb(0xd4, 0x50, 0x2a),
                format!("Last error: {}", truncate(err, 200)),
            );
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn polling_rate_label(hz: u16) -> String {
    if hz == 0 {
        "—".to_string()
    } else if hz >= 1000 {
        format!("{} Hz ({}K)", hz, hz / 1000)
    } else {
        format!("{} Hz", hz)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

fn apply_theme(ctx: &egui::Context) {
    ctx.set_visuals(Visuals::dark());
    let mut v = ctx.style().visuals.clone();
    v.selection.bg_fill = ACCENT;
    v.selection.stroke.color = Color32::WHITE;
    ctx.set_visuals(v);

    // Scale the whole UI up — egui's defaults read small next to native Windows apps.
    ctx.set_pixels_per_point(ctx.native_pixels_per_point().unwrap_or(1.0) * 1.25);
}

fn install_native_font(ctx: &egui::Context) {
    // On Windows, fall back to Segoe UI (the system UI font) so the app
    // visually matches the rest of the OS. If the file isn't present for
    // any reason, egui's bundled font is used.
    let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "segoe_ui".to_owned(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "segoe_ui".to_owned());
    ctx.set_fonts(fonts);
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    let opts = NativeOptions {
        viewport: egui::ViewportBuilder::default() // (1)
            .with_inner_size([800.0, 640.0])
            .with_min_inner_size([640.0, 520.0])
            .with_title("lamzuctl"),
        ..Default::default()
    };
    eframe::run_native(
        "lamzuctl",
        opts,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
