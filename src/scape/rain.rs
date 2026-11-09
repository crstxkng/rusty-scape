//! Rain Scape: blocks fall from the sky, you run left and right to dodge.

use std::f32::consts::PI;

use macroquad::prelude::*;

use super::{Event, Input, Pickup, Power, Scape, Stats};
use crate::art::{self, Look};
use crate::fx::{fade, rgb};
use crate::modes::{Mode, ModeConfig};
use crate::ui::{self, shade, vh, vw};

const PLAYER_SIZE: f32 = 58.0;
/// Shrinks the player's hitbox so near misses feel fair.
const HITBOX_INSET: f32 = 10.0;
const DASH_TIME: f32 = 0.13;
const DASH_COOLDOWN: f32 = 0.8;
const NEAR_MISS_DIST: f32 = 26.0;
const SLOW_TIME: f32 = 4.0;
const MAGNET_TIME: f32 = 7.0;
const MAGNET_RANGE: f32 = 280.0;
const TRAIL_LEN: usize = 10;
/// Finger movement is amplified a little so small drags cover the screen.
const DRAG_GAIN: f32 = 1.35;
/// Seconds a falling block is telegraphed above the screen before it enters.
const WARN_TIME: f32 = 0.45;

fn player_y() -> f32 {
    vh() - if ui::is_portrait() { 170.0 } else { 100.0 }
}

fn floor_y() -> f32 {
    player_y() + PLAYER_SIZE / 2.0 + 4.0
}

fn max_speed() -> f32 {
    vw() * 0.75
}

struct Obstacle {
    rect: Rect,
    vel: Vec2,
    color: Color,
    passed: bool,
}

/// A block queued by a wave pattern.
struct Scheduled {
    at: f32,
    x: f32,
    w: f32,
    h: f32,
    /// Spawn above wherever the player is at spawn time.
    aimed: bool,
}

struct Player {
    x: f32,
    vx: f32,
    /// Where touch / mouse dragging wants the player to be.
    target: Option<f32>,
    facing: f32,
    dash_time: f32,
    dash_cooldown: f32,
    trail: Vec<f32>,
    trail_clock: f32,
}

impl Player {
    fn hitbox(&self) -> Rect {
        let s = PLAYER_SIZE - HITBOX_INSET * 2.0;
        Rect::new(self.x - s / 2.0, player_y() - s / 2.0 + 4.0, s, s - 4.0)
    }

    fn center(&self) -> Vec2 {
        vec2(self.x, player_y())
    }

    fn is_dashing(&self) -> bool {
        self.dash_time > 0.0
    }
}

pub struct Rain {
    cfg: ModeConfig,
    time: f32,
    player: Player,
    obstacles: Vec<Obstacle>,
    pickups: Vec<Pickup>,
    schedule: Vec<Scheduled>,
    rain_timer: f32,
    snake_timer: f32,
    wave: u32,
    next_wave_at: f32,
    wave_end_at: Option<f32>,
    breather_until: f32,
    slow_time: f32,
    magnet: f32,
}

impl Rain {
    pub fn new(mode: Mode) -> Self {
        let cfg = mode.config();
        Rain {
            next_wave_at: cfg.wave_every * 0.65,
            cfg,
            time: 0.0,
            player: Player {
                x: vw() / 2.0,
                vx: 0.0,
                target: None,
                facing: 1.0,
                dash_time: 0.0,
                dash_cooldown: 0.0,
                trail: Vec::new(),
                trail_clock: 0.0,
            },
            obstacles: Vec::new(),
            pickups: Vec::new(),
            schedule: Vec::new(),
            rain_timer: 1.0,
            snake_timer: 4.0,
            wave: 0,
            wave_end_at: None,
            breather_until: 0.0,
            slow_time: 0.0,
            magnet: 0.0,
        }
    }

    pub fn stats(&self) -> Stats {
        let mut stats = Stats::new(self.cfg.lives, self.cfg.score_mult);
        stats.time_limit = self.cfg.time_limit;
        if self.cfg.time_limit.is_some() {
            stats.survival_rate = 5.0;
        }
        stats
    }

