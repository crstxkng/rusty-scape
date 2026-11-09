//! Gravity Scape: run down an endless corridor and tap to flip gravity
//! between the floor and the ceiling, dodging the spikes.

use macroquad::prelude::*;

use super::{Event, Input, Pickup, Power, Scape, Stats};
use crate::art::{self, Look};
use crate::fx::fade;
use crate::ui::{shade, vh, vw};

const PLAYER_SIZE: f32 = 50.0;
const GRAVITY: f32 = 3200.0;
const SPIKE_W: f32 = 34.0;
const SPIKE_H: f32 = 36.0;

fn corridor() -> (f32, f32) {
    let h = (vh() * 0.56).min(440.0);
    let c = vh() * 0.52;
    (c - h / 2.0, c + h / 2.0)
}

fn player_x() -> f32 {
    vw() * 0.25
}

struct Spikes {
    x: f32,
    count: usize,
    on_ceiling: bool,
    passed: bool,
}

impl Spikes {
    fn rect(&self) -> Rect {
        let (top, bottom) = corridor();
        let w = self.count as f32 * SPIKE_W;
        // The deadly part is a bit smaller than the drawing.
        let (inset, h) = (6.0, SPIKE_H * 0.75);
        if self.on_ceiling {
            Rect::new(self.x + inset, top, w - inset * 2.0, h)
        } else {
            Rect::new(self.x + inset, bottom - h, w - inset * 2.0, h)
        }
    }
}

pub struct Gravity {
    time: f32,
    distance: f32,
    y: f32,
    vy: f32,
    /// 1 when falling toward the floor, -1 toward the ceiling.
    down: f32,
    last_flip: f32,
    spikes: Vec<Spikes>,
    pickups: Vec<Pickup>,
    next_spawn: f32,
    scroll: f32,
}

impl Gravity {
    pub fn new() -> Self {
        let (_, bottom) = corridor();
        Gravity {
            time: 0.0,
            distance: 0.0,
            y: bottom - PLAYER_SIZE / 2.0,
            vy: 0.0,
            down: 1.0,
            last_flip: -10.0,
            spikes: Vec::new(),
            pickups: Vec::new(),
            next_spawn: vw() * 0.9,
            scroll: 0.0,
        }
    }

    pub fn stats(&self) -> Stats {
        Stats::new(1, 1.0)
    }

    fn speed(&self) -> f32 {
        vw() * (0.42 + self.time * 0.006).min(0.9)
    }

    fn on_ceiling(&self) -> bool {
        self.down < 0.0
    }

    fn hitbox(&self) -> Rect {
        let s = PLAYER_SIZE * 0.6;
        Rect::new(player_x() - s / 2.0, self.y - s / 2.0, s, s)
    }

    /// Adds the next obstacle group off the right edge, with coins in the gaps.
    fn spawn(&mut self) {
        let x = vw() + 40.0;
        let (top, bottom) = corridor();
        let mid = (top + bottom) / 2.0;
        let d = (self.time / 20.0).min(4.0);
        let roll = rand::gen_range(0.0, 1.0);
        let gap_time = rand::gen_range(0.75, 1.15) - 0.06 * d;
        let mut width = 0.0;
        if roll < 0.35 {
            let count = rand::gen_range(1, 3 + d as usize);
            let ceiling = rand::gen_range(0, 2) == 0;
            self.spikes.push(Spikes {
                x,
                count,
                on_ceiling: ceiling,
                passed: false,
            });
            width = count as f32 * SPIKE_W;
        } else if roll < 0.7 {
            // Floor then ceiling: flip, then flip back.
            let count = rand::gen_range(2, 4);
            let offset = self.speed() * (0.5 - 0.03 * d).max(0.36);
            let first = rand::gen_range(0, 2) == 0;
            self.spikes.push(Spikes {
                x,
                count,
                on_ceiling: first,
                passed: false,
            });
            self.spikes.push(Spikes {
                x: x + offset,
                count,
                on_ceiling: !first,
                passed: false,
            });
            width = offset + count as f32 * SPIKE_W;
        } else {
            // A long strip on one side: stay on the other one.
            let count = rand::gen_range(5, 9);
            let ceiling = rand::gen_range(0, 2) == 0;
            self.spikes.push(Spikes {
                x,
                count,
                on_ceiling: ceiling,
                passed: false,
            });
            width = width.max(count as f32 * SPIKE_W);
            // Coins along the safe side.
            let y = if ceiling {
                bottom - PLAYER_SIZE / 2.0
            } else {
                top + PLAYER_SIZE / 2.0
            };
            for i in 0..count / 2 {
                self.pickups
                    .push(Pickup::coin(vec2(x + 20.0 + i as f32 * 70.0, y)));
            }
        }
        // An arc of coins through the middle of the gap: grab them while flipping.
        let gap = self.speed() * gap_time;
        let n = 5;
        for i in 0..n {
            let t = i as f32 / (n - 1) as f32;
            let cx = x + width + gap * (0.2 + 0.6 * t);
            let cy = mid
                + (t * std::f32::consts::PI).sin()
                    * (bottom - top)
                    * 0.25
                    * if rand::gen_range(0, 2) == 0 {
                        1.0
                    } else {
                        -1.0
                    };
            self.pickups.push(Pickup::coin(vec2(cx, cy)));
        }
        if rand::gen_range(0.0, 1.0) < 0.1 {
            self.pickups.push(Pickup::power(
                vec2(x + width + gap * 0.5, mid),
                Power::common(),
            ));
        }
        self.next_spawn = width + gap;
    }

