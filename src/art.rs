//! Procedural art: the animal cast, faces, pickups and small UI icons.
//!
//! Animals are drawn around the origin with their feet at `y = size / 2`;
//! wrap the call in [`with_transform`] to place, rotate or flip them.

use std::f32::consts::PI;

use macroquad::prelude::*;

use crate::fx::{fade, rgb};
use crate::ui::{self, shade};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Normal,
    Happy,
    Scared,
    Blink,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Crab,
    Frog,
    Penguin,
    Cat,
    Bunny,
    Fox,
    Panda,
    Octopus,
    Chick,
}

pub struct Animal {
    pub kind: Kind,
    pub name: &'static str,
    pub price: u32,
    pub body: (f32, f32, f32),
    /// Trail / sparkle color.
    pub trail: (f32, f32, f32),
}

pub const ANIMALS: [Animal; 9] = [
    Animal {
        kind: Kind::Crab,
        name: "Rusty",
        price: 0,
        body: (0.96, 0.42, 0.15),
        trail: (1.0, 0.65, 0.3),
    },
    Animal {
        kind: Kind::Frog,
        name: "Froggo",
        price: 60,
        body: (0.38, 0.8, 0.35),
        trail: (0.6, 1.0, 0.5),
    },
    Animal {
        kind: Kind::Chick,
        name: "Pip",
        price: 100,
        body: (1.0, 0.85, 0.3),
        trail: (1.0, 1.0, 0.6),
    },
    Animal {
        kind: Kind::Penguin,
        name: "Waddles",
        price: 150,
        body: (0.2, 0.24, 0.38),
        trail: (0.6, 0.85, 1.0),
    },
    Animal {
        kind: Kind::Cat,
        name: "Misu",
        price: 220,
        body: (0.62, 0.64, 0.75),
        trail: (1.0, 0.7, 0.8),
    },
    Animal {
        kind: Kind::Bunny,
        name: "Hopper",
        price: 300,
        body: (0.97, 0.93, 0.95),
        trail: (1.0, 0.75, 0.85),
    },
    Animal {
        kind: Kind::Fox,
        name: "Foxy",
        price: 400,
        body: (0.98, 0.55, 0.2),
        trail: (1.0, 0.8, 0.4),
    },
    Animal {
        kind: Kind::Panda,
        name: "Bamboo",
        price: 500,
        body: (0.96, 0.96, 0.96),
        trail: (0.6, 1.0, 0.6),
    },
    Animal {
        kind: Kind::Octopus,
        name: "Inky",
        price: 650,
        body: (0.72, 0.45, 0.95),
        trail: (0.9, 0.6, 1.0),
    },
];

pub const INK: Color = Color::new(0.1, 0.08, 0.15, 1.0);

/// Runs `draw` with a translate / rotate / vertical-flip transform applied.
pub fn with_transform(pos: Vec2, angle: f32, flip_y: bool, draw: impl FnOnce()) {
    let m = Mat4::from_translation(vec3(pos.x, pos.y, 0.0))
        * Mat4::from_rotation_z(angle)
        * Mat4::from_scale(vec3(1.0, if flip_y { -1.0 } else { 1.0 }, 1.0));
    // SAFETY: only the model-matrix stack is touched, on the render thread.
    unsafe { get_internal_gl().quad_gl.push_model_matrix(m) };
    draw();
    unsafe { get_internal_gl().quad_gl.pop_model_matrix() };
}

/// Everything needed to draw the player's animal.
#[derive(Clone, Copy)]
pub struct Look {
    pub animal: usize,
    pub mood: Mood,
    /// Where the eyes look, roughly unit length.
    pub gaze: Vec2,
    pub clock: f32,
    pub alpha: f32,
    /// Horizontal / vertical stretch (squash & stretch).
    pub stretch: Vec2,
}

impl Look {
    pub fn new(animal: usize, clock: f32) -> Self {
        Look {
            animal,
            mood: Mood::Normal,
            gaze: Vec2::ZERO,
            clock,
            alpha: 1.0,
            stretch: Vec2::ONE,
        }
    }
}

