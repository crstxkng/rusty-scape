//! Deep Sea Scape: hold to swim up, let go to sink, and weave between
//! jellyfish, kelp and fishing hooks.

use std::f32::consts::PI;

use macroquad::prelude::*;

use super::{Event, Input, Pickup, Power, Scape, Stats};
use crate::art::{self, Look};
use crate::fx::fade;
use crate::ui::{vh, vw};

const PLAYER_SIZE: f32 = 52.0;
const SWIM: f32 = 1700.0;
const SINK: f32 = 950.0;
const MAX_VY: f32 = 430.0;

fn surface() -> f32 {
    70.0
}

fn seabed() -> f32 {
    vh() - 80.0
}

fn player_x() -> f32 {
    vw() * 0.28
}

#[derive(Clone, Copy, PartialEq)]
enum HazardKind {
    Jelly,
    Kelp,
    Hook,
}

struct Hazard {
    kind: HazardKind,
    x: f32,
    /// Jelly: center height. Kelp: top. Hook: bottom.
    y: f32,
    phase: f32,
    passed: bool,
}

impl Hazard {
    fn jelly_center(&self, t: f32) -> Vec2 {
        vec2(self.x, self.y + (t * 1.8 + self.phase).sin() * 28.0)
    }

    /// The parts that hurt, as circles (center, radius).
    fn hurts(&self, t: f32) -> Vec<(Vec2, f32)> {
        match self.kind {
            HazardKind::Jelly => {
                let c = self.jelly_center(t);
                vec![(c, 24.0), (c + vec2(0.0, 26.0), 14.0)]
            }
            HazardKind::Kelp => {
                // A column of circles from the seabed up to `y`.
                let mut v = Vec::new();
                let mut y = seabed() - 14.0;
                while y > self.y {
                    v.push((
                        vec2(self.x + ((y * 0.03 + t * 2.0 + self.phase).sin() * 6.0), y),
                        15.0,
                    ));
                    y -= 24.0;
                }
                v
            }
            HazardKind::Hook => {
                let mut v = vec![(vec2(self.x, self.y - 12.0), 16.0)];
                let mut y = self.y - 40.0;
                while y > surface() {
                    v.push((vec2(self.x, y), 5.0));
                    y -= 22.0;
                }
                v
            }
        }
    }

    fn top_bottom(&self, t: f32) -> (f32, f32) {
        match self.kind {
            HazardKind::Jelly => {
                let c = self.jelly_center(t);
                (c.y - 24.0, c.y + 40.0)
            }
            HazardKind::Kelp => (self.y, seabed()),
            HazardKind::Hook => (surface(), self.y),
        }
    }
}

pub struct Sea {
    time: f32,
    distance: f32,
    y: f32,
    vy: f32,
    hazards: Vec<Hazard>,
    pickups: Vec<Pickup>,
    bubbles: Vec<(Vec2, f32)>,
    bubble_timer: f32,
    next_spawn: f32,
    scroll: f32,
}

impl Sea {
    pub fn new() -> Self {
        Sea {
            time: 0.0,
            distance: 0.0,
            y: vh() * 0.5,
            vy: 0.0,
            hazards: Vec::new(),
            pickups: Vec::new(),
            bubbles: Vec::new(),
            bubble_timer: 0.0,
            next_spawn: vw() * 0.7,
            scroll: 0.0,
        }
    }

    pub fn stats(&self) -> Stats {
        Stats::new(1, 1.0)
    }

    fn speed(&self) -> f32 {
        vw() * (0.32 + self.time * 0.005).min(0.72)
    }

