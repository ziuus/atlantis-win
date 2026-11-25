//! Atlantis for Windows — Physical Optical Liquid Glass Dynamic Island & Desktop Shell.
//! Powered by `liquid-glass` (glass-core + glass-platform) and `winit`.

use chrono::{Local, Timelike};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::{GlassSurface, Material, SharedGpu, Tokens};
use glass_platform::WallpaperManager;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::System;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{WindowBuilder, WindowLevel};

const WIN_W: u32 = 480;
const WIN_H: u32 = 160;
const PILL_W: f32 = 190.0;
const PILL_H: f32 = 38.0;
const EXP_W: f32 = 460.0;
const EXP_H: f32 = 148.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Compact,
    ControlCenter,
    SystemOverview,
}

struct WinTelemetry {
    sys: System,
    cpu_pct: u32,
    ram_used_gb: f32,
    ram_total_gb: f32,
    last_poll: Instant,
}

impl WinTelemetry {
    fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys,
            cpu_pct: 0,
            ram_used_gb: 0.0,
            ram_total_gb: 1.0,
            last_poll: Instant::now() - Duration::from_secs(5),
        }
    }

    fn poll(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(1200) {
            return;
        }
        self.last_poll = Instant::now();
        self.sys.refresh_cpu();
        self.sys.refresh_memory();
        self.cpu_pct = self.sys.global_cpu_info().cpu_usage().round() as u32;
        self.ram_used_gb = self.sys.used_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
        self.ram_total_gb = (self.sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0).max(0.1);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_overlay(
    buf: &mut [u8],
    font: &UIFont,
    w: u32,
    h: u32,
    cur_w: f32,
    cur_h: f32,
    page: Page,
    telemetry: &WinTelemetry,
    is_light: bool,
) {
    buf.fill(0);
    let cx = (w as f32) * 0.5;
    let cy = cur_h * 0.5 + 4.0;
    let col_ink = if is_light {
        [10, 14, 24, 255]
    } else {
        Tokens::TEXT_PRIMARY
    };
    let col_dim = if is_light {
        [45, 60, 80, 190]
    } else {
        [210, 222, 240, 180]
    };
    let no_shadow = [0, 0, 0, 0];

    match page {
        Page::Compact => {
            let now = Local::now();
            let (_pm, hour) = now.hour12();
            let ampm = if now.hour() >= 12 { "PM" } else { "AM" };
            let time_s = format!("{}:{:02} {}", hour, now.minute(), ampm);
            font.draw_text_centered_y(
                buf,
                w,
                h,
                &time_s,
                cx - cur_w * 0.5 + 18.0,
                cy,
                12.5,
                col_ink,
                no_shadow,
                FontWeight::SemiBold,
            );
            let cpu_s = format!("CPU {}%", telemetry.cpu_pct);
            let cpu_w = font.measure_text(&cpu_s, 11.5, FontWeight::Medium);
            font.draw_text_centered_y(
                buf,
                w,
                h,
                &cpu_s,
                cx + cur_w * 0.5 - 18.0 - cpu_w,
                cy,
                11.5,
                col_dim,
                no_shadow,
                FontWeight::Medium,
            );
        }
        Page::ControlCenter => {
            font.draw_text(
                buf,
                w,
                h,
                "ATLANTIS · WINDOWS",
                cx - cur_w * 0.5 + 28.0,
                24.0,
                10.5,
                col_dim,
                FontWeight::SemiBold,
            );
            let hint = "Scroll or click to cycle · Right-click to collapse";
            font.draw_text_centered(
                buf,
                w,
                h,
                hint,
                cx,
                84.0,
                12.5,
                col_ink,
                no_shadow,
                FontWeight::Medium,
            );
        }
        Page::SystemOverview => {
            font.draw_text(
                buf,
                w,
                h,
                "SYSTEM TELEMETRY",
                cx - cur_w * 0.5 + 28.0,
                24.0,
                10.5,
                col_dim,
                FontWeight::SemiBold,
            );
            let stats = [
                format!("CPU {}%", telemetry.cpu_pct),
                format!(
                    "RAM {:.1}/{:.0}G",
                    telemetry.ram_used_gb, telemetry.ram_total_gb
                ),
            ];
            for (i, s) in stats.iter().enumerate() {
                let tx = cx - 95.0 + (i as f32) * 190.0;
                font.draw_text_centered(
                    buf,
                    w,
                    h,
                    s,
                    tx,
                    84.0,
                    15.0,
                    col_ink,
                    no_shadow,
                    FontWeight::SemiBold,
                );
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create EventLoop");
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Atlantis Windows Island")
            .with_inner_size(LogicalSize::new(WIN_W, WIN_H))
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .build(&event_loop)
            .expect("Failed to create overlay window"),
    );

    if let Some(mon) = window.current_monitor() {
        let sz = mon.size();
        let x = (sz.width as i32 - WIN_W as i32) / 2;
        window.set_outer_position(LogicalPosition::new(x.max(0), 6));
    }

    let gpu = pollster::block_on(SharedGpu::new()).expect("Failed to initialize WGPU");
    let mut renderer = gpu
        .create_renderer(window.clone(), WIN_W, WIN_H)
        .expect("Failed to create GlassRenderer");

    let wp_mgr = WallpaperManager::new();
    let mut is_light = false;
    if let Some(cache) = wp_mgr.cached_buffer() {
        let crop = WallpaperManager::get_wallpaper_crop(cache, 720, 0, WIN_W, WIN_H);
        is_light = WallpaperManager::is_buffer_light_hysteresis(&crop, false);
        renderer.update_background(WIN_W, WIN_H, &crop);
    }

    let font = UIFont::new();
    let mut overlay = vec![0u8; (WIN_W * WIN_H * 4) as usize];
    let mut telemetry = WinTelemetry::new();
    let mut page = Page::Compact;
    let mut cur_w = PILL_W;
    let mut cur_h = PILL_H;

    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button,
                        ..
                    },
                ..
            } => {
                page = match (button, page) {
                    (MouseButton::Right, _) => Page::Compact,
                    (_, Page::Compact) => Page::SystemOverview,
                    (_, Page::SystemOverview) => Page::ControlCenter,
                    (_, Page::ControlCenter) => Page::Compact,
                };
                window.request_redraw();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        ..
                    },
                ..
            } if y.abs() > 0.1 => {
                page = match page {
                    Page::Compact => Page::SystemOverview,
                    Page::SystemOverview => Page::ControlCenter,
                    Page::ControlCenter => Page::Compact,
                };
                window.request_redraw();
            }
            Event::AboutToWait => {
                telemetry.poll();
                let (tw, th) = match page {
                    Page::Compact => (PILL_W, PILL_H),
                    _ => (EXP_W, EXP_H),
                };
                cur_w += (tw - cur_w) * 0.22;
                cur_h += (th - cur_h) * 0.22;
                window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                render_overlay(
                    &mut overlay,
                    &font,
                    WIN_W,
                    WIN_H,
                    cur_w,
                    cur_h,
                    page,
                    &telemetry,
                    is_light,
                );
                renderer.update_overlay(WIN_W, WIN_H, &overlay);
                let w_norm = cur_w / (WIN_W as f32);
                let h_norm = cur_h / (WIN_H as f32);
                let shape = if page == Page::Compact {
                    Shape::Capsule
                } else {
                    Shape::RoundedRectangle {
                        radius: Tokens::RADIUS_OUTER,
                    }
                };
                let surface = GlassSurface::new()
                    .material(Material::LiquidGlass)
                    .shape(shape)
                    .bounds(0.5 - w_norm * 0.5, 4.0 / (WIN_H as f32), w_norm, h_norm)
                    .bevel(16.0)
                    .refraction(18.0)
                    .dispersion(0.12)
                    .specular(1.15)
                    .fresnel(0.60)
                    .style_mode(2.0)
                    .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
                    .element_type(27.0);
                let _ = renderer.render(&[surface]);
            }
            _ => {}
        }
    });
}