/// Draws the animal at the origin, feet at `y = size / 2`.
pub fn draw_animal(look: &Look, size: f32) {
    let info = &ANIMALS[look.animal.min(ANIMALS.len() - 1)];
    let w = size * look.stretch.x;
    let h = size * look.stretch.y;
    // Keep the feet planted while squashing.
    let base = size / 2.0;
    let cy = base - h / 2.0;
    let body = rgb(info.body, look.alpha);
    let a = look.alpha;
    let t = look.clock;

    match info.kind {
        Kind::Crab => {
            let red = body;
            let dark = shade(red, 0.6);
            // Legs wiggle as it scuttles.
            for side in [-1.0f32, 1.0] {
                for i in 0..3 {
                    let wig = (t * 14.0 + i as f32).sin() * 2.0;
                    let x0 = side * w * (0.18 + i as f32 * 0.12);
                    draw_line(
                        x0,
                        cy + h * 0.25,
                        x0 + side * w * 0.14,
                        base + wig * 0.3,
                        4.0,
                        dark,
                    );
                }
            }
            // Arms and claws.
            for side in [-1.0f32, 1.0] {
                let claw = vec2(
                    side * w * 0.62,
                    cy - h * 0.05 + (t * 3.0 + side).sin() * 2.0,
                );
                draw_line(
                    side * w * 0.38,
                    cy + h * 0.05,
                    claw.x,
                    claw.y + 6.0,
                    5.0,
                    dark,
                );
                draw_circle(claw.x, claw.y, w * 0.19, dark);
                draw_circle(claw.x, claw.y, w * 0.155, red);
                let tip = claw + vec2(side * w * 0.06, -w * 0.15);
                draw_circle(tip.x, tip.y, w * 0.1, dark);
                draw_circle(tip.x, tip.y, w * 0.075, red);
            }
            // Shell.
            draw_ellipse(0.0, cy + h * 0.1, w * 0.48, h * 0.38, 0.0, dark);
            draw_ellipse(0.0, cy + h * 0.08, w * 0.44, h * 0.34, 0.0, red);
            draw_ellipse(
                -w * 0.12,
                cy - h * 0.08,
                w * 0.2,
                h * 0.08,
                0.0,
                Color::new(1.0, 1.0, 1.0, 0.25 * a),
            );
            // Eyes on stalks.
            let r = w * 0.12;
            let e1 = vec2(-w * 0.17, cy - h * 0.42);
            let e2 = vec2(w * 0.17, cy - h * 0.42);
            for e in [e1, e2] {
                draw_line(e.x * 0.8, cy - h * 0.1, e.x, e.y, 4.0, dark);
            }
            eyes(e1, e2, r, look);
            mouth(vec2(0.0, cy + h * 0.16), w * 0.22, look.mood, a);
            cheeks(vec2(0.0, cy + h * 0.1), w * 0.27, w * 0.06, a);
        }
        Kind::Frog => {
            let belly = shade(body, 1.35);
            ui::chunky_rect(-w / 2.0, cy - h * 0.3, w, h * 0.8, w * 0.35, body);
            draw_ellipse(0.0, cy + h * 0.25, w * 0.3, h * 0.18, 0.0, fade(belly, 0.8));
            // Eye bumps.
            let e1 = vec2(-w * 0.24, cy - h * 0.32);
            let e2 = vec2(w * 0.24, cy - h * 0.32);
            for e in [e1, e2] {
                draw_circle(e.x, e.y, w * 0.2, shade(body, 0.6));
                draw_circle(e.x, e.y, w * 0.17, body);
            }
            eyes(e1, e2, w * 0.12, look);
            mouth(vec2(0.0, cy + h * 0.02), w * 0.32, look.mood, a);
            cheeks(vec2(0.0, cy + h * 0.02), w * 0.34, w * 0.06, a);
            // Feet.
            for side in [-1.0, 1.0] {
                draw_ellipse(
                    side * w * 0.3,
                    base - 3.0,
                    w * 0.16,
                    5.0,
                    0.0,
                    shade(body, 0.7),
                );
            }
        }
        Kind::Penguin => {
            for side in [-1.0f32, 1.0] {
                let flap = (t * 8.0).sin() * 10.0;
                draw_ellipse(
                    side * w * 0.42,
                    cy + h * 0.05,
                    w * 0.1,
                    h * 0.25,
                    side * (25.0 + flap),
                    shade(body, 0.8),
                );
            }
            draw_ellipse(0.0, cy, w * 0.44, h * 0.5, 0.0, shade(body, 0.6));
            draw_ellipse(0.0, cy, w * 0.41, h * 0.47, 0.0, body);
            draw_ellipse(
                0.0,
                cy + h * 0.1,
                w * 0.3,
                h * 0.34,
                0.0,
                Color::new(0.97, 0.97, 1.0, a),
            );
            let e1 = vec2(-w * 0.14, cy - h * 0.18);
            let e2 = vec2(w * 0.14, cy - h * 0.18);
            eyes(e1, e2, w * 0.1, look);
            let orange = Color::new(1.0, 0.6, 0.15, a);
            draw_triangle(
                vec2(-w * 0.08, cy - h * 0.06),
                vec2(w * 0.08, cy - h * 0.06),
                vec2(0.0, cy + h * 0.04),
                orange,
            );
            cheeks(vec2(0.0, cy - h * 0.02), w * 0.24, w * 0.05, a);
            for side in [-1.0, 1.0] {
                draw_ellipse(side * w * 0.17, base - 3.0, w * 0.13, 5.0, 0.0, orange);
            }
        }
        Kind::Cat => {
            let pink = Color::new(1.0, 0.6, 0.7, a);
            // Tail.
            for i in 0..6 {
                let k = i as f32 / 5.0;
                let p = vec2(
                    w * (0.45 + k * 0.2),
                    cy + h * (0.3 - k * 0.5) + (t * 4.0 + k * 3.0).sin() * 3.0,
                );
                draw_circle(p.x, p.y, w * 0.07, body);
            }
            for side in [-1.0f32, 1.0] {
                let x = side * w * 0.3;
                draw_triangle(
                    vec2(x - w * 0.16, cy - h * 0.3),
                    vec2(x + w * 0.16, cy - h * 0.3),
                    vec2(x + side * w * 0.04, cy - h * 0.62),
                    shade(body, 0.6),
                );
                draw_triangle(
                    vec2(x - w * 0.08, cy - h * 0.32),
                    vec2(x + w * 0.08, cy - h * 0.32),
                    vec2(x + side * w * 0.03, cy - h * 0.52),
                    pink,
                );
            }
            ui::chunky_rect(-w / 2.0, cy - h * 0.4, w, h * 0.9, w * 0.35, body);
            let e1 = vec2(-w * 0.18, cy - h * 0.08);
            let e2 = vec2(w * 0.18, cy - h * 0.08);
            eyes(e1, e2, w * 0.11, look);
            draw_triangle(
                vec2(-w * 0.05, cy + h * 0.06),
                vec2(w * 0.05, cy + h * 0.06),
                vec2(0.0, cy + h * 0.11),
                pink,
            );
            mouth(vec2(0.0, cy + h * 0.17), w * 0.14, look.mood, a);
            for side in [-1.0, 1.0] {
                for k in [-1.0, 1.0] {
                    draw_line(
                        side * w * 0.2,
                        cy + h * 0.12,
                        side * w * 0.48,
                        cy + h * (0.1 + k * 0.05),
                        1.5,
                        fade(INK, 0.6 * a),
                    );
                }
            }
        }
        Kind::Bunny => {
            let pink = Color::new(1.0, 0.7, 0.8, a);
            for side in [-1.0f32, 1.0] {
                let sway = side * 8.0 + (t * 3.0 + side).sin() * 4.0;
                let x = side * w * 0.18;
                draw_ellipse(
                    x,
                    cy - h * 0.62,
                    w * 0.12,
                    h * 0.34,
                    sway,
                    shade(body, 0.75),
                );
                draw_ellipse(x, cy - h * 0.62, w * 0.1, h * 0.31, sway, body);
                draw_ellipse(x, cy - h * 0.6, w * 0.05, h * 0.22, sway, pink);
            }
            ui::chunky_rect(-w / 2.0, cy - h * 0.38, w, h * 0.88, w * 0.4, body);
            let e1 = vec2(-w * 0.18, cy - h * 0.08);
            let e2 = vec2(w * 0.18, cy - h * 0.08);
            eyes(e1, e2, w * 0.11, look);
            draw_circle(0.0, cy + h * 0.07, w * 0.05, pink);
            mouth(vec2(0.0, cy + h * 0.15), w * 0.12, look.mood, a);
            draw_rectangle(
                -w * 0.05,
                cy + h * 0.18,
                w * 0.1,
                h * 0.09,
                Color::new(1.0, 1.0, 1.0, a),
            );
            draw_line(
                0.0,
                cy + h * 0.18,
                0.0,
                cy + h * 0.27,
                1.5,
                fade(INK, 0.5 * a),
            );
            cheeks(vec2(0.0, cy + h * 0.1), w * 0.3, w * 0.07, a);
        }
        Kind::Fox => {
            let white = Color::new(1.0, 0.97, 0.92, a);
            let dark = Color::new(0.25, 0.15, 0.12, a);
            for side in [-1.0f32, 1.0] {
                let x = side * w * 0.3;
                draw_triangle(
                    vec2(x - w * 0.18, cy - h * 0.3),
                    vec2(x + w * 0.18, cy - h * 0.3),
                    vec2(x + side * w * 0.06, cy - h * 0.68),
                    body,
                );
                draw_triangle(
                    vec2(x - w * 0.06 + side * w * 0.03, cy - h * 0.56),
                    vec2(x + w * 0.06 + side * w * 0.05, cy - h * 0.56),
                    vec2(x + side * w * 0.06, cy - h * 0.68),
                    dark,
                );
            }
            ui::chunky_rect(-w / 2.0, cy - h * 0.4, w, h * 0.9, w * 0.32, body);
            draw_ellipse(-w * 0.18, cy + h * 0.18, w * 0.22, h * 0.2, 0.0, white);
            draw_ellipse(w * 0.18, cy + h * 0.18, w * 0.22, h * 0.2, 0.0, white);
            let e1 = vec2(-w * 0.18, cy - h * 0.1);
            let e2 = vec2(w * 0.18, cy - h * 0.1);
            eyes(e1, e2, w * 0.1, look);
            draw_circle(0.0, cy + h * 0.06, w * 0.06, dark);
            mouth(vec2(0.0, cy + h * 0.18), w * 0.12, look.mood, a);
        }
        Kind::Panda => {
            let black = Color::new(0.12, 0.12, 0.15, a);
            for side in [-1.0f32, 1.0] {
                draw_circle(side * w * 0.32, cy - h * 0.36, w * 0.15, black);
            }
            ui::chunky_rect(-w / 2.0, cy - h * 0.4, w, h * 0.9, w * 0.38, body);
            for side in [-1.0f32, 1.0] {
                draw_ellipse(
                    side * w * 0.47,
                    cy + h * 0.2,
                    w * 0.1,
                    h * 0.18,
                    side * 20.0,
                    black,
                );
                draw_ellipse(
                    side * w * 0.19,
                    cy - h * 0.06,
                    w * 0.15,
                    h * 0.12,
                    side * -30.0,
                    black,
                );
            }
            let e1 = vec2(-w * 0.19, cy - h * 0.07);
            let e2 = vec2(w * 0.19, cy - h * 0.07);
            eyes(e1, e2, w * 0.09, look);
            draw_ellipse(0.0, cy + h * 0.08, w * 0.06, h * 0.04, 0.0, black);
            mouth(vec2(0.0, cy + h * 0.17), w * 0.12, look.mood, a);
            cheeks(vec2(0.0, cy + h * 0.12), w * 0.32, w * 0.06, a);
        }
        Kind::Octopus => {
            let dark = shade(body, 0.65);
            for i in 0..4 {
                let x0 = -w * 0.36 + i as f32 * w * 0.24;
                for k in 0..5 {
                    let f = k as f32 / 4.0;
                    let wave = (t * 6.0 + i as f32 * 1.3 + f * 3.0).sin() * w * 0.06 * f;
                    let r = w * (0.1 - f * 0.05);
                    draw_circle(x0 + wave, cy + h * (0.12 + f * 0.36), r, dark);
                    draw_circle(x0 + wave, cy + h * (0.12 + f * 0.36), r * 0.8, body);
                }
            }
            draw_ellipse(0.0, cy - h * 0.12, w * 0.46, h * 0.38, 0.0, dark);
            draw_ellipse(0.0, cy - h * 0.13, w * 0.43, h * 0.35, 0.0, body);
            draw_ellipse(
                -w * 0.15,
                cy - h * 0.33,
                w * 0.14,
                h * 0.07,
                -20.0,
                Color::new(1.0, 1.0, 1.0, 0.3 * a),
            );
            let e1 = vec2(-w * 0.17, cy - h * 0.1);
            let e2 = vec2(w * 0.17, cy - h * 0.1);
            eyes(e1, e2, w * 0.12, look);
            mouth(vec2(0.0, cy + h * 0.08), w * 0.12, look.mood, a);
            cheeks(vec2(0.0, cy + h * 0.02), w * 0.32, w * 0.06, a);
        }
        Kind::Chick => {
            let orange = Color::new(1.0, 0.55, 0.15, a);
            for side in [-1.0, 1.0] {
                draw_line(
                    side * w * 0.12,
                    cy + h * 0.38,
                    side * w * 0.14,
                    base,
                    3.0,
                    orange,
                );
            }
            for i in -1..=1 {
                draw_ellipse(
                    i as f32 * w * 0.07,
                    cy - h * 0.46,
                    w * 0.04,
                    h * 0.1,
                    i as f32 * 25.0,
                    shade(body, 0.8),
                );
            }
            draw_circle(0.0, cy, w * 0.46, shade(body, 0.7));
            draw_circle(0.0, cy, w * 0.43, body);
            for side in [-1.0f32, 1.0] {
                let flap = (t * 10.0).sin() * 12.0;
                draw_ellipse(
                    side * w * 0.42,
                    cy + h * 0.08,
                    w * 0.1,
                    h * 0.16,
                    side * (30.0 + flap),
                    shade(body, 0.85),
                );
            }
            let e1 = vec2(-w * 0.16, cy - h * 0.1);
            let e2 = vec2(w * 0.16, cy - h * 0.1);
            eyes(e1, e2, w * 0.1, look);
            draw_triangle(
                vec2(-w * 0.08, cy + h * 0.04),
                vec2(w * 0.08, cy + h * 0.04),
                vec2(0.0, cy + h * 0.16),
                orange,
            );
            if look.mood == Mood::Happy {
                draw_triangle(
                    vec2(-w * 0.08, cy + h * 0.04),
                    vec2(w * 0.08, cy + h * 0.04),
                    vec2(0.0, cy - h * 0.02),
                    orange,
                );
            }
            cheeks(vec2(0.0, cy + h * 0.04), w * 0.28, w * 0.06, a);
        }
    }
}

