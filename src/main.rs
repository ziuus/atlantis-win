//! Atlantis for Windows — Full Multi-Widget Liquid Glass Desktop Shell.
//! Parity shell with Linux Atlantis: Dynamic Island, 8-Box Control Hub, Tasks, Dock, and 2-Card Launcher.

use glass_core::font::UIFont;
use glass_core::SharedGpu;
use glass_platform::WallpaperManager;
use std::time::{Duration, Instant};
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};

mod clock;
mod dock;
mod island;
mod launcher;
mod telemetry;
mod todo;
mod win_os;

use clock::ClockWindow;
use dock::DockWindow;
use island::{IslandAction, IslandWindow};
use launcher::LauncherWindow;
use telemetry::SysStats;
use todo::TodoWindow;

fn parse_color(val: &serde_json::Value) -> Option<[u8; 3]> {
    if let Some(arr) = val.as_array() {
        if arr.len() >= 3 {
            return Some([
                arr[0].as_u64()? as u8,
                arr[1].as_u64()? as u8,
                arr[2].as_u64()? as u8,
            ]);
        }
    }
    if let Some(s) = val.as_str() {
        let s = s.trim_start_matches('#');
        if s.len() == 6 {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            return Some([r, g, b]);
        }
    }
    None
}

fn main() {
    UIFont::set_global_scale(1.12);
    let event_loop = EventLoop::new().expect("create winit event loop");
    let gpu = pollster::block_on(SharedGpu::new()).expect("init SharedGpu");

    let (screen_w, screen_h) = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map(|m| {
            let s = m.size();
            (s.width.max(1280), s.height.max(720))
        })
        .unwrap_or((1920, 1080));

    let mut wallpaper_mgr = WallpaperManager::new();
    let _ = wallpaper_mgr.check_update();
    let wp_buf = wallpaper_mgr.cached_buffer().map(|v| v.to_vec());
    let is_light = wp_buf
        .as_deref()
        .map(|b| WallpaperManager::is_buffer_light_hysteresis(b, false))
        .unwrap_or(false);
    wallpaper_mgr.release_full_buffer();

    let mut stats = SysStats::new();
    let mut island = IslandWindow::new(&event_loop, &gpu, screen_w, wp_buf.as_deref(), is_light);
    let mut clock = ClockWindow::new(&event_loop, &gpu, wp_buf.as_deref(), is_light);
    let mut todo = TodoWindow::new(&event_loop, &gpu, screen_w, wp_buf.as_deref(), is_light);
    let mut dock = DockWindow::new(&event_loop, &gpu, screen_w, screen_h, wp_buf.as_deref(), is_light);
    let mut launcher = LauncherWindow::new(
        &event_loop,
        &gpu,
        screen_w,
        screen_h,
        wp_buf.as_deref(),
        is_light,
    );

    let cfg_path = win_os::data_dir().join("config.json");
    if let Ok(content) = std::fs::read_to_string(&cfg_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(tm) = val.get("theme_mode").and_then(|v| v.as_u64()) {
                let t = tm as u32;
                island.theme_mode = t;
                clock.theme_mode = t;
                todo.theme_mode = t;
                dock.theme_mode = t;
                launcher.theme_mode = t;
            }
            if let Some(sm) = val.get("style_mode").and_then(|v| v.as_f64()) {
                let s = sm as f32;
                island.style_mode = s;
                clock.style_mode = s;
                todo.style_mode = s;
                dock.style_mode = s;
                launcher.style_mode = s;
            }
            if let Some(ct) = val.get("color_theme").and_then(|v| v.as_str()) {
                let (bw, ac) = match ct.to_lowercase().as_str() {
                    "bw" | "black_white" | "monochrome" | "default" => (true, [0, 0, 0]),
                    "slate" | "glass" => (false, [18, 108, 224]),
                    "ocean" | "blue" => (false, [30, 144, 255]),
                    "purple" | "violet" => (false, [140, 108, 255]),
                    "emerald" | "green" => (false, [34, 197, 94]),
                    "crimson" | "red" => (false, [239, 68, 68]),
                    "amber" | "gold" => (false, [245, 158, 11]),
                    _ => (true, [0, 0, 0]),
                };
                island.pure_bw = bw;
                clock.pure_bw = bw;
                todo.pure_bw = bw;
                dock.pure_bw = bw;
                launcher.pure_bw = bw;
                island.accent_color = ac;
                clock.accent_color = ac;
                todo.accent_color = ac;
                dock.accent_color = ac;
                launcher.accent_color = ac;
            }
            if let Some(ac) = val.get("accent_color").and_then(parse_color) {
                island.accent_color = ac;
                clock.accent_color = ac;
                todo.accent_color = ac;
                dock.accent_color = ac;
                launcher.accent_color = ac;
            }
            if let Some(bw) = val.get("pure_bw").and_then(|v| v.as_bool()) {
                island.pure_bw = bw;
                clock.pure_bw = bw;
                todo.pure_bw = bw;
                dock.pure_bw = bw;
                launcher.pure_bw = bw;
            }
        }
    }

    let mut last_frame = Instant::now();
    let mut last_overlap_check = Instant::now() - Duration::from_secs(10);

    let _ = event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { window_id, event } => {
            if let WindowEvent::CloseRequested = event {
                elwt.exit();
                return;
            }
            if let WindowEvent::KeyboardInput { event: ref ke, .. } = event {
                if ke.state == ElementState::Pressed
                    && ke.logical_key == Key::Named(NamedKey::F2)
                {
                    launcher.toggle();
                    return;
                }
            }
            if window_id == launcher.window.id() {
                launcher.handle_event(&event);
            } else if window_id == island.window.id() {
                match island.handle_event(&event, &mut stats) {
                    IslandAction::ToggleEco(ps) => {
                        clock.power_save = ps;
                        todo.power_save = ps;
                        dock.power_save = ps;
                        launcher.power_save = ps;
                        island.renderer.set_power_save(ps);
                        clock.renderer.set_power_save(ps);
                        todo.renderer.set_power_save(ps);
                        dock.renderer.set_power_save(ps);
                        launcher.renderer.set_power_save(ps);
                        island.needs_redraw = true;
                        clock.needs_redraw = true;
                        todo.needs_redraw = true;
                        dock.needs_redraw = true;
                    }
                    IslandAction::ToggleAppBg(ab) => {
                        clock.app_backdrop_solid = ab;
                        todo.app_backdrop_solid = ab;
                        dock.app_backdrop_solid = ab;
                        launcher.app_backdrop_solid = ab;
                        clock.needs_redraw = true;
                        todo.needs_redraw = true;
                        dock.needs_redraw = true;
                    }
                    IslandAction::CycleTheme(tm) => {
                        clock.theme_mode = tm;
                        todo.theme_mode = tm;
                        dock.theme_mode = tm;
                        launcher.theme_mode = tm;
                        clock.needs_redraw = true;
                        todo.needs_redraw = true;
                        dock.needs_redraw = true;
                    }
                    IslandAction::ToggleLauncher => launcher.toggle(),
                    IslandAction::None => {}
                }
            } else if window_id == clock.window.id() {
                clock.handle_event(&event, &mut stats);
            } else if window_id == todo.window.id() {
                todo.handle_event(&event);
            } else if window_id == dock.window.id() {
                if let Some("launcher") = dock.handle_event(&event) {
                    launcher.toggle();
                }
            }
        }
        Event::AboutToWait => {
            let now = Instant::now();
            let dt = (now - last_frame).as_secs_f32().clamp(0.001, 0.05);
            last_frame = now;

            stats.poll(island.power_save);

            if last_overlap_check.elapsed() >= Duration::from_secs(2) {
                last_overlap_check = now;
                let ov_i = win_os::is_foreground_overlapping(island.pos_x, island.pos_y, island.width as i32, island.height as i32);
                if ov_i != island.is_window_overlapping {
                    island.is_window_overlapping = ov_i;
                    island.needs_redraw = true;
                }
                let ov_c = win_os::is_foreground_overlapping(clock.pos_x, clock.pos_y, clock.width as i32, clock.height as i32);
                if ov_c != clock.is_window_overlapping {
                    clock.is_window_overlapping = ov_c;
                    clock.needs_redraw = true;
                }
            }

            let a_i = island.update_physics(dt);
            let a_c = clock.update_physics(dt);
            let a_t = todo.update_physics(dt);
            let a_l = launcher.update_physics(dt);

            if a_i || island.needs_redraw {
                island.draw(&stats);
                island.needs_redraw = false;
            }
            if a_c || clock.needs_redraw {
                clock.draw(&stats);
                clock.needs_redraw = false;
            }
            if a_t || todo.needs_redraw {
                todo.draw();
                todo.needs_redraw = false;
            }
            if dock.needs_redraw {
                dock.draw();
                dock.needs_redraw = false;
            }
            if a_l || launcher.needs_redraw {
                launcher.draw();
                launcher.needs_redraw = false;
            }

            let animating = a_i || a_c || a_t || a_l;
            if animating {
                elwt.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
            } else {
                let idle_ms = if island.power_save { 3000 } else { 1000 };
                elwt.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(idle_ms)));
            }
        }
        _ => {}
    });
}
