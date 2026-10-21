//! Resolution-independent drawing and input helpers.
//!
//! The game is laid out on a fixed virtual canvas that is letterboxed into
//! whatever window / browser canvas we get, so it plays the same everywhere.

use std::sync::atomic::{AtomicU32, Ordering};

use macroquad::prelude::*;

pub const VIRTUAL_W: f32 = 960.0;
pub const VIRTUAL_H: f32 = 640.0;

/// Maps the virtual canvas onto the real window.
#[derive(Clone, Copy)]
pub struct View {
    pub scale: f32,
    pub offset: Vec2,
}

/// Current view scale (f32 bits), used to rasterize glyphs at on-screen size.
static TEXT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

impl View {
    pub fn compute() -> Self {
        let (sw, sh) = (screen_width(), screen_height());
        let scale = (sw / VIRTUAL_W).min(sh / VIRTUAL_H).max(0.01);
        let offset = vec2(
            (sw - VIRTUAL_W * scale) / 2.0,
            (sh - VIRTUAL_H * scale) / 2.0,
        );
        View { scale, offset }
    }

    /// Activates a camera so all drawing happens in virtual coordinates.
    pub fn apply(&self, shake: Vec2) {
        TEXT_SCALE.store(self.scale.to_bits(), Ordering::Relaxed);
        let mut cam =
            Camera2D::from_display_rect(Rect::new(-shake.x, -shake.y, VIRTUAL_W, VIRTUAL_H));
        // from_display_rect is y-up (meant for render targets); flip it for the screen.
        cam.zoom.y = -cam.zoom.y;
        cam.viewport = Some((
            self.offset.x.round() as i32,
            self.offset.y.round() as i32,
            (VIRTUAL_W * self.scale).round() as i32,
            (VIRTUAL_H * self.scale).round() as i32,
        ));
        set_camera(&cam);
    }

    pub fn to_virtual(self, p: Vec2) -> Vec2 {
        (p - self.offset) / self.scale
    }

    pub fn mouse(&self) -> Vec2 {
        self.to_virtual(mouse_position().into())
    }
}

fn params(size: f32, color: Color) -> TextParams<'static> {
    // Rasterize at the on-screen pixel size so text stays crisp when scaled.
    let scale = f32::from_bits(TEXT_SCALE.load(Ordering::Relaxed));
    TextParams {
        font_size: (size * scale).round().max(1.0) as u16,
        font_scale: 1.0 / scale,
        color,
        ..Default::default()
    }
}

pub fn text_width(s: &str, size: f32) -> f32 {
    let p = params(size, WHITE);
    measure_text(s, None, p.font_size, p.font_scale).width
}

pub fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(s, x, y, params(size, color));
}

pub fn text_centered(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    text(s, cx - text_width(s, size) / 2.0, y, size, color);
}

/// Text with a dark drop shadow for readability over busy backgrounds.
pub fn text_shadow_centered(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    text_centered(
        s,
        cx + 2.0,
        y + 2.0,
        size,
        Color::new(0.0, 0.0, 0.0, 0.6 * color.a),
    );
    text_centered(s, cx, y, size, color);
}

/// A clickable/tappable button. Returns true when pressed this frame.
pub fn button(view: &View, rect: Rect, label: &str, accent: Color) -> bool {
    let hovered = rect.contains(view.mouse());
    let bg = if hovered {
        Color::new(accent.r, accent.g, accent.b, 0.35)
    } else {
        Color::new(1.0, 1.0, 1.0, 0.06)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, accent);
    text_centered(
        label,
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0 + 8.0,
        24.0,
        WHITE,
    );
    hovered && is_mouse_button_pressed(MouseButton::Left)
}

pub fn clicked_in(view: &View, rect: Rect) -> bool {
    is_mouse_button_pressed(MouseButton::Left) && rect.contains(view.mouse())
}