    /// Continuous difficulty: 0 at the start, ~+1 every 20 s at Classic pace.
    fn difficulty(&self) -> f32 {
        self.time / 20.0 * self.cfg.ramp
    }

    fn fall_time(&self) -> f32 {
        (self.cfg.fall_time / (1.0 + 0.11 * self.difficulty())).max(self.cfg.fall_time * 0.42)
    }

    fn fall_speed(&self) -> f32 {
        vh() / self.fall_time()
    }

    fn rain_interval(&self) -> f32 {
        // Narrow (portrait) screens get proportionally fewer blocks.
        let width_factor = 960.0 / vw();
        let base = self.cfg.spawn_interval * width_factor / (1.0 + 0.14 * self.difficulty());
        let onboarding = if self.time < 5.0 { 1.5 } else { 1.0 };
        base.max(0.13 * width_factor) * onboarding
    }

    fn obstacle_color(&self) -> Color {
        // Shift from the mode's block color toward hot red as waves climb.
        let t = (self.difficulty() / 8.0).min(1.0) * 0.7;
        let (r, g, b) = self.cfg.block;
        Color::new(
            r + (1.0 - r) * t,
            g * (1.0 - 0.6 * t),
            b * (1.0 - 0.6 * t),
            1.0,
        )
    }

    fn spawn_block(&mut self, cx: f32, w: f32, h: f32) {
        let speed =
            self.fall_speed() * rand::gen_range(0.92, 1.12) * if w > 140.0 { 0.85 } else { 1.0 };
        let vx = if self.cfg.drifting {
            rand::gen_range(-150.0, 150.0)
        } else {
            0.0
        };
        let x = (cx - w / 2.0).clamp(0.0, (vw() - w).max(0.0));
        let color = self.obstacle_color();
        self.obstacles.push(Obstacle {
            rect: Rect::new(x, -h - speed * WARN_TIME, w, h),
            vel: vec2(vx, speed),
            color,
            passed: false,
        });
    }

    fn update_player(&mut self, input: &Input, dt: f32, events: &mut Vec<Event>) {
        let max = max_speed();
        let accel = max * 9.0;
        let half = PLAYER_SIZE / 2.0;
        let p = &mut self.player;

        // Relative dragging: the finger moves a target, the player follows it.
        if input.dragging {
            let start = p.target.unwrap_or(p.x);
            p.target = Some((start + input.drag_dx * DRAG_GAIN).clamp(half, vw() - half));
        } else {
            p.target = None;
        }

        p.dash_cooldown = (p.dash_cooldown - dt).max(0.0);
        if input.dash && p.dash_cooldown <= 0.0 {
            if input.dir != 0.0 {
                p.facing = input.dir;
            } else if p.vx.abs() > 30.0 {
                p.facing = p.vx.signum();
            }
            p.dash_time = DASH_TIME;
            p.dash_cooldown = DASH_COOLDOWN;
            // Dragging keeps control after the dash: move the target along.
            if let Some(t) = p.target.as_mut() {
                *t += p.facing * vw() * 0.2;
            }
            events.push(Event::Dash(p.center()));
        }

        if p.dash_time > 0.0 {
            p.dash_time -= dt;
            p.vx = p.facing * vw() * 1.7;
        } else if input.dir != 0.0 {
            // Snappy turnarounds: braking is stronger than accelerating.
            let a = if p.vx * input.dir < 0.0 {
                accel * 2.0
            } else {
                accel
            };
            p.vx = (p.vx + input.dir * a * dt).clamp(-max, max);
            p.facing = input.dir;
        } else if let Some(target) = p.target {
            // Follow the finger closely but smoothly.
            let desired = ((target - p.x) * 22.0).clamp(-vw() * 2.4, vw() * 2.4);
            p.vx += (desired - p.vx) * (dt * 30.0).min(1.0);
            if desired.abs() > 40.0 {
                p.facing = desired.signum();
            }
        } else {
            let friction = accel * 1.6 * dt;
            p.vx = if p.vx.abs() <= friction {
                0.0
            } else {
                p.vx - p.vx.signum() * friction
            };
        }

        p.x += p.vx * dt;
        if p.x < half || p.x > vw() - half {
            p.x = p.x.clamp(half, vw() - half);
            // Bounce off the walls, a nod to the original lateral bounce.
            p.vx = -p.vx * 0.35;
        }

        // Sample the trail at a fixed rate so it looks the same at any FPS.
        p.trail_clock += dt;
        while p.trail_clock >= 1.0 / 60.0 {
            p.trail_clock -= 1.0 / 60.0;
            p.trail.insert(0, p.x);
            p.trail.truncate(TRAIL_LEN);
        }
    }