    fn spawn(&mut self) {
        let x = vw() + 60.0;
        let (top, bottom) = (surface(), seabed());
        let water = bottom - top;
        let d = (self.time / 20.0).min(4.0);
        let gap = PLAYER_SIZE * (3.6 - 0.25 * d).max(2.6);
        let roll = rand::gen_range(0.0, 1.0);
        let gap_center;
        if roll < 0.45 {
            // A gate: kelp from below, a hook from above.
            gap_center = rand::gen_range(top + gap / 2.0 + 30.0, bottom - gap / 2.0 - 30.0);
            self.hazards.push(Hazard {
                kind: HazardKind::Kelp,
                x,
                y: gap_center + gap / 2.0,
                phase: rand::gen_range(0.0, 6.0),
                passed: false,
            });
            self.hazards.push(Hazard {
                kind: HazardKind::Hook,
                x: x + 10.0,
                y: gap_center - gap / 2.0,
                phase: 0.0,
                passed: false,
            });
        } else if roll < 0.8 {
            // A small school of jellyfish.
            let n = 1 + rand::gen_range(0, 2 + d as usize / 2);
            gap_center = rand::gen_range(top + water * 0.2, bottom - water * 0.2);
            for i in 0..n {
                let y = rand::gen_range(top + 50.0, bottom - 60.0);
                if (y - gap_center).abs() < gap * 0.5 {
                    continue;
                }
                self.hazards.push(Hazard {
                    kind: HazardKind::Jelly,
                    x: x + i as f32 * 90.0,
                    y,
                    phase: rand::gen_range(0.0, 6.0),
                    passed: false,
                });
            }
        } else {
            gap_center = rand::gen_range(top + water * 0.3, bottom - water * 0.3);
            let kind = if rand::gen_range(0, 2) == 0 {
                HazardKind::Kelp
            } else {
                HazardKind::Hook
            };
            let y = if kind == HazardKind::Kelp {
                gap_center + gap * 0.6
            } else {
                gap_center - gap * 0.6
            };
            self.hazards.push(Hazard {
                kind,
                x,
                y,
                phase: rand::gen_range(0.0, 6.0),
                passed: false,
            });
        }
        // A wavy line of coins through the safe lane.
        let n = 6;
        for i in 0..n {
            let cx = x + 120.0 + i as f32 * 55.0;
            let cy = gap_center + (i as f32 * 0.9).sin() * 30.0;
            self.pickups.push(Pickup::coin(vec2(cx, cy)));
        }
        if rand::gen_range(0.0, 1.0) < 0.1 {
            self.pickups.push(Pickup::power(
                vec2(x + 120.0 + n as f32 * 55.0 + 40.0, gap_center),
                Power::common(),
            ));
        }
        self.next_spawn = self.speed() * rand::gen_range(1.1, 1.6) * (1.0 - 0.06 * d) + 120.0;
    }

    fn scroll_world(&mut self, dt: f32) {
        let dx = self.speed() * dt;
        self.distance += dx;
        self.scroll += dx;
        self.next_spawn -= dx;
        for h in &mut self.hazards {
            h.x -= dx;
        }
        self.hazards.retain(|h| h.x > -80.0);
        for p in &mut self.pickups {
            p.pos.x -= dx;
            p.age += dt;
        }
        self.pickups.retain(|p| p.pos.x > -40.0);
        for b in &mut self.bubbles {
            b.0.y -= 60.0 * dt;
            b.0.x -= dx * 0.6;
            b.1 -= dt;
        }
        self.bubbles.retain(|b| b.1 > 0.0);
    }
}

impl Scape for Sea {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>) {
        self.time += dt;
        let accel = if input.hold { -SWIM } else { SINK };
        self.vy = (self.vy + accel * dt).clamp(-MAX_VY, MAX_VY);
        self.y += self.vy * dt;
        let (lo, hi) = (surface() + PLAYER_SIZE / 2.0, seabed() - PLAYER_SIZE / 2.0);
        if self.y < lo || self.y > hi {
            // The surface and seabed are soft: no damage, just a gentle bump.
            self.y = self.y.clamp(lo, hi);
            self.vy *= -0.2;
        }

        self.bubble_timer -= dt;
        if input.hold && self.bubble_timer <= 0.0 {
            self.bubble_timer = 0.08;
            let p = vec2(
                player_x() - PLAYER_SIZE * 0.4,
                self.y + rand::gen_range(-8.0, 8.0),
            );
            self.bubbles.push((p, rand::gen_range(0.6, 1.0)));
        }

        self.scroll_world(dt);
        if self.next_spawn <= 0.0 {
            self.spawn();
        }

