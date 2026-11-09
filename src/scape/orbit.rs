//! Orbit Scape: circle a tiny planet and tap to hop between the inner and
//! outer orbit, dodging the asteroids parked along them.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use super::{Event, Input, Pickup, Power, Scape, Stats};
use crate::art::{self, Look};
use crate::fx::fade;
use crate::ui::{shade, vh, vw};

const PLAYER_SIZE: f32 = 46.0;
const ROCK_R: f32 = 22.0;
/// Seconds a rock takes to appear; it can't hurt you before that.
const APPEAR: f32 = 0.45;
/// The run starts at the top of the planet.
const START_ANGLE: f32 = -PI / 2.0;

fn center() -> Vec2 {
    vec2(vw() / 2.0, vh() * 0.5)
}

fn unit() -> f32 {
    vw().min(vh() * 1.1)
}

fn ring_radius(ring: usize) -> f32 {
    unit() * if ring == 0 { 0.2 } else { 0.34 }
}

fn planet_radius() -> f32 {
    unit() * 0.1
}

/// Signed angular distance from `b` to `a`, in (-PI, PI].
fn ang_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(TAU);
    if d > PI {
        d - TAU
    } else {
        d
    }
}

fn polar(angle: f32, r: f32) -> Vec2 {
    center() + vec2(angle.cos(), angle.sin()) * r
}

struct Rock {
    angle: f32,
    ring: usize,
    age: f32,
    passed: bool,
    seed: f32,
}

/// A coin or power-up parked on a ring; `pos.x` is its angle.
struct Item {
    pickup: Pickup,
    ring: usize,
}

pub struct Orbit {
    time: f32,
    angle: f32,
    ring: usize,
    radius: f32,
    prev_ring: usize,
    last_switch: f32,
    laps: u32,
    rocks: Vec<Rock>,
    items: Vec<Item>,
    spawn_timer: f32,
    coin_timer: f32,
}

impl Orbit {
    pub fn new() -> Self {
        Orbit {
            time: 0.0,
            angle: START_ANGLE,
            ring: 1,
            radius: ring_radius(1),
            prev_ring: 1,
            last_switch: -10.0,
            laps: 0,
            rocks: Vec::new(),
            items: Vec::new(),
            spawn_timer: 1.0,
            coin_timer: 0.3,
        }
    }

    pub fn stats(&self) -> Stats {
        Stats::new(1, 1.0)
    }

    fn speed(&self) -> f32 {
        (1.5 + self.time * 0.025).min(3.4)
    }

    fn player_screen(&self) -> Vec2 {
        polar(self.angle, self.radius)
    }

    fn spawn_rock(&mut self) {
        let ahead = rand::gen_range(1.8, 3.0);
        let angle = self.angle + ahead;
        let mut ring = rand::gen_range(0, 2);
        // Never park rocks on both rings at nearly the same spot: always leave a way through.
        let window = 0.35 + self.speed() * 0.12;
        for r in &self.rocks {
            if ang_diff(r.angle, angle).abs() < window && r.ring != ring {
                ring = r.ring;
            }
        }
        if self
            .rocks
            .iter()
            .any(|r| ang_diff(r.angle, angle).abs() < 0.3)
        {
            return;
        }
        self.rocks.push(Rock {
            angle,
            ring,
            age: 0.0,
            passed: false,
            seed: rand::gen_range(0.0, 10.0),
        });
    }

    fn spawn_coins(&mut self) {
        let ring = rand::gen_range(0, 2);
        let start = self.angle + rand::gen_range(2.0, 3.0);
        for i in 0..5 {
            let a = start + i as f32 * 0.17;
            if self
                .rocks
                .iter()
                .any(|r| r.ring == ring && ang_diff(r.angle, a).abs() < 0.25)
            {
                continue;
            }
            self.items.push(Item {
                pickup: Pickup::coin(vec2(a, 0.0)),
                ring,
            });
        }
        if rand::gen_range(0.0, 1.0) < 0.1 {
            let a = start + 1.0;
            self.items.push(Item {
                pickup: Pickup::power(vec2(a, 0.0), Power::common()),
                ring: 1 - ring,
            });
        }
    }

    fn tick(&mut self, dt: f32) {
        for r in &mut self.rocks {
            r.age += dt;
        }
        for it in &mut self.items {
            it.pickup.age += dt;
        }
        // Drop things well behind the player.
        let me = self.angle;
        self.rocks
            .retain(|r| ang_diff(r.angle, me) > -1.6 || r.age < 1.0);
        self.items.retain(|i| ang_diff(i.pickup.pos.x, me) > -1.0);
    }
}

impl Scape for Orbit {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>) {
        self.time += dt;
        if input.action {
            self.prev_ring = self.ring;
            self.ring = 1 - self.ring;
            self.last_switch = self.time;
        }
        let target = ring_radius(self.ring);
        self.radius += (target - self.radius) * (dt * 16.0).min(1.0);