fn eyes(e1: Vec2, e2: Vec2, r: f32, look: &Look) {
    let a = look.alpha;
    let ink = fade(INK, a);
    for e in [e1, e2] {
        match look.mood {
            Mood::Blink => draw_line(e.x - r, e.y, e.x + r, e.y, 3.0, ink),
            Mood::Happy => {
                draw_line(e.x - r, e.y + r * 0.3, e.x, e.y - r * 0.5, 3.5, ink);
                draw_line(e.x, e.y - r * 0.5, e.x + r, e.y + r * 0.3, 3.5, ink);
            }
            Mood::Normal | Mood::Scared => {
                let er = if look.mood == Mood::Scared {
                    r * 1.2
                } else {
                    r
                };
                draw_circle(e.x, e.y, er, Color::new(1.0, 1.0, 1.0, a));
                let pr = if look.mood == Mood::Scared {
                    r * 0.4
                } else {
                    r * 0.6
                };
                let off = look.gaze.clamp_length_max(1.0) * (er - pr) * 0.9;
                draw_circle(e.x + off.x, e.y + off.y, pr, ink);
                draw_circle(
                    e.x + off.x - pr * 0.3,
                    e.y + off.y - pr * 0.35,
                    pr * 0.32,
                    Color::new(1.0, 1.0, 1.0, a),
                );
            }
        }
    }
}

