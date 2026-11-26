//! Atlantis-Win Top-Right Tasks & Session Widget.

use crate::win_os;
use glass_core::animation::{Spring, SpringParams};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowBuilder, WindowLevel};

pub const TODO_W: u32 = 360;
pub const TODO_H: u32 = 320;
const COMPACT_W: f32 = 152.0;
const COMPACT_H: f32 = 32.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

pub struct TodoWindow {
    pub window: Arc<Window>,
    pub renderer: GlassRenderer,
    pub font: UIFont,
    pub overlay: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub is_compact: bool,
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
    pub tasks: Vec<TodoItem>,
    pub needs_redraw: bool,
}

impl TodoWindow {
    pub fn new<T>(
        elwt: &EventLoopWindowTarget<T>,
        gpu: &SharedGpu,
        screen_w: u32,
        wallpaper_buf: Option<&[u8]>,
        is_light: bool,
    ) -> Self {
        let pos_x = (screen_w as i32 - TODO_W as i32 - 24).max(0);
        let pos_y = 8;
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Atlantis Tasks")
                .with_inner_size(LogicalSize::new(TODO_W, TODO_H))
                .with_position(LogicalPosition::new(pos_x, pos_y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .build(elwt)
                .expect("create todo window"),
        );
        let mut renderer = gpu
            .create_renderer(window.clone(), TODO_W, TODO_H)
            .expect("create todo renderer");
        if let Some(bg) = wallpaper_buf {
            let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                bg,
                pos_x as u32,
                pos_y as u32,
                TODO_W,
                TODO_H,
            );
            renderer.update_background(TODO_W, TODO_H, &crop);
        }

        let tasks = Self::load_tasks();
        Self {
            window,
            renderer,
            font: UIFont::new(),
            overlay: vec![0; (TODO_W * TODO_H * 4) as usize],
            width: TODO_W,
            height: TODO_H,
            is_compact: true,
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
                    stiffness: 260.0,
                    damping: 26.0,
                    mass: 1.0,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
            tasks,
            needs_redraw: true,
        }
    }

    fn load_tasks() -> Vec<TodoItem> {
        let p = win_os::data_dir().join("todos.json");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| {
                vec![
                    TodoItem {
                        text: "Review Atlantis Windows".into(),
                        done: false,
                    },
                    TodoItem {
                        text: "Press Super+Space for Launcher".into(),
                        done: false,
                    },
                ]
            })
    }

    fn save_tasks(&self) {
        let p = win_os::data_dir().join("todos.json");
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(&self.tasks) {
            let _ = std::fs::write(p, s);
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

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorEntered { .. } => {
                if self.is_compact {
                    self.is_compact = false;
                    self.size_spring.set_target(1.0);
                    self.needs_redraw = true;
                }
                true
            }
            WindowEvent::CursorLeft { .. } => {
                if !self.is_compact {
                    self.is_compact = true;
                    self.size_spring.set_target(0.0);
                    self.needs_redraw = true;
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
            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed
                    && ke.logical_key == Key::Named(NamedKey::Escape) =>
            {
                self.is_compact = true;
                self.size_spring.set_target(0.0);
                self.needs_redraw = true;
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.is_compact {
                    self.is_compact = false;
                    self.size_spring.set_target(1.0);
                    self.needs_redraw = true;
                    return true;
                }
                let py = self.cursor_pos[1] * self.height as f32;
                if py >= 56.0 {
                    let idx = ((py - 56.0) / 38.0).floor() as usize;
                    if let Some(t) = self.tasks.get_mut(idx) {
                        t.done = !t.done;
                        self.save_tasks();
                        self.needs_redraw = true;
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn draw(&mut self) {
        self.overlay.fill(0);
        let t = self.size_spring.value.clamp(0.0, 1.0);
        let is_light = self.effective_is_light();
        let eff_style = self.effective_style_mode();
        let sw = self.width as f32;
        let sh = self.height as f32;
        let col_p = if is_light {
            if self.pure_bw {
                [0, 0, 0, 255]
            } else {
                [10, 14, 22, 255]
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
                [50, 65, 85, 210]
            }
        } else {
            if self.pure_bw {
                [255, 255, 255, 220]
            } else {
                [205, 218, 238, 210]
            }
        };
        let no_shadow = [0, 0, 0, 0];

        if t <= 0.05 {
            let pending = self.tasks.iter().filter(|x| !x.done).count();
            let txt = format!("Tasks  ·  {}", pending);
            self.font.draw_text_centered(
                &mut self.overlay,
                self.width,
                self.height,
                &txt,
                sw - COMPACT_W * 0.5 - 8.0,
                16.0,
                12.0,
                col_p,
                no_shadow,
                FontWeight::SemiBold,
            );
            self.renderer
                .update_overlay(self.width, self.height, &self.overlay);
            let pill = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::Capsule)
                .bounds(
                    (sw - COMPACT_W - 8.0) / sw,
                    0.0,
                    COMPACT_W / sw,
                    COMPACT_H / sh,
                )
                .style_mode(eff_style)
                .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
                .element_type(27.0);
            let _ = self.renderer.render(&[pill]);
            return;
        }

        self.font.draw_text(
            &mut self.overlay,
            self.width,
            self.height,
            "TASKS",
            28.0,
            32.0,
            11.0,
            col_dim,
            FontWeight::Bold,
        );
        for (i, item) in self.tasks.iter().take(6).enumerate() {
            let ry = 74.0 + i as f32 * 38.0;
            let mark = if item.done { "✓" } else { "○" };
            self.font.draw_text_centered_y(
                &mut self.overlay,
                self.width,
                self.height,
                mark,
                28.0,
                ry,
                13.0,
                col_p,
                no_shadow,
                FontWeight::Bold,
            );
            self.font.draw_text_centered_y(
                &mut self.overlay,
                self.width,
                self.height,
                &item.text,
                50.0,
                ry,
                12.5,
                if item.done { col_dim } else { col_p },
                no_shadow,
                FontWeight::Medium,
            );
        }
        self.renderer
            .update_overlay(self.width, self.height, &self.overlay);
        let card = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::RoundedRectangle { radius: 22.0 })
            .bounds(
                8.0 / sw,
                0.0,
                (sw - 16.0) / sw,
                ((64.0 + (sh - 64.0) * t) / sh).min(1.0),
            )
            .blur(self.blur_radius)
            .style_mode(eff_style)
            .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
            .element_type(27.0);
        let _ = self.renderer.render(&[card]);
    }
}
