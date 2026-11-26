//! Atlantis-Win Top-Center Dynamic Island (Compact, Notifications, ControlCenter, SystemOverview).

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

pub const ISLAND_W: u32 = 540;
pub const ISLAND_H: u32 = 180;
const COMPACT_W: f32 = 220.0;
const COMPACT_H: f32 = 34.0;
const EXPANDED_W: f32 = 500.0;
const EXPANDED_H: f32 = 160.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandPage {
    Compact,
    Notifications,
    ControlCenter,
    SystemOverview,
}

pub enum IslandAction {
    None,
    ToggleEco(bool),
    ToggleAppBg(bool),
    CycleTheme(u32),
    ToggleLauncher,
}

pub struct IslandWindow {
    pub window: Arc<Window>,
    pub renderer: GlassRenderer,
    pub font: UIFont,
    pub overlay: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,

    pub page: IslandPage,
    pub is_hovered: bool,
    pub power_save: bool,
    pub app_backdrop_solid: bool,
    pub is_window_overlapping: bool,
    pub style_mode: f32,
    pub theme_mode: u32,
    pub pure_bw: bool,
    pub accent_color: [u8; 3],
    pub blur_radius: f32,
    pub is_wallpaper_light: bool,
    pub cursor_pos: [f32; 2],

    pub size_spring: Spring,
    pub needs_redraw: bool,
}