    /// The "director": alternates free rain, pattern waves and breathers.
    fn direct(&mut self, dt: f32, events: &mut Vec<Event>) {
        let now = self.time;

        // Release queued pattern blocks.
        let player_x = self.player.x;
        let mut i = 0;
        while i < self.schedule.len() {
            if self.schedule[i].at <= now {
                let s = self.schedule.swap_remove(i);
                let x = if s.aimed { player_x } else { s.x };
                self.spawn_block(x, s.w, s.h);
            } else {
                i += 1;
            }
        }

        // Wave cleared: reward, breathe, then rain again.
        if let Some(end) = self.wave_end_at {
            if now >= end && self.schedule.is_empty() {
                self.wave_cleared(events);
            }
            return;
        }

        if now >= self.next_wave_at {
            self.queue_wave(events);
            return;
        }

        if now < self.breather_until {
            return;
        }
        self.rain(dt);
    }

    fn rain(&mut self, dt: f32) {
        // Every few seconds, a snake of coins worth chasing.
        self.snake_timer -= dt;
        if self.snake_timer <= 0.0 {
            self.snake_timer = rand::gen_range(5.0, 8.0);
            let x0 = rand::gen_range(90.0, vw() - 90.0);
            for i in 0..6 {
                let x = x0 + (i as f32 * 0.9).sin() * 60.0;
                self.pickups
                    .push(Pickup::coin(vec2(x, -30.0 - i as f32 * 46.0)));
            }
        }

        self.rain_timer -= dt;
        if self.rain_timer > 0.0 {
            return;
        }
        self.rain_timer = self.rain_interval() * rand::gen_range(0.7, 1.3);
        let d = self.difficulty();
        let (w, h) = if self.cfg.drifting {
            (rand::gen_range(40.0, 190.0), rand::gen_range(40.0, 90.0))
        } else if d > 1.5 && rand::gen_range(0.0, 1.0) < 0.12 {
            // Occasional wide slab once things heat up.
            (rand::gen_range(150.0, 240.0f32).min(vw() * 0.5), 48.0)
        } else {
            let s = rand::gen_range(52.0, 84.0);
            (s, s * rand::gen_range(0.85, 1.15))
        };
        let cx = rand::gen_range(w / 2.0, vw() - w / 2.0);
        self.spawn_block(cx, w, h);

        let roll = rand::gen_range(0.0, 1.0);
        let pos = vec2(rand::gen_range(40.0, vw() - 40.0), -30.0);
        if roll < self.cfg.coin_chance {
            self.pickups.push(Pickup::coin(pos));
        } else if roll < self.cfg.coin_chance + self.cfg.powerup_chance {
            let power = match rand::gen_range(0, 4) {
                0 => Power::Shield,
                1 => Power::Slow,
                2 => Power::Magnet,
                _ => Power::CoinX2,
            };
            self.pickups.push(Pickup::power(pos, power));
        }
    }

