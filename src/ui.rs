//! Resolution-independent drawing and input helpers.
//!
//! The game is laid out on a virtual canvas whose shape follows the screen:
//! landscape windows get a wide canvas, phones held upright get a tall one.
//! The canvas is then scaled to fit, so it plays the same everywhere.

use std::cell::RefCell;
use std::f32::consts::FRAC_PI_2;
use std::sync::atomic::{AtomicU32, Ordering};

use macroquad::prelude::*;

/// Current virtual canvas size and glyph raster scale (f32 bits).
static VW: AtomicU32 = AtomicU32::new(0x4470_0000); // 960.0
static VH: AtomicU32 = AtomicU32::new(0x4420_0000); // 640.0
static TEXT_SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

thread_local! {
    static FONT: RefCell<Option<Font>> = const { RefCell::new(None) };
}

/// Virtual canvas width.
pub fn vw() -> f32 {
    f32::from_bits(VW.load(Ordering::Relaxed))
}

/// Virtual canvas height.
pub fn vh() -> f32 {
    f32::from_bits(VH.load(Ordering::Relaxed))
}

pub fn is_portrait() -> bool {
    vh() > vw()
}

pub fn load_font() {
    if let Ok(font) = load_ttf_font_from_bytes(include_bytes!("../assets/LilitaOne-Regular.ttf")) {
        FONT.with(|f| *f.borrow_mut() = Some(font));
    }
}

/// Maps the virtual canvas onto the real window.
#[derive(Clone, Copy)]
pub struct View {
    pub scale: f32,
    pub offset: Vec2,
}

impl View {
    pub fn compute() -> Self {
        let (sw, sh) = (screen_width().max(1.0), screen_height().max(1.0));
        let aspect = sw / sh;
        let (w, h) = if aspect < 0.9 {
            // Portrait phone: fixed width, height follows the screen.
            (540.0, (540.0 / aspect).clamp(760.0, 1200.0))
        } else {
            ((640.0 * aspect).clamp(860.0, 1200.0), 640.0)
        };
        VW.store(f32::to_bits(w), Ordering::Relaxed);
        VH.store(f32::to_bits(h), Ordering::Relaxed);
        let scale = (sw / w).min(sh / h);
        let offset = vec2((sw - w * scale) / 2.0, (sh - h * scale) / 2.0);
        View { scale, offset }
    }