impl IslandWindow {
    pub fn new<T>(
        elwt: &EventLoopWindowTarget<T>,
        gpu: &SharedGpu,
        screen_w: u32,
        wallpaper_buf: Option<&[u8]>,
        is_light: bool,
    ) -> Self {
        let pos_x = (screen_w as i32 - ISLAND_W as i32) / 2;
        let pos_y = 8;
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Atlantis Island")
                .with_inner_size(LogicalSize::new(ISLAND_W, ISLAND_H))
                .with_position(LogicalPosition::new(pos_x, pos_y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .build(elwt)
                .expect("create island window"),
        );
        let mut renderer = gpu
            .create_renderer(window.clone(), ISLAND_W, ISLAND_H)
            .expect("create island renderer");
        if let Some(bg) = wallpaper_buf {
            let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                bg,
                pos_x.max(0) as u32,
                pos_y as u32,
                ISLAND_W,
                ISLAND_H,
            );
            renderer.update_background(ISLAND_W, ISLAND_H, &crop);
        }

        Self {
            window,
            renderer,
            font: UIFont::new(),
            overlay: vec![0; (ISLAND_W * ISLAND_H * 4) as usize],
            width: ISLAND_W,
            height: ISLAND_H,
            pos_x,
            pos_y,
            page: IslandPage::Compact,
            is_hovered: false,
            power_save: false,
            app_backdrop_solid: true,
            is_window_overlapping: false,
            style_mode: 2.0,
            theme_mode: 0,
            pure_bw: true,
            accent_color: [0, 0, 0],
            blur_radius: 0.0,
            is_wallpaper_light: is_light,
            cursor_pos: [-1.0, -1.0],
            size_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 280.0,
                    damping: 26.0,
                    mass: 0.9,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
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

    pub fn update_physics(&mut self, dt: f32) -> bool {
        if self.power_save {
            let changed = !self.size_spring.is_at_rest();
            self.size_spring.snap_to_target();
            if changed {
                self.needs_redraw = true;
            }
            return false;
        }
        self.size_spring.update(dt.clamp(0.0, 0.05));
        if (self.size_spring.value - self.size_spring.target).abs() < 0.002 {
            self.size_spring.snap_to_target();
        }
        !self.size_spring.is_at_rest()
    }

    pub fn handle_event(&mut self, event: &WindowEvent, stats: &mut SysStats) -> IslandAction {
        match event {
            WindowEvent::CursorEntered { .. } => {
                self.is_hovered = true;
                if self.page == IslandPage::Compact {
                    self.page = IslandPage::Notifications;
                    self.size_spring.set_target(1.0);
                    self.needs_redraw = true;
                }
                IslandAction::None
            }
            WindowEvent::CursorLeft { .. } => {
                self.is_hovered = false;
                if self.page != IslandPage::Compact {
                    self.page = IslandPage::Compact;
                    self.size_spring.set_target(0.0);
                    self.needs_redraw = true;
                }
                IslandAction::None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = [
                    position.x as f32 / self.width as f32,
                    position.y as f32 / self.height as f32,
                ];
                IslandAction::None
            }
            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed
                    && ke.logical_key == Key::Named(NamedKey::Escape) =>
            {
                self.page = IslandPage::Compact;
                self.size_spring.set_target(0.0);
                self.needs_redraw = true;
                IslandAction::None
            }
            WindowEvent::MouseWheel { delta, .. } if self.page != IslandPage::Compact => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 24.0,
                };
                if dy < -0.2 {
                    self.page = match self.page {
                        IslandPage::Notifications => IslandPage::ControlCenter,
                        IslandPage::ControlCenter => IslandPage::SystemOverview,
                        _ => IslandPage::Notifications,
                    };
                    self.needs_redraw = true;
                } else if dy > 0.2 {
                    self.page = match self.page {
                        IslandPage::SystemOverview => IslandPage::ControlCenter,
                        IslandPage::ControlCenter => IslandPage::Notifications,
                        _ => IslandPage::SystemOverview,
                    };
                    self.needs_redraw = true;
                }
                IslandAction::None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.page == IslandPage::Compact {
                    self.page = IslandPage::Notifications;
                    self.size_spring.set_target(1.0);
                    self.needs_redraw = true;
                    return IslandAction::None;
                }
                let lx = self.cursor_pos[0] * self.width as f32;
                let ly = self.cursor_pos[1] * self.height as f32;
                let cx = self.width as f32 * 0.5;
                if ly >= 134.0 {
                    if (lx - (cx - 48.0)).abs() <= 40.0 {
                        self.page = IslandPage::Notifications;
                    } else if (lx - cx).abs() <= 40.0 {
                        self.page = IslandPage::ControlCenter;
                    } else if (lx - (cx + 48.0)).abs() <= 40.0 {
                        self.page = IslandPage::SystemOverview;
                    }
                    self.needs_redraw = true;
                    return IslandAction::None;
                }
                if self.page == IslandPage::ControlCenter {
                    if (ly - 28.0).abs() <= 14.0 {
                        if (lx - (cx + 150.0)).abs() <= 44.0 {
                            self.app_backdrop_solid = !self.app_backdrop_solid;
                            self.needs_redraw = true;
                            return IslandAction::ToggleAppBg(self.app_backdrop_solid);
                        }
                        if (lx - (cx + 76.0)).abs() <= 30.0 {
                            self.power_save = !self.power_save;
                            self.needs_redraw = true;
                            return IslandAction::ToggleEco(self.power_save);
                        }
                    } else if (ly - 72.0).abs() <= 18.0 {
                        if lx < cx - 60.0 {
                            self.theme_mode = (self.theme_mode + 1) % 3;
                            self.needs_redraw = true;
                            return IslandAction::CycleTheme(self.theme_mode);
                        } else if lx < cx + 60.0 {
                            win_os::adjust_volume(0);
                            stats.vol_muted = !stats.vol_muted;
                            self.needs_redraw = true;
                        } else {
                            return IslandAction::ToggleLauncher;
                        }
                    }
                }
                IslandAction::None
            }
            _ => IslandAction::None,
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
        let col_ink = if is_light {
            if self.pure_bw {
                [0, 0, 0, 255]
            } else {
                [10, 14, 24, 255]
            }
        } else {
            if self.pure_bw {
                [255, 255, 255, 255]
            } else {
                [248, 250, 255, 255]
            }
        };
        let col_dim = if is_light {
            if self.pure_bw {
                [0, 0, 0, 220]
            } else {
                [50, 65, 85, 200]
            }
        } else {
            if self.pure_bw {
                [255, 255, 255, 220]
            } else {
                [205, 218, 238, 200]
            }
        };
        let no_shadow = [0, 0, 0, 0];

        let cur_w = COMPACT_W + (EXPANDED_W - COMPACT_W) * t;
        let cur_h = COMPACT_H + (EXPANDED_H - COMPACT_H) * t;

        if t <= 0.08 {
            let txt = format!("Atlantis  ·  {}", Local::now().format("%H:%M"));
            self.font.draw_text_centered(
                &mut self.overlay,
                self.width,
                self.height,
                &txt,
                cx,
                cur_h * 0.5,
                12.0,
                col_ink,
                no_shadow,
                FontWeight::SemiBold,
            );
        } else {
            let hdr = match self.page {
                IslandPage::Notifications => "NOTIFICATIONS",
                IslandPage::ControlCenter => "CONTROL CENTER",
                IslandPage::SystemOverview => "SYSTEM OVERVIEW",
                IslandPage::Compact => "ATLANTIS",
            };
            self.font.draw_text(
                &mut self.overlay,
                self.width,
                self.height,
                hdr,
                cx - cur_w * 0.5 + 24.0,
                32.0,
                10.5,
                col_dim,
                FontWeight::Bold,
            );

            match self.page {
                IslandPage::Notifications => {
                    self.font.draw_text_centered(
                        &mut self.overlay,
                        self.width,
                        self.height,
                        "No unread notifications",
                        cx,
                        84.0,
                        13.0,
                        col_dim,
                        no_shadow,
                        FontWeight::Medium,
                    );
                }
                IslandPage::ControlCenter => {
                    let eco = if self.power_save { "Eco" } else { "Full" };
                    let ab = if self.app_backdrop_solid {
                        "App BG: Solid"
                    } else {
                        "App BG: Glass"
                    };
                    self.font.draw_text_centered(
                        &mut self.overlay,
                        self.width,
                        self.height,
                        eco,
                        cx + 76.0,
                        28.0,
                        10.0,
                        col_ink,
                        no_shadow,
                        FontWeight::SemiBold,
                    );
                    self.font.draw_text_centered(
                        &mut self.overlay,
                        self.width,
                        self.height,
                        ab,
                        cx + 150.0,
                        28.0,
                        10.0,
                        col_ink,
                        no_shadow,
                        FontWeight::SemiBold,
                    );

                    let th = match self.theme_mode {
                        1 => "Theme: Dark",
                        2 => "Theme: Light",
                        _ => "Theme: Auto",
                    };
                    let vol = if stats.vol_muted {
                        "Muted"
                    } else {
                        "Sound: On"
                    };
                    for (bx, lbl) in [(cx - 120.0, th), (cx, vol), (cx + 120.0, "Launcher")] {
                        let bg = if is_light {
                            [0, 0, 0, 16]
                        } else {
                            [255, 255, 255, 24]
                        };
                        UIFont::draw_rounded_rect_fill(
                            &mut self.overlay,
                            self.width,
                            self.height,
                            bx,
                            72.0,
                            52.0,
                            16.0,
                            11.0,
                            bg,
                        );
                        self.font.draw_text_centered(
                            &mut self.overlay,
                            self.width,
                            self.height,
                            lbl,
                            bx,
                            72.0,
                            11.5,
                            col_ink,
                            no_shadow,
                            FontWeight::SemiBold,
                        );
                    }
                }
                IslandPage::SystemOverview | IslandPage::Compact => {
                    let line = format!(
                        "CPU {}%   ·   RAM {:.1}/{:.1} GB   ·   Disk {}%",
                        stats.cpu_pct, stats.ram_used_gb, stats.ram_total_gb, stats.disk_pct
                    );
                    self.font.draw_text_centered(
                        &mut self.overlay,
                        self.width,
                        self.height,
                        &line,
                        cx,
                        82.0,
                        13.0,
                        col_ink,
                        no_shadow,
                        FontWeight::SemiBold,
                    );
                }
            }

            // Page switcher dots
            for (i, p) in [
                IslandPage::Notifications,
                IslandPage::ControlCenter,
                IslandPage::SystemOverview,
            ]
            .iter()
            .enumerate()
            {
                let dx = cx + (i as f32 - 1.0) * 48.0;
                let col = if self.page == *p { col_ink } else { col_dim };
                UIFont::draw_rounded_rect_fill(
                    &mut self.overlay,
                    self.width,
                    self.height,
                    dx,
                    144.0,
                    if self.page == *p { 10.0 } else { 4.0 },
                    3.0,
                    3.0,
                    col,
                );
            }
        }

        self.renderer
            .update_overlay(self.width, self.height, &self.overlay);
        let card = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::RoundedRectangle { radius: 22.0 })
            .bounds((cx - cur_w * 0.5) / sw, 0.0, cur_w / sw, cur_h / sh)
            .blur(self.blur_radius)
            .style_mode(eff_style)
            .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
            .element_type(27.0)
            .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);
        let _ = self.renderer.render(&[card]);
    }
}
