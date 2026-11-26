//! Atlantis-Win 2-Card Spatial Liquid Glass Launcher.

use crate::win_os::{self, WinApp};
use glass_core::animation::{Spring, SpringParams};
use glass_core::font::{FontWeight, UIFont};
use glass_core::geometry::Shape;
use glass_core::renderer::GlassRenderer;
use glass_core::{GlassSurface, Material, SharedGpu};
use std::collections::HashMap;
use std::sync::Arc;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowBuilder, WindowLevel};

pub const LAUNCHER_W: u32 = 680;
pub const LAUNCHER_H: u32 = 490;
const SEARCH_CARD_H: f32 = 64.0;
const MAIN_CARD_Y: f32 = 76.0;
const LIST_TOP_Y: f32 = 94.0;
const ROW_H: f32 = 56.0;
const DIVIDER_X: f32 = 414.0;
const MAX_VISIBLE_ROWS: usize = 6;

#[derive(Debug, Clone)]
enum SearchResult {
    Calc { expr: String, value: String },
    App(WinApp),
}

pub struct LauncherWindow {
    pub window: Arc<Window>,
    pub renderer: GlassRenderer,
    pub font: UIFont,
    pub overlay: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,

    pub is_open: bool,
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

    pub open_spring: Spring,
    pub panel_spring: Spring,
    pub query: String,
    pub selected_idx: usize,
    pub scroll_offset: usize,
    pub apps: Vec<WinApp>,
    pub freq: HashMap<String, u32>,
    filtered: Vec<SearchResult>,
    pub needs_redraw: bool,
}

impl LauncherWindow {
    pub fn new<T>(
        elwt: &EventLoopWindowTarget<T>,
        gpu: &SharedGpu,
        screen_w: u32,
        screen_h: u32,
        wallpaper_buf: Option<&[u8]>,
        is_light: bool,
    ) -> Self {
        let pos_x = (screen_w as i32 - LAUNCHER_W as i32) / 2;
        let pos_y = ((screen_h as i32 - LAUNCHER_H as i32) / 3).max(80);
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("Atlantis Launcher")
                .with_inner_size(LogicalSize::new(LAUNCHER_W, LAUNCHER_H))
                .with_position(LogicalPosition::new(pos_x, pos_y))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(false)
                .build(elwt)
                .expect("create launcher window"),
        );
        let mut renderer = gpu
            .create_renderer(window.clone(), LAUNCHER_W, LAUNCHER_H)
            .expect("create launcher renderer");
        if let Some(bg) = wallpaper_buf {
            let crop = glass_platform::WallpaperManager::get_wallpaper_crop(
                bg,
                pos_x.max(0) as u32,
                pos_y.max(0) as u32,
                LAUNCHER_W,
                LAUNCHER_H,
            );
            renderer.update_background(LAUNCHER_W, LAUNCHER_H, &crop);
        }

