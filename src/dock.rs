//! Atlantis-Win Bottom Liquid Glass Dock.

use crate::win_os;
use glass_core::font::UIFont;
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu};
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::{Window, WindowBuilder, WindowLevel};

pub const DOCK_W: u32 = 460;
pub const DOCK_H: u32 = 84;

pub struct DockItem {
    pub icon: &'static str,
    pub target: &'static str,
}

pub struct DockWindow {
    pub window: Arc<Window>,
    pub renderer: GlassRenderer,
    pub overlay: Vec<u8>,
    pub width: u32,
    pub height: u32,
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
    pub hovered_idx: Option<usize>,
    pub items: Vec<DockItem>,
    pub needs_redraw: bool,
}

impl DockWindow {
    pub fn new<T>(
        elwt: &EventLoopWindowTarget<T>,
        gpu: &SharedGpu,
        screen_w: u32,
        screen_h: u32,
        wallpaper_buf: Option<&[u8]>,
        is_light: bool,
    ) -> Self {
        let pos_x = (screen_w as i32 - DOCK_W as i32) / 2;
        let pos_y = (screen_h as i32 - DOCK_H as i32 - 12).max(0);
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Atlantis Dock")
                .with_inner_size(LogicalSize::new(DOCK_W, DOCK_H))
                .with_position(LogicalPosition::new(pos_x, pos_y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .build(elwt)
                .expect("create dock window"),
        );
        let mut renderer = gpu
            .create_renderer(window.clone(), DOCK_W, DOCK_H)
            .expect("create dock renderer");
        if let Some(bg) = wallpaper_buf {
            let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                bg,
                pos_x.max(0) as u32,
                pos_y.max(0) as u32,
                DOCK_W,
                DOCK_H,
            );
            renderer.update_background(DOCK_W, DOCK_H, &crop);
        }

        Self {
            window,
            renderer,
            overlay: vec![0; (DOCK_W * DOCK_H * 4) as usize],
            width: DOCK_W,
            height: DOCK_H,
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
            hovered_idx: None,
            items: vec![
                DockItem { icon: "terminal", target: "wt.exe" },
                DockItem { icon: "browser", target: "msedge.exe" },
                DockItem { icon: "folder", target: "explorer.exe" },
                DockItem { icon: "editor", target: "code" },
                DockItem { icon: "launcher", target: "__launcher__" },
                DockItem { icon: "settings", target: "ms-settings:" },
            ],
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

    pub fn handle_event(&mut self, event: &WindowEvent) -> Option<&'static str> {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                self.cursor_pos = [px / self.width as f32, py / self.height as f32];
                let step = (self.width as f32 - 48.0) / self.items.len() as f32;
                let idx = ((px - 24.0) / step).floor() as isize;
                let next = if idx >= 0 && (idx as usize) < self.items.len() {
                    Some(idx as usize)
                } else {
                    None
                };
                if next != self.hovered_idx {
                    self.hovered_idx = next;
                    self.needs_redraw = true;
                }
                None
            }
            WindowEvent::CursorLeft { .. } => {
                self.hovered_idx = None;
                self.needs_redraw = true;
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(i) = self.hovered_idx {
                    let target = self.items[i].target;
                    if target == "__launcher__" {
                        return Some("launcher");
                    }
                    win_os::launch_app(target);
                }
                None
            }
            _ => None,
        }
    }

    pub fn draw(&mut self) {
        self.overlay.fill(0);
        let is_light = self.effective_is_light();
        let eff_style = self.effective_style_mode();
        let col = if is_light {
            if self.pure_bw { [0, 0, 0, 255] } else { [14, 18, 26, 255] }
        } else {
            if self.pure_bw { [255, 255, 255, 255] } else { [248, 250, 255, 255] }
        };
        let step = (self.width as f32 - 48.0) / self.items.len() as f32;
        let cy = self.height as f32 * 0.5;

        for (i, item) in self.items.iter().enumerate() {
            let cx = 24.0 + step * (i as f32 + 0.5);
            if self.hovered_idx == Some(i) {
                let bg = if is_light { [0, 0, 0, 22] } else { [255, 255, 255, 34] };
                UIFont::draw_rounded_rect_fill(&mut self.overlay, self.width, self.height, cx, cy, 22.0, 22.0, 12.0, bg);
            }
            match item.icon {
                "terminal" => UIFont::draw_terminal_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
                "browser" => UIFont::draw_browser_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
                "folder" => UIFont::draw_folder_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
                "editor" => UIFont::draw_editor_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
                "settings" => UIFont::draw_settings_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
                _ => UIFont::draw_cpu_icon(&mut self.overlay, self.width, self.height, cx, cy, col),
            }
        }

        self.renderer.update_overlay(self.width, self.height, &self.overlay);
        let card = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::Capsule)
            .bounds(0.02, 0.08, 0.96, 0.84)
            .blur(self.blur_radius)
            .style_mode(eff_style)
            .tint(0.0, 0.0, 1.0, if is_light { 1.0 } else { 0.0 })
            .element_type(27.0)
            .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);
        let _ = self.renderer.render(&[card]);
    }
}