    fn queue_wave(&mut self, events: &mut Vec<Event>) {
        self.wave += 1;
        let wave = self.wave;
        let t0 = self.time;
        let intensity = (wave as f32 - 1.0).min(8.0);
        let w = vw();
        let pattern = match wave {
            1 => 0,
            2 => 1,
            3 => 2,
            _ => rand::gen_range(0, 5),
        };
        let schedule = &mut self.schedule;
        let mut push = |at: f32, x: f32, bw: f32, bh: f32, aimed: bool| {
            schedule.push(Scheduled {
                at,
                x,
                w: bw,
                h: bh,
                aimed,
            });
        };
        let name;
        let last_at;
        match pattern {
            0 => {
                name = "WALLS";
                let walls = 1 + (wave / 2).min(3) as usize;
                let gap = PLAYER_SIZE * (2.8 - 0.1 * intensity).max(2.1);
                let spacing = (1.25 - 0.05 * intensity).max(0.8);
                let mut gx = rand::gen_range(gap, w - gap);
                for i in 0..walls {
                    let at = t0 + i as f32 * spacing;
                    if i > 0 {
                        // Keep the next gap reachable from the last one.
                        let reach = max_speed() * spacing * 0.6;
                        gx = (gx + rand::gen_range(-reach, reach)).clamp(gap, w - gap);
                    }
                    for (a, b) in [(0.0, gx - gap / 2.0), (gx + gap / 2.0, w)] {
                        let len = b - a;
                        if len < 10.0 {
                            continue;
                        }
                        let n = (len / 100.0).ceil().max(1.0);
                        let bw = len / n;
                        for k in 0..n as usize {
                            push(at, a + bw * (k as f32 + 0.5), bw - 4.0, 50.0, false);
                        }
                    }
                }
                last_at = t0 + (walls - 1) as f32 * spacing;
            }
            1 => {
                name = "ZIG-ZAG";
                let n = 6 + intensity as usize;
                let spacing = (0.42 - 0.02 * intensity).max(0.28);
                let bw = w * 0.44;
                for i in 0..n {
                    let side = if i % 2 == 0 { bw / 2.0 } else { w - bw / 2.0 };
                    push(t0 + i as f32 * spacing, side, bw, 50.0, false);
                }
                last_at = t0 + (n - 1) as f32 * spacing;
            }
            2 => {
                name = "SNIPER";
                let n = 4 + (intensity / 2.0) as usize;
                let spacing = (0.6 - 0.03 * intensity).max(0.36);
                for i in 0..n {
                    push(t0 + i as f32 * spacing, 0.0, 74.0, 74.0, true);
                }
                last_at = t0 + (n - 1) as f32 * spacing;
            }
            3 => {
                name = "STAIRS";
                let n = 7 + (intensity / 2.0) as usize;
                let spacing = (0.3 - 0.01 * intensity).max(0.2);
                let margin = 50.0;
                let ltr = rand::gen_range(0, 2) == 0;
                for i in 0..n {
                    let t = i as f32 / (n - 1) as f32;
                    let t = if ltr { t } else { 1.0 - t };
                    push(
                        t0 + i as f32 * spacing,
                        margin + (w - 2.0 * margin) * t,
                        96.0,
                        60.0,
                        false,
                    );
                }
                last_at = t0 + (n - 1) as f32 * spacing;
            }
            _ => {
                name = "STORM";
                let n = ((14.0 + 2.0 * intensity) * w / 960.0) as usize;
                let dur = 2.6;
                for i in 0..n {
                    let s = rand::gen_range(50.0, 80.0);
                    push(
                        t0 + dur * i as f32 / n as f32,
                        rand::gen_range(s / 2.0, w - s / 2.0),
                        s,
                        s,
                        false,
                    );
                }
                last_at = t0 + dur;
            }
        }
        self.wave_end_at = Some(last_at + self.fall_time() + WARN_TIME);
        events.push(Event::Banner(format!("WAVE {wave}"), name.to_string()));
    }

    fn wave_cleared(&mut self, events: &mut Vec<Event>) {
        self.wave_end_at = None;
        self.breather_until = self.time + 1.4;
        self.next_wave_at = self.time + self.cfg.wave_every;
        let bonus = (50.0 * self.wave as f32 * self.cfg.score_mult).round();
        // A rainbow of coins as a reward.
        let n = 9;
        for i in 0..n {
            let t = i as f32 / (n - 1) as f32;
            let x = vw() * (0.12 + 0.76 * t);
            let y = -30.0 - (t * PI).sin() * 90.0;
            self.pickups.push(Pickup::coin(vec2(x, y)));
        }
        events.push(Event::Bonus {
            text: format!("WAVE CLEAR +{bonus}"),
            points: bonus,
        });
    }