    fn scroll_world(&mut self, dt: f32) {
        let dx = self.speed() * dt;
        self.distance += dx;
        self.scroll += dx;
        for s in &mut self.spikes {
            s.x -= dx;
        }
        self.spikes
            .retain(|s| s.x + s.count as f32 * SPIKE_W > -50.0);
        for p in &mut self.pickups {
            p.pos.x -= dx;
            p.age += dt;
        }
        self.pickups.retain(|p| p.pos.x > -50.0);
        self.next_spawn -= dx;
    }
}

impl Scape for Gravity {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>) {
        self.time += dt;
        if input.action {
            self.down = -self.down;
            self.last_flip = self.time;
            // A little kick so the flip feels instant.
            self.vy = self.down * 350.0;
        }
        let (top, bottom) = corridor();
        self.vy += self.down * GRAVITY * dt;
        self.y += self.vy * dt;
        let (lo, hi) = (top + PLAYER_SIZE / 2.0, bottom - PLAYER_SIZE / 2.0);
        if self.y > hi {
            self.y = hi;
            self.vy = 0.0;
        } else if self.y < lo {
            self.y = lo;
            self.vy = 0.0;
        }

        self.scroll_world(dt);
        if self.next_spawn <= 0.0 {
            self.spawn();
        }

        let hb = self.hitbox();
        let me = vec2(player_x(), self.y);
        let on_ceiling = self.on_ceiling();
        let just_flipped = self.time - self.last_flip < 0.35;
        let mut hit = None;
        for (i, s) in self.spikes.iter_mut().enumerate() {
            let r = s.rect();
            if stats.invuln <= 0.0 && hit.is_none() && r.overlaps(&hb) {
                hit = Some(i);
            }
            if !s.passed && r.right() < hb.x {
                s.passed = true;
                // Flipped away from these spikes at the last moment.
                if s.on_ceiling != on_ceiling && just_flipped {
                    events.push(Event::Close(me - vec2(0.0, 60.0 * self.down)));
                }
            }
        }
        if let Some(i) = hit {
            self.spikes.remove(i);
            events.push(Event::Hit {
                at: me,
                color: Color::new(0.85, 0.85, 0.9, 1.0),
            });
        }
        super::collect_pickups(&mut self.pickups, me, PLAYER_SIZE * 0.5 + 18.0, events);

        stats.label = format!("{} m", (self.distance / 50.0) as u32);
    }

    fn coast(&mut self, dt: f32) {
        self.time += dt;
        self.scroll_world(dt);
    }

    fn draw(&self, look: &Look, _stats: &Stats, show_player: bool) {
        let (top, bottom) = corridor();
        let w = vw();
        let wall = Color::new(0.08, 0.2, 0.17, 1.0);
        let edge = Color::new(0.4, 1.0, 0.55, 1.0);
        draw_rectangle(-30.0, -30.0, w + 60.0, top + 30.0, wall);
        draw_rectangle(-30.0, bottom, w + 60.0, vh() - bottom + 30.0, wall);
        // Scrolling stripes sell the speed.
        let stripe = 80.0;
        let off = self.scroll % stripe;
        let mut x = -off;
        while x < w + stripe {
            draw_rectangle(x, top - 14.0, stripe / 2.0, 10.0, fade(edge, 0.25));
            draw_rectangle(x, bottom + 4.0, stripe / 2.0, 10.0, fade(edge, 0.25));
            x += stripe;
        }
        draw_rectangle(-30.0, top - 4.0, w + 60.0, 4.0, edge);
        draw_rectangle(-30.0, bottom, w + 60.0, 4.0, edge);

        let metal = Color::new(0.85, 0.87, 0.95, 1.0);
        for s in &self.spikes {
            for k in 0..s.count {
                let x0 = s.x + k as f32 * SPIKE_W;
                let (base, tip) = if s.on_ceiling {
                    (top, top + SPIKE_H)
                } else {
                    (bottom, bottom - SPIKE_H)
                };
                draw_triangle(
                    vec2(x0, base),
                    vec2(x0 + SPIKE_W, base),
                    vec2(x0 + SPIKE_W / 2.0, tip),
                    shade(metal, 0.6),
                );
                draw_triangle(
                    vec2(x0 + 4.0, base),
                    vec2(x0 + SPIKE_W - 4.0, base),
                    vec2(x0 + SPIKE_W / 2.0, tip + (base - tip).signum() * 6.0),
                    metal,
                );
            }
        }
        for p in &self.pickups {
            p.draw();
        }

        if show_player {
            let mut look = *look;
            let airborne = self.vy.abs() > 1.0;
            let bob = if airborne {
                0.0
            } else {
                (look.clock * 18.0).sin()
            };
            look.stretch = vec2(1.0 + bob * 0.05, 1.0 - bob * 0.05);
            look.gaze = vec2(1.0, 0.0);
            art::with_transform(vec2(player_x(), self.y), 0.0, self.on_ceiling(), || {
                art::draw_animal(&look, PLAYER_SIZE);
            });
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
        self.spikes.iter().any(|s| {
            s.on_ceiling == self.on_ceiling() && s.x > px && s.x - px < self.speed() * 0.35
        })
    }

    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32)) {
        ((0.05, 0.16, 0.14), (0.1, 0.3, 0.22))
    }

    fn accent(&self) -> Color {
        Color::new(0.4, 1.0, 0.55, 1.0)
    }

    fn bg_speed(&self) -> f32 {
        30.0
    }

    fn hints(&self) -> [(&'static str, &'static str); 2] {
        [
            ("Tap to flip gravity", "Space / Up to flip gravity"),
            ("Stay away from the spikes!", "Stay away from the spikes!"),
        ]
    }
}