        let mut w = Self {
            window,
            renderer,
            font: UIFont::new(),
            overlay: vec![0; (LAUNCHER_W * LAUNCHER_H * 4) as usize],
            width: LAUNCHER_W,
            height: LAUNCHER_H,
            pos_x,
            pos_y,
            is_open: false,
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
            open_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 380.0,
                    damping: 33.0,
                    mass: 0.75,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
            panel_spring: Spring::with_params(
                0.0,
                SpringParams {
                    stiffness: 240.0,
                    damping: 30.0,
                    mass: 1.05,
                    initial_velocity: 0.0,
                    epsilon: 0.002,
                },
            ),
            query: String::new(),
            selected_idx: 0,
            scroll_offset: 0,
            apps: win_os::scan_apps(),
            freq: Self::load_freq(),
            filtered: Vec::new(),
            needs_redraw: false,
        };
        w.refilter();
        w
    }

    fn freq_path() -> std::path::PathBuf {
        win_os::data_dir().join("launcher_freq.json")
    }

    fn load_freq() -> HashMap<String, u32> {
        std::fs::read_to_string(Self::freq_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_freq(&self) {
        let path = Self::freq_path();
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        if let Ok(json) = serde_json::to_string(&self.freq) {
            let _ = std::fs::write(path, json);
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

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.is_window_overlapping = win_os::is_foreground_overlapping(
            self.pos_x,
            self.pos_y,
            self.width as i32,
            self.height as i32,
        );
        self.query.clear();
        self.selected_idx = 0;
        self.scroll_offset = 0;
        if self.apps.is_empty() {
            self.apps = win_os::scan_apps();
        }
        self.refilter();
        self.open_spring.set_target(1.0);
        self.panel_spring.set_target(1.0);
        self.window.set_visible(true);
        self.window.focus_window();
        self.needs_redraw = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.open_spring.set_target(0.0);
        self.panel_spring.set_target(0.0);
        self.needs_redraw = true;
    }

    pub fn update_physics(&mut self, dt: f32) -> bool {
        if self.power_save {
            let changed = !self.open_spring.is_at_rest() || !self.panel_spring.is_at_rest();
            self.open_spring.snap_to_target();
            self.panel_spring.snap_to_target();
            if changed {
                self.needs_redraw = true;
            }
            if !self.is_open {
                self.window.set_visible(false);
            }
            return false;
        }
        let eff_dt = dt.clamp(0.0, 0.05);
        self.open_spring.update(eff_dt);
        self.panel_spring.update(eff_dt);
        if (self.open_spring.value - self.open_spring.target).abs() < 0.002
            && self.open_spring.velocity.abs() < 0.01
        {
            self.open_spring.snap_to_target();
        }
        if (self.panel_spring.value - self.panel_spring.target).abs() < 0.002
            && self.panel_spring.velocity.abs() < 0.01
        {
            self.panel_spring.snap_to_target();
        }
        if !self.is_open && self.open_spring.is_at_rest() && self.panel_spring.is_at_rest() {
            self.window.set_visible(false);
        }
        !self.open_spring.is_at_rest() || !self.panel_spring.is_at_rest()
    }

    fn system_actions() -> Vec<WinApp> {
        vec![
            WinApp {
                name: "Lock Screen".into(),
                target: "__lock__".into(),
                comment: "Lock Windows session".into(),
                is_action: true,
            },
            WinApp {
                name: "Sleep".into(),
                target: "__sleep__".into(),
                comment: "Put PC to sleep".into(),
                is_action: true,
            },
            WinApp {
                name: "Reboot".into(),
                target: "__reboot__".into(),
                comment: "Restart Windows".into(),
                is_action: true,
            },
            WinApp {
                name: "Power Off".into(),
                target: "__poweroff__".into(),
                comment: "Shut down PC".into(),
                is_action: true,
            },
        ]
    }

    fn eval_math(expr: &str) -> Option<String> {
        let clean: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
        if clean.is_empty()
            || !clean
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '*' | '/' | '.'))
            || !clean.chars().any(|c| matches!(c, '+' | '-' | '*' | '/'))
        {
            return None;
        }
        let val = eval_expr_add_sub(&clean)?;
        if val.is_finite() {
            if (val - val.round()).abs() < 1e-6 {
                Some(format!("{}", val.round() as i64))
            } else {
                Some(
                    format!("{:.4}", val)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string(),
                )
            }
        } else {
            None
        }
    }

    fn refilter(&mut self) {
        self.filtered.clear();
        let q = self.query.trim().to_lowercase();
        if let Some(ans) = Self::eval_math(&self.query) {
            self.filtered.push(SearchResult::Calc {
                expr: self.query.trim().to_string(),
                value: ans,
            });
        }
        if let Some(cmd_q) = q.strip_prefix('>') {
            let cq = cmd_q.trim();
            for act in Self::system_actions() {
                if cq.is_empty() || act.name.to_lowercase().contains(cq) {
                    self.filtered.push(SearchResult::App(act));
                }
            }
        } else {
            let mut scored: Vec<(i32, &WinApp)> = self
                .apps
                .iter()
                .filter_map(|app| {
                    let nl = app.name.to_lowercase();
                    let fb = (*self.freq.get(&app.name).unwrap_or(&0) as i32) * 10;
                    if q.is_empty() {
                        Some((fb, app))
                    } else if nl.starts_with(&q) {
                        Some((1000 + fb, app))
                    } else if nl.contains(&q) {
                        Some((500 + fb, app))
                    } else {
                        None
                    }
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
            for (_, app) in scored.into_iter().take(32) {
                self.filtered.push(SearchResult::App(app.clone()));
            }
        }
        if self.selected_idx >= self.filtered.len() {
            self.selected_idx = 0;
        }
        self.scroll_offset = 0;
    }

    fn activate_selected(&mut self) {
        let Some(item) = self.filtered.get(self.selected_idx).cloned() else {
            return;
        };
        match item {
            SearchResult::Calc { value, .. } => {
                win_os::copy_to_clipboard(&value);
                self.close();
            }
            SearchResult::App(app) => {
                *self.freq.entry(app.name.clone()).or_insert(0) += 1;
                self.save_freq();
                match app.target.as_str() {
                    "__lock__" => win_os::lock_screen(),
                    "__sleep__" => win_os::suspend(),
                    "__reboot__" => win_os::reboot(),
                    "__poweroff__" => win_os::poweroff(),
                    t => win_os::launch_app(t),
                }
                self.close();
            }
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        if !self.is_open {
            return false;
        }
        match event {
            WindowEvent::KeyboardInput { event: ke, .. } if ke.state == ElementState::Pressed => {
                match &ke.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.close();
                        true
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.activate_selected();
                        true
                    }
                    Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::Tab) => {
                        if !self.filtered.is_empty() {
                            self.selected_idx = (self.selected_idx + 1) % self.filtered.len();
                            if self.selected_idx >= self.scroll_offset + MAX_VISIBLE_ROWS {
                                self.scroll_offset = self.selected_idx - MAX_VISIBLE_ROWS + 1;
                            } else if self.selected_idx == 0 {
                                self.scroll_offset = 0;
                            }
                        }
                        self.needs_redraw = true;
                        true
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if !self.filtered.is_empty() {
                            self.selected_idx = if self.selected_idx == 0 {
                                self.filtered.len() - 1
                            } else {
                                self.selected_idx - 1
                            };
                            if self.selected_idx < self.scroll_offset {
                                self.scroll_offset = self.selected_idx;
                            } else if self.selected_idx >= self.scroll_offset + MAX_VISIBLE_ROWS {
                                self.scroll_offset = self.selected_idx - MAX_VISIBLE_ROWS + 1;
                            }
                        }
                        self.needs_redraw = true;
                        true
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.query.pop();
                        self.refilter();
                        self.needs_redraw = true;
                        true
                    }
                    _ => {
                        if let Some(txt) = &ke.text {
                            let mut pushed = false;
                            for ch in txt.chars() {
                                if !ch.is_control() {
                                    self.query.push(ch);
                                    pushed = true;
                                }
                            }
                            if pushed {
                                self.refilter();
                                self.needs_redraw = true;
                            }
                            pushed
                        } else {
                            false
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let lx = position.x as f32;
                let ly = position.y as f32;
                self.cursor_pos = [lx / self.width as f32, ly / self.height as f32];
                if lx < DIVIDER_X && ly >= LIST_TOP_Y {
                    let rel = ((ly - LIST_TOP_Y) / ROW_H).floor() as usize;
                    let idx = self.scroll_offset + rel;
                    if rel < MAX_VISIBLE_ROWS && idx < self.filtered.len() {
                        self.selected_idx = idx;
                        self.needs_redraw = true;
                    }
                }
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let lx = self.cursor_pos[0] * self.width as f32;
                let ly = self.cursor_pos[1] * self.height as f32;
                if lx < DIVIDER_X && ly >= LIST_TOP_Y {
                    let rel = ((ly - LIST_TOP_Y) / ROW_H).floor() as usize;
                    let idx = self.scroll_offset + rel;
                    if rel < MAX_VISIBLE_ROWS && idx < self.filtered.len() {
                        self.selected_idx = idx;
                        self.activate_selected();
                        return true;
                    }
                } else if (448.0..=644.0).contains(&lx) && (396.0..=432.0).contains(&ly) {
                    self.activate_selected();
                    return true;
                }
                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 24.0,
                };
                if dy < -0.2 && self.selected_idx + 1 < self.filtered.len() {
                    self.selected_idx += 1;
                    if self.selected_idx >= self.scroll_offset + MAX_VISIBLE_ROWS {
                        self.scroll_offset = self.selected_idx - MAX_VISIBLE_ROWS + 1;
                    }
                    self.needs_redraw = true;
                } else if dy > 0.2 && self.selected_idx > 0 {
                    self.selected_idx -= 1;
                    if self.selected_idx < self.scroll_offset {
                        self.scroll_offset = self.selected_idx;
                    }
                    self.needs_redraw = true;
                }
                true
            }
            _ => false,
        }
    }

    pub fn draw(&mut self) {
        let t_search = self.open_spring.value.clamp(0.0, 1.0);
        let t_panel = ((self.panel_spring.value - 0.12) / 0.88).clamp(0.0, 1.0);
        if !self.is_open && t_search <= 0.01 && t_panel <= 0.01 {
            self.overlay.fill(0);
            self.renderer
                .update_overlay(self.width, self.height, &self.overlay);
            let _ = self.renderer.render(&[]);
            return;
        }

        self.render_overlay(t_search);
        self.renderer
            .update_overlay(self.width, self.height, &self.overlay);

        let is_light = if self.effective_is_light() { 1.0 } else { 0.0 };
        let eff_style = self.effective_style_mode();
        let h_total = self.height as f32;

        let m_search = (1.0 - t_search).clamp(0.0, 1.0);
        let w_search = (0.42 + 0.58 * t_search).clamp(0.25, 1.0);
        let x_search = (0.5 - w_search * 0.5).clamp(0.0, 0.75);
        let y_search = (m_search * 0.035).clamp(0.0, 0.8);
        let h_search = ((SEARCH_CARD_H / h_total) * (0.76 + 0.24 * t_search)).clamp(0.04, 1.0);

        let search_card = GlassSurface::new()
            .material(Material::LiquidGlass)
            .shape(Shape::RoundedRectangle { radius: 20.0 })
            .bounds(x_search, y_search, w_search.min(1.0 - x_search), h_search)
            .curvature(18.0)
            .bevel(14.0 + m_search * 6.0)
            .refraction(18.0 + m_search * 16.0)
            .dispersion(0.14)
            .blur(self.blur_radius)
            .specular(1.25)
            .fresnel(0.65)
            .style_mode(eff_style)
            .tint(0.0, 0.0, t_search, is_light)
            .element_type(27.0)
            .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);

        if t_panel > 0.015 {
            let m_panel = (1.0 - t_panel).clamp(0.0, 1.0);
            let w_panel = (0.84 + 0.16 * t_panel).clamp(0.3, 1.0);
            let x_panel = (0.5 - w_panel * 0.5).clamp(0.0, 0.7);
            let main_y = ((MAIN_CARD_Y / h_total) - m_panel * 0.045).clamp(0.0, 0.85);
            let full_h = (h_total - MAIN_CARD_Y) / h_total;
            let main_h = (full_h * (0.18 + 0.82 * t_panel)).min(1.0 - main_y);
            let main_card = GlassSurface::new()
                .material(Material::LiquidGlass)
                .shape(Shape::RoundedRectangle { radius: 24.0 })
                .bounds(x_panel, main_y, w_panel.min(1.0 - x_panel), main_h)
                .curvature(20.0)
                .bevel(16.0 + m_panel * 8.0)
                .refraction(20.0 + m_panel * 18.0)
                .dispersion(0.14)
                .blur(self.blur_radius)
                .specular(1.25)
                .fresnel(0.65)
                .style_mode(eff_style)
                .tint(0.0, 0.0, t_panel, is_light)
                .element_type(27.0)
                .pointer_pos(self.cursor_pos[0], self.cursor_pos[1]);
            let _ = self.renderer.render(&[search_card, main_card]);
        } else {
            let _ = self.renderer.render(&[search_card]);
        }
    }

    fn render_overlay(&mut self, t: f32) {
        self.overlay.fill(0);
        if t <= 0.02 {
            return;
        }
        let is_light = self.effective_is_light();
        let buf = &mut self.overlay;
        let a = (255.0 * t) as u8;
        let col_ink = if is_light {
            if self.pure_bw {
                [0, 0, 0, a]
            } else {
                [10, 14, 24, a]
            }
        } else {
            if self.pure_bw {
                [255, 255, 255, a]
            } else {
                [245, 247, 252, a]
            }
        };
        let col_dim = if is_light {
            if self.pure_bw {
                [0, 0, 0, (175.0 * t) as u8]
            } else {
                [42, 56, 76, (175.0 * t) as u8]
            }
        } else {
            if self.pure_bw {
                [255, 255, 255, (175.0 * t) as u8]
            } else {
                [205, 218, 238, (175.0 * t) as u8]
            }
        };
        let ac = self.accent_color;
        let col_acc = if self.pure_bw {
            if is_light {
                [0, 0, 0, (220.0 * t) as u8]
            } else if ac == [0, 0, 0] {
                [255, 255, 255, (230.0 * t) as u8]
            } else {
                [ac[0], ac[1], ac[2], (230.0 * t) as u8]
            }
        } else if is_light {
            [ac[0], ac[1], ac[2], (220.0 * t) as u8]
        } else {
            let lr = (ac[0] as u16 + 97).min(255) as u8;
            let lg = (ac[1] as u16 + 82).min(255) as u8;
            let lb = (ac[2] as u16 + 31).min(255) as u8;
            [lr, lg, lb, (230.0 * t) as u8]
        };
        let pill_bg = if is_light {
            [0, 0, 0, (16.0 * t) as u8]
        } else {
            [255, 255, 255, (24.0 * t) as u8]
        };
        let no_shadow = [0, 0, 0, 0];

        let scy = SEARCH_CARD_H * 0.5;
        UIFont::draw_rounded_rect_stroke(
            buf,
            self.width,
            self.height,
            28.5,
            scy - 1.5,
            5.5,
            5.5,
            5.5,
            1.5,
            col_acc,
        );
        UIFont::draw_segment(
            buf,
            self.width,
            self.height,
            (32.8, scy + 2.8),
            (37.2, scy + 7.2),
            1.6,
            col_acc,
        );

        let disp_q = if self.query.is_empty() {
            "Search apps, > commands, or math..."
        } else {
            &self.query
        };
        self.font.draw_text_centered_y(
            buf,
            self.width,
            self.height,
            disp_q,
            52.0,
            scy,
            16.5,
            if self.query.is_empty() {
                col_dim
            } else {
                col_ink
            },
            no_shadow,
            FontWeight::Medium,
        );

        let badge = format!("Apps · {}", self.filtered.len());
        UIFont::draw_rounded_rect_fill(
            buf,
            self.width,
            self.height,
            self.width as f32 - 58.0,
            scy,
            38.0,
            12.0,
            10.0,
            pill_bg,
        );
        self.font.draw_text_centered(
            buf,
            self.width,
            self.height,
            &badge,
            self.width as f32 - 58.0,
            scy,
            11.0,
            col_dim,
            no_shadow,
            FontWeight::SemiBold,
        );

        let row_w = DIVIDER_X - 30.0;
        let row_cx = 14.0 + row_w * 0.5;
        for slot in 0..MAX_VISIBLE_ROWS {
            let idx = self.scroll_offset + slot;
            let Some(item) = self.filtered.get(idx) else {
                break;
            };
            let ry = LIST_TOP_Y + ROW_H * 0.5 + (slot as f32) * ROW_H;
            let is_sel = idx == self.selected_idx;
            if is_sel {
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    row_cx,
                    ry,
                    row_w * 0.5,
                    24.0,
                    12.0,
                    pill_bg,
                );
                UIFont::draw_rounded_rect_fill(
                    buf,
                    self.width,
                    self.height,
                    18.0,
                    ry,
                    1.5,
                    12.0,
                    1.5,
                    col_acc,
                );
            }
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                40.0,
                ry,
                16.0,
                16.0,
                9.0,
                pill_bg,
            );
            let (title, sub, glyph) = match item {
                SearchResult::Calc { expr, value } => (
                    format!("= {}", value),
                    format!("{} · Enter to copy", expr),
                    "=",
                ),
                SearchResult::App(a) => (
                    a.name.clone(),
                    a.comment.clone(),
                    if a.is_action { ">" } else { "•" },
                ),
            };
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                glyph,
                40.0,
                ry,
                14.0,
                col_acc,
                no_shadow,
                FontWeight::Bold,
            );
            let t_tr: String = title.chars().take(24).collect();
            let s_tr: String = sub.chars().take(34).collect();
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &t_tr,
                66.0,
                ry - 7.5,
                13.5,
                col_ink,
                no_shadow,
                FontWeight::SemiBold,
            );
            self.font.draw_text_centered_y(
                buf,
                self.width,
                self.height,
                &s_tr,
                66.0,
                ry + 9.0,
                10.5,
                col_dim,
                no_shadow,
                FontWeight::Medium,
            );
        }

        // Right Inspector
        let insp_cx = (DIVIDER_X + self.width as f32) * 0.5;
        if let Some(sel) = self.filtered.get(self.selected_idx) {
            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                insp_cx,
                158.0,
                34.0,
                34.0,
                16.0,
                pill_bg,
            );
            let (title, kind, desc) = match sel {
                SearchResult::Calc { expr, value } => (value.as_str(), "Calculator", expr.as_str()),
                SearchResult::App(a) => (
                    a.name.as_str(),
                    if a.is_action {
                        "System Action"
                    } else {
                        "Application"
                    },
                    a.comment.as_str(),
                ),
            };
            let ch = title
                .chars()
                .next()
                .unwrap_or('•')
                .to_uppercase()
                .to_string();
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                &ch,
                insp_cx,
                158.0,
                24.0,
                col_acc,
                no_shadow,
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                &title.chars().take(20).collect::<String>(),
                insp_cx,
                214.0,
                15.5,
                col_ink,
                no_shadow,
                FontWeight::Bold,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                kind,
                insp_cx,
                240.0,
                11.0,
                col_acc,
                no_shadow,
                FontWeight::SemiBold,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                &desc.chars().take(28).collect::<String>(),
                insp_cx,
                276.0,
                11.5,
                col_dim,
                no_shadow,
                FontWeight::Medium,
            );

            UIFont::draw_rounded_rect_fill(
                buf,
                self.width,
                self.height,
                insp_cx,
                414.0,
                96.0,
                16.0,
                11.0,
                pill_bg,
            );
            self.font.draw_text_centered(
                buf,
                self.width,
                self.height,
                "Launch · Enter",
                insp_cx,
                414.0,
                11.5,
                col_ink,
                no_shadow,
                FontWeight::SemiBold,
            );
        }

        self.font.draw_text_centered(
            buf,
            self.width,
            self.height,
            "↑↓ Navigate   ·   Enter Launch   ·   > System   ·   2+2 Math   ·   Esc Close",
            self.width as f32 * 0.5,
            468.0,
            10.5,
            col_dim,
            no_shadow,
            FontWeight::Medium,
        );
    }
}