        let me = vec2(player_x(), self.y);
        let reach = PLAYER_SIZE * 0.32;
        let t = self.time;
        let mut hit = None;
        for (i, h) in self.hazards.iter_mut().enumerate() {
            let parts = h.hurts(t);
            if stats.invuln <= 0.0
                && hit.is_none()
                && parts.iter().any(|(c, r)| c.distance(me) < r + reach)
            {
                hit = Some(i);
            }
            if !h.passed && h.x < me.x - 20.0 {
                h.passed = true;
                let closest = parts
                    .iter()
                    .map(|(c, r)| c.distance(me) - r)
                    .fold(f32::MAX, f32::min);
                if closest < reach + 26.0 && stats.invuln <= 0.0 {
                    events.push(Event::Close(me - vec2(0.0, 60.0)));
                }
            }
        }
        if let Some(i) = hit {
            let kind = self.hazards.remove(i).kind;
            let color = match kind {
                HazardKind::Jelly => Color::new(1.0, 0.5, 0.85, 1.0),
                HazardKind::Kelp => Color::new(0.3, 0.8, 0.4, 1.0),
                HazardKind::Hook => Color::new(0.8, 0.8, 0.85, 1.0),
            };
            events.push(Event::Hit { at: me, color });
        }
        super::collect_pickups(&mut self.pickups, me, PLAYER_SIZE * 0.5 + 18.0, events);

