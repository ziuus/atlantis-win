//! Atlantis-Win Top-Left Modular Control Hub (4 Pinned + Expandable More + Customizable).

use crate::telemetry::SysStats;
use crate::win_os;
use chrono::Local;
use glass_core::animation::{Spring, SpringParams};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu};
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowBuilder, WindowLevel};

pub const CLOCK_W: u32 = 388;
pub const CLOCK_H: u32 = 640;
const COMPACT_W: f32 = 196.0;
const COMPACT_H: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockBoxKind {
    Calendar,
    Telemetry,
    Battery,
    QuickActions,
    AudioMedia,
    DisplayBright,
    Connectivity,
    SystemTools,
}

impl ClockBoxKind {
    pub fn all() -> [Self; 8] {
        [
            Self::Calendar,
            Self::Telemetry,
            Self::Battery,
            Self::QuickActions,
            Self::AudioMedia,
            Self::DisplayBright,
            Self::Connectivity,
            Self::SystemTools,
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClockBoxConfig {
    pub kind: ClockBoxKind,
    pub enabled: bool,
}

pub struct ClockWindow {
    pub window: Arc<Window>,
    pub renderer: GlassRenderer,
    pub font: UIFont,
    pub overlay: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,

    pub is_compact: bool,
    pub is_hovered: bool,
    pub show_more: bool,
    pub customizing: bool,
    pub power_save: bool,
    pub app_backdrop_solid: bool,
    pub is_window_overlapping: bool,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub blur_radius: f32,
    pub is_wallpaper_light: bool,
    pub cursor_pos: [f32; 2],

    pub size_spring: Spring,
    pub more_spring: Spring,
    pub box_configs: Vec<ClockBoxConfig>,
    pub needs_redraw: bool,
}

impl ClockWindow {
    pub fn new<T>(
        elwt: &EventLoopWindowTarget<T>,
        gpu: &SharedGpu,
        wallpaper_buf: Option<&[u8]>,
        is_light: bool,
    ) -> Self {
        let pos_x = 24;
        let pos_y = 8;
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Atlantis Control Hub")
                .with_inner_size(LogicalSize::new(CLOCK_W, CLOCK_H))
                .with_position(LogicalPosition::new(pos_x, pos_y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .build(elwt)
                .expect("create clock window"),
        );
        let mut renderer = gpu
            .create_renderer(window.clone(), CLOCK_W, CLOCK_H)
            .expect("create clock renderer");
        if let Some(bg) = wallpaper_buf {
            let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                bg,
                pos_x as u32,
                pos_y as u32,
                CLOCK_W,
                CLOCK_H,
            );
            renderer.update_background(CLOCK_W, CLOCK_H, &crop);
        }

        Self {
            window,
            renderer,
            font: UIFont::new(),
            overlay: vec![0; (CLOCK_W * CLOCK_H * 4) as usize],
            width: CLOCK_W,
            height: CLOCK_H,
            pos_x,
            pos_y,
            is_compact: true,
            is_hovered: false,
            show_more: false,
            customizing: false,
            power_save: false,
            app_backdrop_solid: true,
            is_window_overlapping: false,
            style_mode: 2.0,
            theme_mode: 0,
            blur_radius: 0.0,
            is_wallpaper_light: is_light,
            cursor_pos: [-1.0, -1.0],
            size_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 260.0,
                    damping: 26.0,
                    mass: 1.0,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
            more_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 280.0,
                    damping: 28.0,
                    mass: 0.9,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
            box_configs: ClockBoxKind::all()
                .into_iter()
                .map(|kind| ClockBoxConfig { kind, enabled: true })
                .collect(),
            needs_redraw: true,
        }
    }

    pub fn effective_is_light(&self) -> bool {
        match self.theme_mode {
            1 => false,
            2 => true,
            _ => self.is_wallpaper_light,
        }
    }

    pub fn effective_style_mode(&self) -> f32 {
        if self.app_backdrop_solid && self.is_window_overlapping {
            0.0
        } else {
            self.style_mode
        }
    }

    pub fn expand(&mut self) {
        if self.is_compact {
            self.is_compact = false;
            self.size_spring.set_target(1.0);
            self.needs_redraw = true;
        }
    }

    pub fn collapse(&mut self) {
        if !self.is_compact {
            self.is_compact = true;
            self.show_more = false;
            self.customizing = false;
            self.size_spring.set_target(0.0);
            self.more_spring.set_target(0.0);
            self.needs_redraw = true;
        }
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        if self.power_save {
            let changed = !self.size_spring.is_at_rest() || !self.more_spring.is_at_rest();
            self.size_spring.snap_to_target();
            self.more_spring.snap_to_target();
            if changed {
                self.needs_redraw = true;
            }
            return false;
        }
        let eff_dt = dt.clamp(0.0, 0.05);
        self.size_spring.update(eff_dt);
        self.more_spring.update(eff_dt);
        if (self.size_spring.value - self.size_spring.target).abs() < 0.002 {
            self.size_spring.snap_to_target();
        }
        if (self.more_spring.value - self.more_spring.target).abs() < 0.002 {
            self.more_spring.snap_to_target();
        }
        !self.size_spring.is_at_rest() || !self.more_spring.is_at_rest()
    }

    fn visible_boxes(&self) -> Vec<(usize, ClockBoxConfig)> {
        if self.customizing {
            return self.box_configs.iter().copied().enumerate().collect();
        }
        let en: Vec<(usize, ClockBoxConfig)> = self
            .box_configs
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, b)| b.enabled)
            .collect();
        if self.show_more || self.more_spring.value > 0.02 {
            en
        } else {
            en.into_iter().take(4).collect()
        }
    }

    fn box_center(&self, slot: usize) -> (f32, f32) {
        let cx = self.width as f32 * 0.5;
        let col = slot % 2;
        let row = slot / 2;
        let tcx = if col == 0 { cx - 86.0 } else { cx + 86.0 };
        let base_y = 74.0 + (row as f32) * 144.0;
        let tcy = if slot >= 4 {
            let m = self.more_spring.value.clamp(0.0, 1.0);
            218.0 + (base_y - 218.0) * m
        } else {
            base_y
        };
        (tcx, tcy)
    }

    fn footer_bar_y(&self) -> f32 {
        let count = self.visible_boxes().len().max(1);
        let full_rows = count.div_ceil(2) as f32;
        let pinned_rows = full_rows.min(2.0);
        let m = if self.customizing {
            1.0
        } else {
            self.more_spring.value.clamp(0.0, 1.0)
        };
        (pinned_rows + (full_rows - pinned_rows) * m) * 144.0 + 16.0
    }

    pub fn handle_event(&mut self, event: &WindowEvent, stats: &mut SysStats) -> bool {
        match event {
            WindowEvent::CursorEntered { .. } => {
                self.is_hovered = true;
                if self.is_compact {
                    self.expand();
                }
                true
            }
            WindowEvent::CursorLeft { .. } => {
                self.is_hovered = false;
                if !self.is_compact && !self.customizing {
                    self.collapse();
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = [
                    position.x as f32 / self.width as f32,
                    position.y as f32 / self.height as f32,
                ];
                false
            }
            WindowEvent::KeyboardInput { event: ke, .. } if ke.state == ElementState::Pressed => {
                if ke.logical_key == Key::Named(NamedKey::Escape) && !self.is_compact {
                    self.collapse();
                    return true;
                }
                false
            }
            WindowEvent::MouseWheel { delta, .. } if !self.is_compact => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 24.0,
                };
                if dy < -0.2 && !self.show_more {
                    self.show_more = true;
                    self.more_spring.set_target(1.0);
                    self.needs_redraw = true;
                } else if dy > 0.2 && self.show_more && !self.customizing {
                    self.show_more = false;
                    self.more_spring.set_target(0.0);
                    self.needs_redraw = true;
                }
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.is_compact {
                    self.expand();
                    return true;
                }
                let px = self.cursor_pos[0] * self.width as f32;
                let py = self.cursor_pos[1] * self.height as f32;
                let cx = self.width as f32 * 0.5;
                let bar_y = self.footer_bar_y();
                if (py - bar_y).abs() <= 14.0 {
                    if !self.customizing && (px - (cx - 58.0)).abs() <= 48.0 {
                        self.show_more = !self.show_more;
                        self.more_spring
                            .set_target(if self.show_more { 1.0 } else { 0.0 });
                        self.needs_redraw = true;
                        return true;
                    }
                    if (px - (cx + 58.0)).abs() <= 48.0 {
                        self.customizing = !self.customizing;
                        if self.customizing {
                            self.show_more = true;
                            self.more_spring.set_target(1.0);
                        }
                        self.needs_redraw = true;
                        return true;
                    }
                }
                let vis = self.visible_boxes();
                for (slot, (_, cfg)) in vis.iter().enumerate() {
                    let (tcx, tcy) = self.box_center(slot);
                    if (px - tcx).abs() <= 80.0 && (py - tcy).abs() <= 66.0 {
                        if self.customizing && (py - (tcy - 44.0)).abs() <= 14.0 {
                            if (px - (tcx - 58.0)).abs() <= 14.0 && slot > 0 {
                                self.box_configs.swap(slot, slot - 1);
                            } else if (px - (tcx - 32.0)).abs() <= 14.0
                                && slot + 1 < self.box_configs.len()
                            {
                                self.box_configs.swap(slot, slot + 1);
                            } else if (px - (tcx + 48.0)).abs() <= 24.0 {
                                self.box_configs[slot].enabled = !self.box_configs[slot].enabled;
                            }
                            self.needs_redraw = true;
                            return true;
                        }
                        match cfg.kind {
                            ClockBoxKind::AudioMedia => {
                                if px < tcx - 20.0 {
                                    win_os::adjust_volume(-5);
                                    stats.vol_pct = stats.vol_pct.saturating_sub(5);
                                } else if px > tcx + 20.0 {
                                    win_os::adjust_volume(5);
                                    stats.vol_pct = (stats.vol_pct + 5).min(100);
                                } else {
                                    win_os::adjust_volume(0);
                                    stats.vol_muted = !stats.vol_muted;
                                }
                            }
                            ClockBoxKind::DisplayBright => {
                                if px < tcx {
                                    stats.brightness_pct = stats.brightness_pct.saturating_sub(10).max(5);
                                } else {
                                    stats.brightness_pct = (stats.brightness_pct + 10).min(100);
                                }
                                win_os::adjust_brightness(stats.brightness_pct);
                            }
                            ClockBoxKind::SystemTools => {
                                if py < tcy + 12.0 {
                                    if px < tcx {
                                        win_os::take_screenshot();
                                    } else {
                                        win_os::launch_app("ms-settings:");
                                    }
                                } else if px < tcx {
                                    win_os::suspend();
                                } else {
                                    win_os::reboot();
                                }
                            }
                            ClockBoxKind::QuickActions => {
                                if py < tcy {
                                    if px < tcx {
                                        win_os::launch_app("wt.exe");
                                    } else {
                                        win_os::launch_app("msedge.exe");
                                    }
                                } else if px < tcx {
                                    win_os::lock_screen();
                                } else {
                                    win_os::poweroff();
                                }
                            }
                            _ => {}
                        }
                        self.needs_redraw = true;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn draw(&mut self, stats: &SysStats) {
        self.overlay.fill(0);
        let t = self.size_spring.value.clamp(0.0, 1.0);
        let is_light = self.effective_is_light();
        let eff_style = self.effective_style_mode();
        let sw = self.width as f32;
        let sh = self.height as f32;
        let cx = sw * 0.5;
        let no_shadow = [0, 0, 0, 0];

        if t <= 0.05 {
            let now = Local::now();
            let txt = format!("{}  |  {}%", now.format("%H:%M"), stats.cpu_pct);
            let col = if is_light { [12, 16, 24, 255] } else { [245, 248, 255, 255] };
            self.font.draw_text_centered(
                &mut self.overlay,
                self.width,
                self.height,
                &txt,
                108.0,
                16.0,
                12.5,
                col,
                no_shadow,
                FontWeight::SemiBold,
            );
            self.renderer.update_overlay(self.width, self.height, &self.overlay);
            let pill = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(10.0 / sw, 0.0, COMPACT_W / sw, COMPACT_H / sh)
                .blur(self.blur_radius)
                .style_mode(eff_style)
                .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
                .element_type(27.0);
            let _ = self.renderer.render(&[pill]);
            return;
        }

        let vis = self.visible_boxes();
        let m = if self.customizing { 1.0 } else { self.more_spring.value.clamp(0.0, 1.0) };
        let col_p = if is_light { [10, 14, 22, 255] } else { [255, 255, 255, 255] };
        let col_sub = if is_light { [50, 65, 85, 220] } else { [210, 224, 244, 220] };
        let now = Local::now();

        let mut surfaces = Vec::new();
        for (slot, (_, cfg)) in vis.iter().enumerate() {
            let extra_m = if slot >= 4 { m } else { 1.0 };
            if extra_m <= 0.02 {
                continue;
            }
            let (tcx, tcy) = self.box_center(slot);
            let buf = &mut self.overlay;
            let title = match cfg.kind {
                ClockBoxKind::Calendar => now.format("%A").to_string().to_uppercase(),
                ClockBoxKind::Telemetry => "CPU UTILIZATION".into(),
                ClockBoxKind::Battery => "POWER STATUS".into(),
                ClockBoxKind::QuickActions => "QUICK CONTROLS".into(),
                ClockBoxKind::AudioMedia => "AUDIO & VOLUME".into(),
                ClockBoxKind::DisplayBright => "DISPLAY & DISK".into(),
                ClockBoxKind::Connectivity => "CONNECTIVITY".into(),
                ClockBoxKind::SystemTools => "SYSTEM TOOLS".into(),
            };
            self.font.draw_text_centered_y(buf, self.width, self.height, &title, tcx - 64.0, tcy - 44.0, 10.0, col_sub, no_shadow, FontWeight::Bold);
            let val = match cfg.kind {
                ClockBoxKind::Calendar => now.format("%d %b").to_string(),
                ClockBoxKind::Telemetry => format!("{}%", stats.cpu_pct),
                ClockBoxKind::Battery => format!("{}%", stats.bat_pct),
                ClockBoxKind::QuickActions => "Term · Edge · Lock".into(),
                ClockBoxKind::AudioMedia => if stats.vol_muted { "Muted".into() } else { format!("{}%", stats.vol_pct) },
                ClockBoxKind::DisplayBright => format!("{}%", stats.brightness_pct),
                ClockBoxKind::Connectivity => "Wi-Fi · BT".into(),
                ClockBoxKind::SystemTools => "Shot · Settings".into(),
            };
            self.font.draw_text_centered_y(buf, self.width, self.height, &val, tcx - 64.0, tcy, 20.0, col_p, no_shadow, FontWeight::Bold);

            if self.customizing {
                let lbl = format!("<  >   {}", if cfg.enabled { "Show" } else { "Hide" });
                self.font.draw_text_centered(buf, self.width, self.height, &lbl, tcx, tcy + 40.0, 10.0, col_sub, no_shadow, FontWeight::Bold);
            }

            surfaces.push(
                GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(Shape::RoundedRectangle { radius: 18.0 })
                    .bounds((tcx - 80.0) / sw, (tcy - 66.0) / sh, 160.0 / sw, 132.0 / sh)
                    .style_mode(eff_style)
                    .tint(0.0, 0.0, extra_m, if is_light { 1.0 } else { 0.0 })
                    .element_type(27.0),
            );
        }

        let bar_y = self.footer_bar_y();
        if !self.customizing {
            self.font.draw_text_centered(&mut self.overlay, self.width, self.height, if self.show_more { "Less" } else { "More (4)" }, cx - 58.0, bar_y, 10.0, col_p, no_shadow, FontWeight::SemiBold);
            surfaces.push(GlassSurface::new().material(Material::LiquidGlass).shape(Shape::Capsule).bounds((cx - 104.0) / sw, (bar_y - 12.0) / sh, 92.0 / sw, 24.0 / sh).style_mode(eff_style).tint(0.0, 0.0, t, if is_light { 1.0 } else { 0.0 }).element_type(27.0));
        }
        self.font.draw_text_centered(&mut self.overlay, self.width, self.height, if self.customizing { "Done" } else { "Customize" }, cx + 58.0, bar_y, 10.0, col_p, no_shadow, FontWeight::SemiBold);
        surfaces.push(GlassSurface::new().material(Material::LiquidGlass).shape(Shape::Capsule).bounds((cx + 12.0) / sw, (bar_y - 12.0) / sh, 92.0 / sw, 24.0 / sh).style_mode(eff_style).tint(0.0, 0.0, t, if is_light { 1.0 } else { 0.0 }).element_type(27.0));

        self.renderer.update_overlay(self.width, self.height, &self.overlay);
        let _ = self.renderer.render(&surfaces);
    }
}