    /// Activates a camera so all drawing happens in virtual coordinates.
    pub fn apply(&self, shake: Vec2) {
        let dpi = macroquad::miniquad::window::dpi_scale();
        TEXT_SCALE.store((self.scale * dpi).to_bits(), Ordering::Relaxed);
        let mut cam = Camera2D::from_display_rect(Rect::new(-shake.x, -shake.y, vw(), vh()));
        // from_display_rect is y-up (meant for render targets); flip it for the screen.
        cam.zoom.y = -cam.zoom.y;
        // The viewport is in physical framebuffer pixels.
        cam.viewport = Some((
            (self.offset.x * dpi).round() as i32,
            (self.offset.y * dpi).round() as i32,
            (vw() * self.scale * dpi).round() as i32,
            (vh() * self.scale * dpi).round() as i32,
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

fn with_params<R>(size: f32, color: Color, f: impl FnOnce(TextParams) -> R) -> R {
    // Rasterize near the on-screen pixel size so text stays crisp, but snap to
    // steps of 6px: animated text would otherwise rasterize a new glyph set
    // every frame and churn the font atlas.
    let scale = f32::from_bits(TEXT_SCALE.load(Ordering::Relaxed));
    let raster = ((size * scale / 6.0).round().max(1.0) * 6.0).min(360.0);
    FONT.with(|font| {
        let font = font.borrow();
        f(TextParams {
            font: font.as_ref(),
            font_size: raster as u16,
            font_scale: size / raster,
            color,
            ..Default::default()
        })
    })
}

pub fn text_width(s: &str, size: f32) -> f32 {
    with_params(size, WHITE, |p| {
        measure_text(s, p.font, p.font_size, p.font_scale).width
    })
}

pub fn text(s: &str, x: f32, y: f32, size: f32, color: Color) {
    with_params(size, color, |p| {
        draw_text_ex(s, x, y, p);
    });
}

pub fn text_centered(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    text(s, cx - text_width(s, size) / 2.0, y, size, color);
}

/// Text with a chunky drop shadow for readability over busy backgrounds.
pub fn text_shadow_centered(s: &str, cx: f32, y: f32, size: f32, color: Color) {
    let d = (size * 0.06).max(2.0);
    text_centered(
        s,
        cx + d,
        y + d,
        size,
        Color::new(0.0, 0.0, 0.0, 0.45 * color.a),
    );
    text_centered(s, cx, y, size, color);
}

/// Darkens (`amount` < 1) or lightens (`amount` > 1) a color.
pub fn shade(c: Color, amount: f32) -> Color {
    if amount <= 1.0 {
        Color::new(c.r * amount, c.g * amount, c.b * amount, c.a)
    } else {
        let t = amount - 1.0;
        Color::new(
            c.r + (1.0 - c.r) * t,
            c.g + (1.0 - c.g) * t,
            c.b + (1.0 - c.b) * t,
            c.a,
        )
    }
}

/// A filled rectangle with rounded corners. Pieces never overlap, so
/// translucent colors blend correctly.
pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    if r < 0.5 {
        draw_rectangle(x, y, w, h, color);
        return;
    }
    draw_rectangle(x, y + r, w, h - 2.0 * r, color);
    draw_rectangle(x + r, y, w - 2.0 * r, r, color);
    draw_rectangle(x + r, y + h - r, w - 2.0 * r, r, color);
    let corners = [
        (vec2(x + w - r, y + r), -FRAC_PI_2),
        (vec2(x + r, y + r), -2.0 * FRAC_PI_2),
        (vec2(x + r, y + h - r), FRAC_PI_2),
        (vec2(x + w - r, y + h - r), 0.0),
    ];
    const SEGMENTS: usize = 6;
    for (c, start) in corners {
        for i in 0..SEGMENTS {
            let a0 = start + FRAC_PI_2 * i as f32 / SEGMENTS as f32;
            let a1 = start + FRAC_PI_2 * (i + 1) as f32 / SEGMENTS as f32;
            draw_triangle(
                c,
                c + vec2(a0.cos(), a0.sin()) * r,
                c + vec2(a1.cos(), a1.sin()) * r,
                color,
            );
        }
    }
}

/// A rounded rect with a darker rim, the house style for every block.
pub fn chunky_rect(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let rim = (w.min(h) * 0.08).clamp(2.0, 5.0);
    rounded_rect(x, y, w, h, r, shade(color, 0.55));
    rounded_rect(
        x + rim,
        y + rim,
        w - 2.0 * rim,
        h - 2.0 * rim,
        (r - rim).max(1.0),
        color,
    );
    // Glossy top highlight.
    rounded_rect(
        x + rim * 2.0,
        y + rim * 1.6,
        w - 4.0 * rim,
        (h * 0.22).min(14.0),
        (r * 0.5).max(1.0),
        Color::new(1.0, 1.0, 1.0, 0.28 * color.a),
    );
}

/// Vertical gradient filling a rectangle.
pub fn vertical_gradient(rect: Rect, top: Color, bottom: Color) {
    let v = |x: f32, y: f32, c: Color| Vertex::new(x, y, 0.0, 0.0, 0.0, c);
    let mesh = Mesh {
        vertices: vec![
            v(rect.x, rect.y, top),
            v(rect.right(), rect.y, top),
            v(rect.right(), rect.bottom(), bottom),
            v(rect.x, rect.bottom(), bottom),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    };
    draw_mesh(&mesh);
}

/// A big, finger-friendly button. Returns true when pressed this frame.
pub fn button(view: &View, rect: Rect, label: &str, accent: Color) -> bool {
    let hovered = rect.contains(view.mouse());
    let down = hovered && is_mouse_button_down(MouseButton::Left);
    let lift = if down { 0.0 } else { 4.0 };
    rounded_rect(
        rect.x,
        rect.y + 4.0,
        rect.w,
        rect.h,
        14.0,
        shade(accent, 0.45),
    );
    let face = if hovered { shade(accent, 1.15) } else { accent };
    rounded_rect(rect.x, rect.y + 4.0 - lift, rect.w, rect.h, 14.0, face);
    let size = (rect.h * 0.5).min(30.0);
    ui_label(
        label,
        rect.x + rect.w / 2.0,
        rect.y + 4.0 - lift + rect.h / 2.0 + size * 0.36,
        size,
    );
    hovered && is_mouse_button_pressed(MouseButton::Left)
}

fn ui_label(label: &str, cx: f32, y: f32, size: f32) {
    text_centered(
        label,
        cx + 1.5,
        y + 1.5,
        size,
        Color::new(0.0, 0.0, 0.0, 0.3),
    );
    text_centered(label, cx, y, size, WHITE);
}

pub fn clicked_in(view: &View, rect: Rect) -> bool {
    is_mouse_button_pressed(MouseButton::Left) && rect.contains(view.mouse())
}