    fn move_world(&mut self, dt: f32) {
        let w = vw();
        for o in &mut self.obstacles {
            o.rect.x += o.vel.x * dt;
            o.rect.y += o.vel.y * dt;
            if o.rect.x < 0.0 || o.rect.right() > w {
                o.vel.x = -o.vel.x;
                o.rect.x = o.rect.x.clamp(0.0, (w - o.rect.w).max(0.0));
            }
        }
        let bottom = vh() + 10.0;
        self.obstacles.retain(|o| o.rect.y < bottom);

        let pickup_speed = 160.0 + self.fall_speed() * 0.2;
        let center = self.player.center();
        let magnet = self.magnet > 0.0;
        for p in &mut self.pickups {
            p.age += dt;
            p.pos.y += pickup_speed * dt;
            p.pos.x += (p.age * 3.0).sin() * 40.0 * dt;
            if magnet && p.power.is_none() {
                let to = center - p.pos;
                let d = to.length();
                if d < MAGNET_RANGE && d > 1.0 {
                    p.pos += to / d * 700.0 * dt;
                }
            }
        }
        self.pickups.retain(|p| p.pos.y < bottom + 30.0);
    }

    fn check_collisions(&mut self, stats: &Stats, events: &mut Vec<Event>) {
        let hitbox = self.player.hitbox();
        let phasing = self.player.is_dashing() || stats.invuln > 0.0;

        let mut hit = None;
        for (i, o) in self.obstacles.iter_mut().enumerate() {
            if o.rect.overlaps(&hitbox) {
                if !phasing {
                    hit = Some(i);
                    break;
                }
                continue;
            }
            // Near miss: the block passes the player's row within a whisker.
            if !o.passed && o.rect.y > hitbox.bottom() {
                o.passed = true;
                let gap = (o.rect.x - hitbox.right()).max(hitbox.x - o.rect.right());
                if (0.0..NEAR_MISS_DIST).contains(&gap) && !phasing {
                    events.push(Event::Close(vec2(hitbox.center().x, player_y() - 60.0)));
                }
            }
        }
        if let Some(i) = hit {
            let o = self.obstacles.swap_remove(i);
            events.push(Event::Hit {
                at: self.player.center(),
                color: o.color,
            });
        }

        let center = self.player.center();
        let reach = PLAYER_SIZE * 0.5 + 24.0;
        let mut i = 0;
        while i < self.pickups.len() {
            if self.pickups[i].pos.distance(center) < reach {
                let p = self.pickups.swap_remove(i);
                match p.power {
                    Some(Power::Slow) => self.slow_time = SLOW_TIME,
                    Some(Power::Magnet) => self.magnet = MAGNET_TIME,
                    _ => {}
                }
                events.push(p.event());
            } else {
                i += 1;
            }
        }
    }

    fn draw_player(&self, look: &Look) {
        let p = &self.player;
        let py = player_y();
        let trail = rgb(art::ANIMALS[look.animal].trail, 1.0);

        for (i, x) in p.trail.iter().enumerate().skip(1) {
            let t = 1.0 - i as f32 / TRAIL_LEN as f32;
            let r = PLAYER_SIZE * (0.15 + 0.3 * t);
            let a = if p.is_dashing() { 0.6 } else { 0.22 } * t * look.alpha;
            draw_circle(*x, py + 4.0, r, fade(trail, a));
        }
        draw_ellipse(
            p.x,
            floor_y() - 2.0,
            PLAYER_SIZE * 0.5,
            7.0,
            0.0,
            Color::new(0.0, 0.0, 0.0, 0.25),
        );

        // Squash & stretch with speed.
        let speed = (p.vx.abs() / max_speed()).min(1.5);
        let mut look = *look;
        look.stretch = vec2(1.0 + 0.16 * speed, 1.0 - 0.1 * speed);
        look.gaze = vec2(p.facing * (0.4 + 0.6 * speed.min(1.0)), -0.2);
        if p.is_dashing() {
            look.alpha *= 0.6;
        }
        art::with_transform(
            vec2(p.x, floor_y() - 2.0 - PLAYER_SIZE / 2.0),
            0.0,
            false,
            || {
                art::draw_animal(&look, PLAYER_SIZE);
            },
        );

        if self.magnet > 0.0 {
            let r = 40.0 + (look.clock * 3.0).fract() * 60.0;
            draw_circle_lines(
                p.x,
                py,
                r,
                2.0,
                Color::new(1.0, 0.4, 0.4, 0.5 * (1.0 - (r - 40.0) / 60.0)),
            );
        }
        if p.dash_cooldown > 0.0 {
            let t = 1.0 - p.dash_cooldown / DASH_COOLDOWN;
            let bw = PLAYER_SIZE;
            let by = floor_y() + 12.0;
            ui::rounded_rect(
                p.x - bw / 2.0,
                by,
                bw,
                7.0,
                3.5,
                Color::new(0.0, 0.0, 0.0, 0.3),
            );
            ui::rounded_rect(
                p.x - bw / 2.0,
                by,
                (bw * t).max(7.0),
                7.0,
                3.5,
                fade(trail, 0.95),
            );
        }
    }
}