        stats.label = format!("{} m", (self.distance / 50.0) as u32);
    }

    fn coast(&mut self, dt: f32) {
        self.time += dt;
        self.scroll_world(dt);
    }

    fn draw(&self, look: &Look, _stats: &Stats, show_player: bool) {
        let (w, t) = (vw(), self.time);
        // Light rays from the surface.
        for i in 0..4 {
            let x = ((i as f32 * 0.31 + 0.1) * w - self.scroll * 0.1).rem_euclid(w + 200.0) - 100.0;
            let sway = (t * 0.5 + i as f32).sin() * 30.0;
            draw_triangle(
                vec2(x - 30.0, surface()),
                vec2(x + 30.0, surface()),
                vec2(x + 120.0 + sway, seabed()),
                Color::new(1.0, 1.0, 1.0, 0.04),
            );
        }
        // Surface waves.
        draw_rectangle(
            -30.0,
            -30.0,
            w + 60.0,
            surface() + 30.0,
            Color::new(0.55, 0.85, 1.0, 0.35),
        );
        let mut x = -((self.scroll * 0.5) % 40.0);
        while x < w + 40.0 {
            draw_circle(x, surface(), 16.0, Color::new(0.55, 0.85, 1.0, 0.35));
            x += 40.0;
        }

        // Sandy seabed with pebbles.
        let sand = Color::new(0.85, 0.72, 0.45, 1.0);
        draw_rectangle(-30.0, seabed(), w + 60.0, vh() - seabed() + 30.0, sand);
        let mut x = -(self.scroll % 90.0);
        while x < w + 90.0 {
            draw_ellipse(
                x + 30.0,
                seabed() + 22.0,
                10.0,
                5.0,
                0.0,
                Color::new(0.7, 0.58, 0.35, 1.0),
            );
            draw_ellipse(
                x + 70.0,
                seabed() + 44.0,
                6.0,
                3.0,
                0.0,
                Color::new(0.7, 0.58, 0.35, 1.0),
            );
            x += 90.0;
        }

        let me = vec2(player_x(), self.y);
        for h in &self.hazards {
            match h.kind {
                HazardKind::Jelly => {
                    let c = h.jelly_center(t);
                    let pink = Color::new(1.0, 0.55, 0.85, 0.9);
                    for k in 0..4 {
                        let x0 = c.x - 15.0 + k as f32 * 10.0;
                        for s in 0..5 {
                            let y = c.y + 8.0 + s as f32 * 9.0;
                            let wave = (t * 5.0 + s as f32 + k as f32).sin() * 3.0;
                            draw_circle(x0 + wave, y, 3.5, fade(pink, 0.7));
                        }
                    }
                    draw_circle(c.x, c.y, 30.0, fade(pink, 0.18));
                    // The dome: a half disk turned upside down.
                    art::with_transform(c + vec2(0.0, 6.0), PI, false, || {
                        art::half_disk(Vec2::ZERO, 24.0, pink)
                    });
                    draw_rectangle(c.x - 24.0, c.y + 4.0, 48.0, 4.0, pink);
                    art::grumpy_face(c - vec2(0.0, 4.0), 30.0, 40.0, me);
                }
                HazardKind::Kelp => {
                    let green = Color::new(0.25, 0.7, 0.35, 1.0);
                    for (c, r) in h.hurts(t) {
                        draw_circle(c.x, c.y, r, green);
                        draw_ellipse(
                            c.x + r * 0.8,
                            c.y - 4.0,
                            r * 0.7,
                            r * 0.3,
                            -30.0,
                            Color::new(0.35, 0.8, 0.45, 1.0),
                        );
                    }
                }
                HazardKind::Hook => {
                    let gray = Color::new(0.8, 0.82, 0.88, 1.0);
                    draw_line(
                        h.x,
                        -30.0,
                        h.x,
                        h.y - 26.0,
                        3.0,
                        Color::new(1.0, 1.0, 1.0, 0.6),
                    );
                    // A "J": shank, round bottom, barbed tip.
                    draw_line(h.x, h.y - 28.0, h.x, h.y - 14.0, 5.0, gray);
                    let c = vec2(h.x - 11.0, h.y - 14.0);
                    let mut prev = c + vec2(11.0, 0.0);
                    for k in 1..=8 {
                        let a = std::f32::consts::PI * k as f32 / 8.0;
                        let p = c + vec2(a.cos(), a.sin()) * 11.0;
                        draw_line(prev.x, prev.y, p.x, p.y, 5.0, gray);
                        prev = p;
                    }
                    draw_line(prev.x, prev.y, prev.x, prev.y - 8.0, 5.0, gray);
                    draw_triangle(
                        vec2(prev.x - 3.0, prev.y - 14.0),
                        vec2(prev.x + 7.0, prev.y - 6.0),
                        vec2(prev.x - 3.0, prev.y - 4.0),
                        gray,
                    );
                    draw_circle(h.x, h.y - 30.0, 5.0, Color::new(1.0, 0.3, 0.3, 1.0));
                }
            }
        }

        for p in &self.pickups {
            p.draw();
        }
        for (pos, life) in &self.bubbles {
            draw_circle_lines(
                pos.x,
                pos.y,
                4.0 + (1.0 - life) * 4.0,
                1.5,
                Color::new(1.0, 1.0, 1.0, life * 0.7),
            );
        }

        if show_player {
            let mut look = *look;
            look.gaze = vec2(1.0, self.vy / MAX_VY);
            let tilt = self.vy / MAX_VY * 0.35;
            let swim = (look.clock * 9.0).sin() * 0.04;
            look.stretch = vec2(1.0 + swim, 1.0 - swim);
            art::with_transform(me, tilt, false, || art::draw_animal(&look, PLAYER_SIZE));
        }
    }

    fn player_pos(&self) -> Vec2 {
        vec2(player_x(), self.y)
    }

    fn player_size(&self) -> f32 {
        PLAYER_SIZE
    }

    fn danger(&self) -> bool {
        let px = player_x();
        self.hazards.iter().any(|h| {
            let (top, bottom) = h.top_bottom(self.time);
            h.x > px && h.x - px < 170.0 && self.y > top - 40.0 && self.y < bottom + 40.0
        })
    }

    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32)) {
        ((0.1, 0.45, 0.7), (0.02, 0.12, 0.3))
    }

    fn accent(&self) -> Color {
        Color::new(0.4, 0.85, 1.0, 1.0)
    }

    fn bg_speed(&self) -> f32 {
        -25.0
    }

    fn hints(&self) -> [(&'static str, &'static str); 2] {
        [
            (
                "Hold to swim up, let go to sink",
                "Hold Space / Up to swim up",
            ),
            (
                "Weave past jellyfish and hooks!",
                "Weave past jellyfish and hooks!",
            ),
        ]
    }
}