fn mouth(c: Vec2, w: f32, mood: Mood, a: f32) {
    let ink = fade(INK, a);
    match mood {
        Mood::Scared => draw_circle(c.x, c.y + 2.0, w * 0.3, ink),
        Mood::Happy => half_disk(vec2(c.x, c.y - 1.0), w * 0.45, ink),
        Mood::Normal | Mood::Blink => {
            let m = w * 0.4;
            draw_line(c.x - m, c.y - 2.0, c.x - m * 0.4, c.y + 1.5, 2.5, ink);
            draw_line(c.x - m * 0.4, c.y + 1.5, c.x + m * 0.4, c.y + 1.5, 2.5, ink);
            draw_line(c.x + m * 0.4, c.y + 1.5, c.x + m, c.y - 2.0, 2.5, ink);
        }
    }
}

fn cheeks(c: Vec2, dx: f32, r: f32, a: f32) {
    let pink = Color::new(1.0, 0.45, 0.5, 0.35 * a);
    draw_circle(c.x - dx, c.y, r, pink);
    draw_circle(c.x + dx, c.y, r, pink);
}

/// Lower half of a disk (a big open smile).
pub fn half_disk(c: Vec2, r: f32, color: Color) {
    const N: usize = 10;
    for i in 0..N {
        let a0 = PI * i as f32 / N as f32;
        let a1 = PI * (i + 1) as f32 / N as f32;
        draw_triangle(
            c,
            c + vec2(a0.cos(), a0.sin()) * r,
            c + vec2(a1.cos(), a1.sin()) * r,
            color,
        );
    }
}