impl Scape for Rain {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>) {
        self.time += dt;
        self.update_player(input, dt, events);
        let world_dt = dt * if self.slow_time > 0.0 { 0.45 } else { 1.0 };
        self.direct(world_dt, events);
        self.move_world(world_dt);
        self.check_collisions(stats, events);
        self.slow_time = (self.slow_time - dt).max(0.0);
        self.magnet = (self.magnet - dt).max(0.0);
        stats.label = format!("WAVE {}", self.wave.max(1));
    }

    fn coast(&mut self, dt: f32) {
        self.move_world(dt);
    }

    fn draw(&self, look: &Look, _stats: &Stats, show_player: bool) {
        let (w, h) = (vw(), vh());
        let fy = floor_y();
        let ground = shade(rgb(self.cfg.sky.1, 1.0), 0.6);
        draw_rectangle(-30.0, fy, w + 60.0, h - fy + 30.0, ground);
        draw_rectangle(
            -30.0,
            fy,
            w + 60.0,
            5.0,
            fade(rgb(self.cfg.color, 1.0), 0.8),
        );

        // Telegraph incoming blocks: a faint lane plus a pulsing arrow.
        let pulse = 0.6 + 0.4 * (look.clock * 14.0).sin();
        for o in &self.obstacles {
            if o.rect.bottom() >= 0.0 {
                continue;
            }
            let r = o.rect;
            draw_rectangle(r.x, 0.0, r.w, fy, fade(o.color, 0.07));
            let cx = r.center().x;
            let s = (r.w * 0.35).clamp(14.0, 26.0);
            draw_triangle(
                vec2(cx - s, 8.0),
                vec2(cx + s, 8.0),
                vec2(cx, 8.0 + s * 1.1),
                fade(o.color, 0.9 * pulse),
            );
        }

        let target = self.player.center();
        for o in &self.obstacles {
            if o.rect.bottom() < 0.0 {
                continue;
            }
            let r = o.rect;
            let radius = (r.w.min(r.h) * 0.25).min(16.0);
            ui::chunky_rect(r.x, r.y, r.w, r.h, radius, o.color);
            let size = r.w.min(r.h);
            if size >= 38.0 {
                art::grumpy_face(r.center(), size, r.w, target);
            }
        }

        for p in &self.pickups {
            p.draw();
        }
        if show_player {
            self.draw_player(look);
        }
        if self.slow_time > 0.0 {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.4, 0.3, 1.0, 0.08));
        }
    }

    fn player_pos(&self) -> Vec2 {
        self.player.center()
    }

    fn player_size(&self) -> f32 {
        PLAYER_SIZE
    }

    fn danger(&self) -> bool {
        let x = self.player.x;
        self.obstacles.iter().any(|o| {
            let dy = player_y() - o.rect.bottom();
            (0.0..170.0).contains(&dy) && o.rect.x < x + 60.0 && o.rect.right() > x - 60.0
        })
    }

    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32)) {
        self.cfg.sky
    }

    fn accent(&self) -> Color {
        rgb(self.cfg.color, 1.0)
    }

    fn bg_speed(&self) -> f32 {
        self.fall_speed() * 0.35 * if self.slow_time > 0.0 { 0.45 } else { 1.0 }
    }

    fn hints(&self) -> [(&'static str, &'static str); 2] {
        [
            ("Drag anywhere to move", "Arrows / A-D to move"),
            ("Tap to dash through blocks", "Space to dash through blocks"),
        ]
    }
}
