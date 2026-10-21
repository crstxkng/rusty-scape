//! Visual effects: particles, floating text, and the starfield background.

use macroquad::prelude::*;

use crate::ui::{self, VIRTUAL_H, VIRTUAL_W};

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
                size: rand::gen_range(3.0, 8.0),
                color,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.pos += p.vel * dt;
            p.vel.y += 600.0 * dt;
            p.vel *= 1.0 - 1.5 * dt;
            p.life -= dt;
        }
        self.list.retain(|p| p.life > 0.0);
    }

    pub fn draw(&self) {
        for p in &self.list {
            let t = p.life / p.max_life;
            let s = p.size * t;
            draw_rectangle(p.pos.x - s / 2.0, p.pos.y - s / 2.0, s, s, fade(p.color, t));
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
    color: Color,
}

#[derive(Default)]
pub struct FloatingTexts {
    list: Vec<FloatingText>,
}

impl FloatingTexts {
    pub fn spawn(&mut self, text: impl Into<String>, pos: Vec2, color: Color) {
        self.list.push(FloatingText {
            text: text.into(),
            pos,
            life: 1.0,
            color,
        });
    }

    pub fn update(&mut self, dt: f32) {
        for t in &mut self.list {
            t.pos.y -= 60.0 * dt;
            t.life -= dt;
        }
        self.list.retain(|t| t.life > 0.0);
    }

    pub fn draw(&self) {
        for t in &self.list {
            ui::text_centered(
                &t.text,
                t.pos.x,
                t.pos.y,
                22.0,
                fade(t.color, t.life.min(1.0)),
            );
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

struct Star {
    pos: Vec2,
    depth: f32,
}

pub struct Starfield {
    stars: Vec<Star>,
}

impl Starfield {
    pub fn new() -> Self {
        let stars = (0..90)
            .map(|_| Star {
                pos: vec2(
                    rand::gen_range(0.0, VIRTUAL_W),
                    rand::gen_range(0.0, VIRTUAL_H),
                ),
                depth: rand::gen_range(0.15, 1.0),
            })
            .collect();
        Starfield { stars }
    }

    pub fn update(&mut self, dt: f32, speed: f32) {
        for s in &mut self.stars {
            s.pos.y += speed * s.depth * dt;
            if s.pos.y > VIRTUAL_H {
                s.pos.y -= VIRTUAL_H;
                s.pos.x = rand::gen_range(0.0, VIRTUAL_W);
            }
        }
    }

    pub fn draw(&self, tint: Color) {
        for s in &self.stars {
            let len = 2.0 + 6.0 * s.depth;
            let c = Color::new(
                0.5 + tint.r * 0.5,
                0.5 + tint.g * 0.5,
                0.5 + tint.b * 0.5,
                0.15 + 0.5 * s.depth,
            );
            draw_rectangle(s.pos.x, s.pos.y, 1.0 + s.depth * 1.5, len, c);
        }
    }
}