/// Grumpy eyes, brows and mouth for obstacles, eyeing `target`.
pub fn grumpy_face(c: Vec2, size: f32, width: f32, target: Vec2) {
    let eye_r = (size * 0.13).min(10.0);
    let spread = (width * 0.18).clamp(eye_r * 1.6, 26.0);
    let look = (target - c).normalize_or_zero();
    for side in [-1.0, 1.0] {
        let ex = c.x + side * spread;
        let ey = c.y - size * 0.05;
        draw_circle(ex, ey, eye_r, WHITE);
        let pr = eye_r * 0.55;
        draw_circle(
            ex + look.x * (eye_r - pr),
            ey + look.y * (eye_r - pr),
            pr,
            INK,
        );
        let by = ey - eye_r - 3.0;
        draw_line(
            ex - side * eye_r,
            by - 4.0,
            ex + side * eye_r * 0.6,
            by + 2.0,
            3.5,
            INK,
        );
    }
    let mw = (width * 0.1).min(12.0);
    draw_line(
        c.x - mw,
        c.y + size * 0.25,
        c.x + mw,
        c.y + size * 0.25,
        3.0,
        INK,
    );
}

pub fn draw_coin(pos: Vec2, age: f32, r: f32) {
    // Fake a spinning coin by squashing its width.
    let spin = (age * 4.0).cos().abs().max(0.25);
    draw_ellipse(
        pos.x,
        pos.y,
        r * spin,
        r,
        0.0,
        Color::new(0.85, 0.55, 0.05, 1.0),
    );
    draw_ellipse(pos.x, pos.y, r * spin * 0.75, r * 0.75, 0.0, GOLD);
    draw_ellipse(
        pos.x - r * spin * 0.25,
        pos.y - r * 0.3,
        r * spin * 0.2,
        r * 0.22,
        0.0,
        Color::new(1.0, 1.0, 0.8, 0.8),
    );
}