        self.angle += self.speed() * dt;
        self.laps = ((self.angle - START_ANGLE) / TAU) as u32;

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_rock();
            self.spawn_timer = (0.75 - self.time * 0.006).max(0.32) * rand::gen_range(0.8, 1.2);
        }
        self.coin_timer -= dt;
        if self.coin_timer <= 0.0 {
            self.spawn_coins();
            self.coin_timer = rand::gen_range(1.4, 2.4);
        }
        self.tick(dt);

        let me = self.player_screen();
        let mut hit = false;
        for r in &mut self.rocks {
            let pos = polar(r.angle, ring_radius(r.ring));
            if r.age >= APPEAR
                && !hit
                && stats.invuln <= 0.0
                && pos.distance(me) < ROCK_R + PLAYER_SIZE * 0.3
            {
                hit = true;
                r.passed = true;
                r.age = -100.0;
            }
            if !r.passed && ang_diff(r.angle, self.angle) < 0.0 {
                r.passed = true;
                // A close call: we hopped off this rock's ring just in time.
                if r.ring != self.ring
                    && r.ring == self.prev_ring
                    && self.time - self.last_switch < 0.6
                {
                    events.push(Event::Close(me - vec2(0.0, 60.0)));
                }
            }
        }
        if hit {
            self.rocks.retain(|r| r.age > -50.0);
            events.push(Event::Hit {
                at: me,
                color: Color::new(0.6, 0.55, 0.6, 1.0),
            });
        }

        self.items.retain(|it| {
            let pos = polar(it.pickup.pos.x, ring_radius(it.ring));
            if pos.distance(me) < PLAYER_SIZE * 0.5 + 18.0 {
                let mut p = Pickup::coin(pos);
                p.power = it.pickup.power;
                events.push(p.event());
                false
            } else {
                true
            }
        });

        stats.label = format!("LAP {}", self.laps + 1);
    }

    fn coast(&mut self, dt: f32) {
        self.time += dt;
        self.tick(dt);
    }

    fn draw(&self, look: &Look, _stats: &Stats, show_player: bool) {
        let c = center();
        // Orbit rings: the current one glows brighter.
        for ring in 0..2 {
            let r = ring_radius(ring);
            let a = if ring == self.ring { 0.55 } else { 0.25 };
            draw_circle_lines(c.x, c.y, r, 3.0, Color::new(1.0, 0.8, 0.95, a));
        }

        // The planet, with a friendly face.
        let pr = planet_radius();
        draw_circle(c.x, c.y, pr * 1.35, Color::new(1.0, 0.6, 0.8, 0.12));
        draw_circle(c.x, c.y, pr, Color::new(0.55, 0.3, 0.75, 1.0));
        draw_circle(
            c.x - pr * 0.1,
            c.y - pr * 0.1,
            pr * 0.88,
            Color::new(0.7, 0.42, 0.9, 1.0),
        );
        draw_circle(
            c.x + pr * 0.4,
            c.y + pr * 0.35,
            pr * 0.16,
            Color::new(0.55, 0.3, 0.75, 1.0),
        );
        draw_circle(
            c.x - pr * 0.45,
            c.y + pr * 0.2,
            pr * 0.1,
            Color::new(0.55, 0.3, 0.75, 1.0),
        );
        draw_circle(c.x - pr * 0.3, c.y - pr * 0.1, pr * 0.08, art::INK);
        draw_circle(c.x + pr * 0.3, c.y - pr * 0.1, pr * 0.08, art::INK);
        art::half_disk(vec2(c.x, c.y + pr * 0.12), pr * 0.2, art::INK);

        for it in &self.items {
            let pos = polar(it.pickup.pos.x, ring_radius(it.ring));
            let mut p = Pickup::coin(pos);
            p.age = it.pickup.age;
            p.power = it.pickup.power;
            p.draw();
        }

        let me = self.player_screen();
        for r in &self.rocks {
            if r.age < -50.0 {
                continue;
            }
            let pos = polar(r.angle, ring_radius(r.ring));
            let appear = (r.age / APPEAR).clamp(0.0, 1.0);
            let s = ROCK_R * (0.4 + 0.6 * appear);
            let gray = Color::new(0.62, 0.58, 0.66, appear);
            // Lumpy outline from a few overlapping circles.
            for k in 0..5 {
                let a = r.seed + k as f32 * 1.3;
                draw_circle(
                    pos.x + a.cos() * s * 0.35,
                    pos.y + a.sin() * s * 0.35,
                    s * 0.75,
                    shade(gray, 0.6),
                );
            }
            draw_circle(pos.x, pos.y, s * 0.95, gray);
            draw_circle(
                pos.x + s * 0.35,
                pos.y - s * 0.3,
                s * 0.18,
                shade(gray, 0.75),
            );
            if appear > 0.8 {
                art::grumpy_face(pos + vec2(0.0, 2.0), s * 1.3, s * 1.6, me);
            } else {
                draw_circle_lines(
                    pos.x,
                    pos.y,
                    s * 1.4,
                    2.0,
                    fade(WHITE, 0.6 * (1.0 - appear)),
                );
            }
        }

        if show_player {
            let mut look = *look;
            look.gaze = vec2(1.0, 0.0);
            // Feet point at the planet.
            art::with_transform(me, self.angle + PI / 2.0, false, || {
                art::draw_animal(&look, PLAYER_SIZE);
            });
        }
    }

    fn player_pos(&self) -> Vec2 {
        self.player_screen()
    }

    fn player_size(&self) -> f32 {
        PLAYER_SIZE
    }

    fn danger(&self) -> bool {
        self.rocks
            .iter()
            .any(|r| r.ring == self.ring && (0.0..0.7).contains(&ang_diff(r.angle, self.angle)))
    }

    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32)) {
        ((0.06, 0.03, 0.15), (0.18, 0.06, 0.25))
    }

    fn accent(&self) -> Color {
        Color::new(1.0, 0.45, 0.75, 1.0)
    }

    fn bg_speed(&self) -> f32 {
        12.0
    }

    fn hints(&self) -> [(&'static str, &'static str); 2] {
        [
            (
                "Tap to hop between orbits",
                "Space / Up to hop between orbits",
            ),
            ("Dodge the grumpy rocks!", "Dodge the grumpy rocks!"),
        ]
    }
}