fn eval_expr_add_sub(s: &str) -> Option<f64> {
    let mut terms = Vec::new();
    let mut ops = Vec::new();
    let mut start = 0usize;
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if (bytes[i] == b'+' || bytes[i] == b'-') && i > start {
            terms.push(eval_expr_mul_div(&s[start..i])?);
            ops.push(bytes[i]);
            start = i + 1;
        }
    }
    terms.push(eval_expr_mul_div(&s[start..])?);
    let mut acc = terms[0];
    for (idx, &op) in ops.iter().enumerate() {
        if op == b'+' {
            acc += terms[idx + 1];
        } else {
            acc -= terms[idx + 1];
        }
    }
    Some(acc)
}

fn eval_expr_mul_div(s: &str) -> Option<f64> {
    let mut factors = Vec::new();
    let mut ops = Vec::new();
    let mut start = 0usize;
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if (bytes[i] == b'*' || bytes[i] == b'/') && i > start {
            factors.push(s[start..i].parse::<f64>().ok()?);
            ops.push(bytes[i]);
            start = i + 1;
        }
    }
    factors.push(s[start..].parse::<f64>().ok()?);
    let mut acc = factors[0];
    for (idx, &op) in ops.iter().enumerate() {
        if op == b'*' {
            acc *= factors[idx + 1];
        } else {
            if factors[idx + 1].abs() < 1e-12 {
                return None;
            }
            acc /= factors[idx + 1];
        }
    }
    Some(acc)
}