/// A round power-up bubble with a letter.
pub fn draw_power_bubble(pos: Vec2, age: f32, color: Color, label: &str, r: f32) {
    let r = r * (1.0 + (age * 8.0).sin() * 0.1);
    draw_circle(pos.x, pos.y, r + 8.0, fade(color, 0.2));
    draw_circle(pos.x, pos.y, r, shade(color, 0.6));
    draw_circle(pos.x, pos.y, r - 3.0, color);
    ui::text_centered(label, pos.x, pos.y + r * 0.42, r * 1.25, WHITE);
}

pub fn coin_icon(c: Vec2, r: f32) {
    draw_circle(c.x, c.y, r, Color::new(0.85, 0.55, 0.05, 1.0));
    draw_circle(c.x, c.y, r * 0.75, GOLD);
}

pub fn heart(c: Vec2, s: f32, color: Color) {
    draw_circle(c.x - s * 0.24, c.y - s * 0.1, s * 0.28, color);
    draw_circle(c.x + s * 0.24, c.y - s * 0.1, s * 0.28, color);
    draw_triangle(
        vec2(c.x - s * 0.5, c.y),
        vec2(c.x + s * 0.5, c.y),
        vec2(c.x, c.y + s * 0.5),
        color,
    );
}

pub fn trophy(c: Vec2, color: Color) {
    ui::rounded_rect(c.x - 8.0, c.y - 9.0, 16.0, 12.0, 4.0, color);
    draw_rectangle(c.x - 2.0, c.y + 2.0, 4.0, 5.0, color);
    draw_rectangle(c.x - 6.0, c.y + 6.0, 12.0, 3.0, color);
}

