//! Visual effects: particles, floating text, banners and the background.

use macroquad::prelude::*;

use crate::ui::{self, vh, vw};

pub fn rgb(c: (f32, f32, f32), a: f32) -> Color {
    Color::new(c.0, c.1, c.2, a)
}

pub fn fade(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a)
}

pub struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max_life: f32,
    size: f32,
    color: Color,
}

#[derive(Default)]
pub struct Particles {
    list: Vec<Particle>,
}

impl Particles {
    pub fn burst(&mut self, at: Vec2, count: usize, color: Color, speed: f32) {
        for _ in 0..count {
            let angle = rand::gen_range(0.0, std::f32::consts::TAU);
            let s = rand::gen_range(speed * 0.3, speed);
            let life = rand::gen_range(0.4, 0.9);
            self.list.push(Particle {
                pos: at,
                vel: vec2(angle.cos(), angle.sin()) * s,
                life,
                max_life: life,
                size: rand::gen_range(4.0, 11.0),
                color,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.pos += p.vel * dt;
            p.vel.y += 700.0 * dt;
            p.vel *= 1.0 - 1.8 * dt;
            p.life -= dt;
        }
        self.list.retain(|p| p.life > 0.0);
    }

    pub fn draw(&self) {
        for p in &self.list {
            let t = p.life / p.max_life;
            draw_circle(p.pos.x, p.pos.y, p.size * t, fade(p.color, t.sqrt()));
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

pub struct FloatingText {
    text: String,
    pos: Vec2,
    life: f32,
    size: f32,
    color: Color,
}

#[derive(Default)]
pub struct FloatingTexts {
    list: Vec<FloatingText>,
}

impl FloatingTexts {
    pub fn spawn(&mut self, text: impl Into<String>, pos: Vec2, color: Color) {
        self.spawn_sized(text, pos, color, 28.0);
    }

    pub fn spawn_sized(&mut self, text: impl Into<String>, pos: Vec2, color: Color, size: f32) {
        // Keep popups on screen near the walls.
        let x = pos.x.clamp(80.0, vw() - 80.0);
        self.list.push(FloatingText {
            text: text.into(),
            pos: vec2(x, pos.y),
            life: 1.0,
            size,
            color,
        });
    }

    pub fn update(&mut self, dt: f32) {
        for t in &mut self.list {
            t.pos.y -= 70.0 * dt;
            t.life -= dt;
        }
        self.list.retain(|t| t.life > 0.0);
    }

    pub fn draw(&self) {
        for t in &self.list {
            // Pop in with a little overshoot, then fade.
            let age = 1.0 - t.life;
            let pop = if age < 0.15 {
                0.6 + age / 0.15 * 0.6
            } else {
                1.2 - ((age - 0.15) * 2.0).min(0.2)
            };
            ui::text_shadow_centered(
                &t.text,
                t.pos.x,
                t.pos.y,
                t.size * pop,
                fade(t.color, t.life.min(0.5) * 2.0),
            );
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

/// A big centered announcement ("WAVE 3", "NEW BEST!").
#[derive(Default)]
pub struct Banner {
    title: String,
    subtitle: String,
    color: Color,
    time: f32,
}

const BANNER_TIME: f32 = 1.6;

impl Banner {
    pub fn show(&mut self, title: impl Into<String>, subtitle: impl Into<String>, color: Color) {
        self.title = title.into();
        self.subtitle = subtitle.into();
        self.color = color;
        self.time = BANNER_TIME;
    }

    pub fn update(&mut self, dt: f32) {
        self.time = (self.time - dt).max(0.0);
    }

    pub fn draw(&self) {
        if self.time <= 0.0 {
            return;
        }
        let age = BANNER_TIME - self.time;
        let scale = if age < 0.2 {
            0.5 + age / 0.2 * 0.6
        } else {
            1.1 - ((age - 0.2) * 0.5).min(0.1)
        };
        let alpha = (self.time / 0.35).min(1.0);
        let y = vh() * 0.36;
        let size = if ui::is_portrait() { 58.0 } else { 70.0 };
        ui::text_shadow_centered(
            &self.title,
            vw() / 2.0,
            y,
            size * scale,
            fade(self.color, alpha),
        );
        if !self.subtitle.is_empty() {
            ui::text_shadow_centered(
                &self.subtitle,
                vw() / 2.0,
                y + 40.0,
                26.0,
                fade(WHITE, alpha),
            );
        }
    }
}

struct Bubble {
    pos: Vec2,
    depth: f32,
    radius: f32,
}

/// Soft out-of-focus bubbles drifting down at different depths.
pub struct Background {
    bubbles: Vec<Bubble>,
}

impl Background {
    pub fn new() -> Self {
        let bubbles = (0..40)
            .map(|_| Bubble {
                pos: vec2(rand::gen_range(0.0, 1200.0), rand::gen_range(0.0, 1200.0)),
                depth: rand::gen_range(0.15, 1.0),
                radius: rand::gen_range(2.0, 7.0),
            })
            .collect();
        Background { bubbles }
    }

    pub fn update(&mut self, dt: f32, speed: f32) {
        let (w, h) = (vw(), vh());
        for b in &mut self.bubbles {
            b.pos.y += speed * b.depth * dt;
            if b.pos.y > h + 20.0 {
                b.pos.y -= h + 40.0;
                b.pos.x = rand::gen_range(0.0, w);
            }
        }
    }

    pub fn draw(&self, sky: ((f32, f32, f32), (f32, f32, f32)), time: f32) {
        let (w, h) = (vw(), vh());
        ui::vertical_gradient(
            Rect::new(-30.0, -30.0, w + 60.0, h + 60.0),
            rgb(sky.0, 1.0),
            rgb(sky.1, 1.0),
        );
        for b in &self.bubbles {
            if b.pos.x > w + 10.0 {
                continue;
            }
            let twinkle = 0.75 + 0.25 * (time * 2.0 + b.pos.x * 0.05).sin();
            let a = (0.08 + 0.25 * b.depth) * twinkle;
            draw_circle(
                b.pos.x,
                b.pos.y,
                b.radius * (0.6 + b.depth),
                Color::new(1.0, 1.0, 1.0, a),
            );
        }
    }
}