pub fn lock(c: Vec2) {
    let gold = Color::new(1.0, 0.8, 0.2, 1.0);
    draw_circle_lines(c.x, c.y - 8.0, 10.0, 4.0, gold);
    ui::rounded_rect(c.x - 15.0, c.y - 6.0, 30.0, 24.0, 6.0, gold);
    draw_circle(c.x, c.y + 4.0, 3.5, Color::new(0.3, 0.2, 0.05, 1.0));
}

pub fn speaker(c: Vec2, on: bool) {
    let white = WHITE;
    draw_rectangle(c.x - 14.0, c.y - 6.0, 8.0, 12.0, white);
    draw_triangle(
        vec2(c.x - 8.0, c.y - 6.0),
        vec2(c.x + 2.0, c.y - 14.0),
        vec2(c.x + 2.0, c.y + 14.0),
        white,
    );
    draw_triangle(
        vec2(c.x - 8.0, c.y - 6.0),
        vec2(c.x - 8.0, c.y + 6.0),
        vec2(c.x + 2.0, c.y + 14.0),
        white,
    );
    if on {
        draw_line(c.x + 7.0, c.y - 6.0, c.x + 10.0, c.y, 3.0, white);
        draw_line(c.x + 10.0, c.y, c.x + 7.0, c.y + 6.0, 3.0, white);
        draw_line(c.x + 12.0, c.y - 11.0, c.x + 16.0, c.y, 3.0, white);
        draw_line(c.x + 16.0, c.y, c.x + 12.0, c.y + 11.0, 3.0, white);
    } else {
        draw_line(c.x + 7.0, c.y - 7.0, c.x + 17.0, c.y + 7.0, 3.0, white);
        draw_line(c.x + 17.0, c.y - 7.0, c.x + 7.0, c.y + 7.0, 3.0, white);
    }
}
