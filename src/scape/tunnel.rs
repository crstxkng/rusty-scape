//! Tunnel Scape: an endless round tunnel rushes at you in pseudo-3D. Your
//! animal stays at the bottom; spinning the tunnel brings the gaps to you.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use super::{Event, Input, Pickup, Power, Scape, Stats};
use crate::art::{self, Look};
use crate::fx::fade;
use crate::ui::{shade, vh, vw};

/// Perspective strength: a ring at depth z has radius R / (1 + z * K).
const K: f32 = 0.32;
const FAR: f32 = 30.0;
const RING_SPACING: f32 = 2.0;
const SEGMENTS: usize = 18;
/// Depth an obstacle occupies.
const THICKNESS: f32 = 0.7;
/// How far blocks stick out from the wall, as a fraction of the radius.
const BLOCK_HEIGHT: f32 = 0.3;
const PLAYER_SIZE: f32 = 54.0;
/// Tunnel rotation per virtual pixel dragged.
const DRAG_TURN: f32 = PI / 520.0;

type Rgb = (f32, f32, f32);

/// Wall stripes (two tones) and block color, cycling every 20 s.
const PALETTES: [(Rgb, Rgb, Rgb); 4] = [
    ((0.16, 0.2, 0.45), (0.22, 0.28, 0.6), (1.0, 0.45, 0.35)),
    ((0.35, 0.15, 0.4), (0.45, 0.2, 0.55), (0.3, 0.9, 0.7)),
    ((0.1, 0.32, 0.35), (0.14, 0.42, 0.45), (1.0, 0.75, 0.25)),
    ((0.4, 0.2, 0.12), (0.52, 0.27, 0.16), (0.45, 0.6, 1.0)),
];

fn center() -> Vec2 {
    vec2(vw() / 2.0, vh() * 0.47)
}

fn radius() -> f32 {
    // On phones the tunnel spills past the sides so it fills the tall screen.
    if vh() > vw() {
        vw() * 0.6
    } else {
        vh() * 0.47
    }
}

fn proj(z: f32) -> f32 {
    1.0 / (1.0 + z.max(-1.2) * K)
}

/// Screen position of a point on the wall at depth `z`, tunnel angle `a`.
fn wall_point(z: f32, a: f32, rot: f32, inset: f32) -> Vec2 {
    let r = radius() * proj(z) * (1.0 - inset);
    center() + vec2((a + rot).cos(), (a + rot).sin()) * r
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

struct Block {
    /// Start angle in tunnel space and angular span.
    start: f32,
    span: f32,
    z: f32,
    spin: f32,
    passed: bool,
}

impl Block {
    /// Distance from the player's angle to the block, 0 when inside it.
    fn gap_to(&self, angle: f32) -> f32 {
        let rel = (angle - self.start).rem_euclid(TAU);
        if rel < self.span {
            0.0
        } else {
            (rel - self.span).min(TAU - rel)
        }
    }
}

/// A coin or power-up on the wall; `pos.x` is its angle, `pos.y` its depth.
struct Item(Pickup);

pub struct Tunnel {
    time: f32,
    rot: f32,
    rot_vel: f32,
    distance: f32,
    blocks: Vec<Block>,
    items: Vec<Item>,
    spawn_timer: f32,
    coin_timer: f32,
}

impl Tunnel {
    pub fn new() -> Self {
        Tunnel {
            time: 0.0,
            rot: 0.0,
            rot_vel: 0.0,
            distance: 0.0,
            blocks: Vec::new(),
            items: Vec::new(),
            spawn_timer: 1.2,
            coin_timer: 0.5,
        }
    }

    pub fn stats(&self) -> Stats {
        Stats::new(1, 1.0)
    }

    /// The player's angle in tunnel space (they sit at the screen bottom).
    fn player_angle(&self) -> f32 {
        (PI / 2.0 - self.rot).rem_euclid(TAU)
    }

    fn speed(&self) -> f32 {
        (11.0 + self.time * 0.22).min(26.0)
    }

    /// Half the angular width of the player, used for collisions.
    fn player_half_angle(&self) -> f32 {
        (PLAYER_SIZE * 0.32) / radius()
    }

    fn spawn(&mut self) {
        let d = (self.time / 20.0).min(4.0);
        let pa = self.player_angle();
        let roll = rand::gen_range(0.0, 1.0);
        if roll < 0.3 {
            // A full ring with one gap somewhere reachable.
            let gap = (1.5 - 0.15 * d).max(0.95);
            let gap_center = pa + rand::gen_range(-2.2, 2.2);
            self.blocks.push(Block {
                start: gap_center + gap / 2.0,
                span: TAU - gap,
                z: FAR,
                spin: 0.0,
                passed: false,
            });
        } else if roll < 0.5 && self.time > 12.0 {
            // A spinning blade.
            let span = rand::gen_range(0.9, 1.4);
            let spin = rand::gen_range(0.6, 1.2 + 0.2 * d)
                * if rand::gen_range(0, 2) == 0 {
                    -1.0
                } else {
                    1.0
                };
            self.blocks.push(Block {
                start: pa + rand::gen_range(-1.5, 1.5),
                span,
                z: FAR,
                spin,
                passed: false,
            });
        } else if roll < 0.65 {
            // Two opposite wedges.
            let a = pa + rand::gen_range(-1.0, 1.0);
            for k in 0..2 {
                self.blocks.push(Block {
                    start: a + k as f32 * PI,
                    span: 1.1 + 0.1 * d,
                    z: FAR,
                    spin: 0.0,
                    passed: false,
                });
            }
        } else {
            // A single wedge, usually near the player.
            let span = rand::gen_range(0.7, 1.2 + 0.1 * d);
            let a = pa + rand::gen_range(-1.3, 1.3) - span / 2.0;
            self.blocks.push(Block {
                start: a,
                span,
                z: FAR,
                spin: 0.0,
                passed: false,
            });
        }
    }

    fn spawn_coins(&mut self) {
        // Spiral trails of coins, the main way to earn here.
        let a0 = self.player_angle() + rand::gen_range(-2.0, 2.0);
        let dir = if rand::gen_range(0, 2) == 0 {
            -1.0
        } else {
            1.0
        };
        let n = rand::gen_range(6, 10);
        for i in 0..n {
            let pos = vec2(a0 + dir * i as f32 * 0.22, FAR + i as f32 * 1.1);
            self.items.push(Item(Pickup::coin(pos)));
        }
        if rand::gen_range(0.0, 1.0) < 0.12 {
            let pos = vec2(a0 + rand::gen_range(-1.0, 1.0), FAR + n as f32 * 1.1 + 2.0);
            self.items.push(Item(Pickup::power(pos, Power::common())));
        }
    }

    fn advance(&mut self, dt: f32) {
        let dz = self.speed() * dt;
        self.distance += dz;
        for b in &mut self.blocks {
            b.z -= dz;
            b.start += b.spin * dt;
        }
        self.blocks.retain(|b| b.z + THICKNESS > -1.0);
        for it in &mut self.items {
            it.0.pos.y -= dz;
            it.0.age += dt;
        }
        self.items.retain(|it| it.0.pos.y > -1.0);
    }

    fn palette(&self) -> (Rgb, Rgb, Rgb) {
        PALETTES[(self.time / 20.0) as usize % PALETTES.len()]
    }

    fn draw_walls(&self) {
        let (a, b, _) = self.palette();
        let fog = Color::new(0.02, 0.02, 0.06, 1.0);
        let offset = self.distance % RING_SPACING;
        let first_ring = (self.distance / RING_SPACING) as i64;
        let rings = (FAR / RING_SPACING) as i32 + 1;
        // Far to near so nearer rings paint over farther ones.
        for k in (-1..rings).rev() {
            let z0 = k as f32 * RING_SPACING - offset;
            let z1 = z0 + RING_SPACING;
            let depth = (z0 / FAR).clamp(0.0, 1.0);
            for s in 0..SEGMENTS {
                let a0 = s as f32 / SEGMENTS as f32 * TAU;
                let a1 = (s + 1) as f32 / SEGMENTS as f32 * TAU;
                let stripe = (s as i64 + k as i64 + first_ring) % 2 == 0;
                let base = if stripe { a } else { b };
                let c = Color::new(base.0, base.1, base.2, 1.0);
                let c = lerp_color(c, fog, depth * 0.85);
                let p0 = wall_point(z0, a0, self.rot, 0.0);
                let p1 = wall_point(z0, a1, self.rot, 0.0);
                let p2 = wall_point(z1, a1, self.rot, 0.0);
                let p3 = wall_point(z1, a0, self.rot, 0.0);
                draw_triangle(p0, p1, p2, c);
                draw_triangle(p0, p2, p3, c);
            }
        }
        // Light at the end of the tunnel.
        let c = center();
        let r_far = radius() * proj(FAR);
        draw_circle(c.x, c.y, r_far * 1.2, Color::new(0.9, 0.95, 1.0, 0.25));
        draw_circle(c.x, c.y, r_far * 0.8, Color::new(1.0, 1.0, 1.0, 0.5));
    }

    fn draw_block(&self, b: &Block) {
        let (_, _, col) = self.palette();
        let color = Color::new(col.0, col.1, col.2, 1.0);
        let fog = (b.z / FAR).clamp(0.0, 1.0);
        let front = lerp_color(color, Color::new(0.02, 0.02, 0.06, 1.0), fog * 0.8);
        let side = shade(front, 0.6);
        let steps = ((b.span / 0.12).ceil() as usize).max(2);
        let z_back = b.z + THICKNESS;
        for i in 0..steps {
            let a0 = b.start + b.span * i as f32 / steps as f32;
            let a1 = b.start + b.span * (i + 1) as f32 / steps as f32;
            // Top face (the part facing the tunnel axis), then the front.
            let t0 = wall_point(b.z, a0, self.rot, BLOCK_HEIGHT);
            let t1 = wall_point(b.z, a1, self.rot, BLOCK_HEIGHT);
            let t2 = wall_point(z_back, a1, self.rot, BLOCK_HEIGHT);
            let t3 = wall_point(z_back, a0, self.rot, BLOCK_HEIGHT);
            draw_triangle(t0, t1, t2, side);
            draw_triangle(t0, t2, t3, side);
            let o0 = wall_point(b.z, a0, self.rot, 0.0);
            let o1 = wall_point(b.z, a1, self.rot, 0.0);
            draw_triangle(o0, o1, t1, front);
            draw_triangle(o0, t1, t0, front);
        }
        // Grumpy face on the front, when close enough to read.
        let scale = proj(b.z);
        if scale > 0.25 && b.span < PI {
            let mid = b.start + b.span / 2.0;
            let c = wall_point(b.z, mid, self.rot, BLOCK_HEIGHT / 2.0);
            let size = radius() * BLOCK_HEIGHT * scale;
            art::grumpy_face(c, size * 0.9, size * 1.4, center());
        }
    }

    fn draw_item(&self, it: &Item) {
        let p = &it.0;
        let scale = proj(p.pos.y);
        let pos = wall_point(p.pos.y, p.pos.x, self.rot, 0.12);
        let r = 16.0 * scale * 2.0;
        match p.power {
            None => art::draw_coin(pos, p.age, r),
            Some(power) => {
                art::draw_power_bubble(pos, p.age, power.color(), power.letter(), r * 1.2)
            }
        }
    }

    fn player_screen(&self) -> Vec2 {
        center() + vec2(0.0, radius() - PLAYER_SIZE / 2.0 - 4.0)
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a,
    )
}

impl Scape for Tunnel {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>) {
        self.time += dt;

        // Steering: dragging turns the tunnel 1:1, keys spin it smoothly.
        if input.dragging {
            self.rot += input.drag_dx * DRAG_TURN;
            self.rot_vel = 0.0;
        } else {
            let target = input.dir * 3.6;
            self.rot_vel += (target - self.rot_vel) * (dt * 14.0).min(1.0);
            self.rot += self.rot_vel * dt;
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn();
            let d = (self.time / 20.0).min(4.0);
            self.spawn_timer = (1.0 - 0.12 * d).max(0.48) * rand::gen_range(0.85, 1.15);
        }
        self.coin_timer -= dt;
        if self.coin_timer <= 0.0 {
            self.spawn_coins();
            self.coin_timer = rand::gen_range(1.6, 2.6);
        }

        // Distance moved this frame: collisions cover the whole sweep so a
        // fast block can't skip over the player on a slow frame.
        let dz = self.speed() * dt;
        self.advance(dt);

        let pa = self.player_angle();
        let half = self.player_half_angle();
        let at = self.player_screen();
        let mut hit = false;
        for b in &mut self.blocks {
            let at_player = b.z <= 0.0 && b.z + THICKNESS + dz >= 0.0;
            let gap = b.gap_to(pa);
            if at_player && gap < half * 0.6 && stats.invuln <= 0.0 && !hit {
                hit = true;
                b.passed = true;
            }
            if !b.passed && b.z + THICKNESS < 0.0 {
                b.passed = true;
                if gap < half + 0.2 && stats.invuln <= 0.0 {
                    events.push(Event::Close(at - vec2(0.0, 70.0)));
                }
            }
        }
        if hit {
            let (_, _, c) = self.palette();
            events.push(Event::Hit {
                at,
                color: Color::new(c.0, c.1, c.2, 1.0),
            });
        }

        self.items.retain(|it| {
            let p = &it.0;
            let reached = p.pos.y < 0.4;
            if reached && ang_diff(p.pos.x, pa).abs() < half + 0.12 {
                events.push(p.event());
                return false;
            }
            !reached || p.pos.y > -0.5
        });

        stats.label = format!("{} m", (self.distance * 2.0) as u32);
    }

    fn coast(&mut self, dt: f32) {
        self.time += dt;
        self.advance(dt);
    }

    fn draw(&self, look: &Look, _stats: &Stats, show_player: bool) {
        self.draw_walls();

        // Far to near: blocks and items share one depth order.
        let mut order: Vec<(f32, Result<&Block, &Item>)> = self
            .blocks
            .iter()
            .filter(|b| b.z > -0.4)
            .map(|b| (b.z, Ok(b)))
            .chain(
                self.items
                    .iter()
                    .filter(|i| i.0.pos.y > 0.0)
                    .map(|i| (i.0.pos.y, Err(i))),
            )
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (z, obj) in &order {
            if *z < 0.0 {
                break;
            }
            match obj {
                Ok(b) => self.draw_block(b),
                Err(i) => self.draw_item(i),
            }
        }

        if show_player {
            let pos = self.player_screen();
            draw_ellipse(
                pos.x,
                pos.y + PLAYER_SIZE / 2.0,
                PLAYER_SIZE * 0.45,
                6.0,
                0.0,
                Color::new(0.0, 0.0, 0.0, 0.3),
            );
            let mut look = *look;
            look.gaze = vec2(-self.rot_vel.clamp(-1.0, 1.0), -0.6);
            art::with_transform(pos, 0.0, false, || art::draw_animal(&look, PLAYER_SIZE));
        }

        // Blocks right at the camera plane pass "over" the player.
        for (z, obj) in &order {
            if *z < 0.0 {
                if let Ok(b) = obj {
                    self.draw_block(b);
                }
            }
        }

        // Vignette to sell the depth.
        let c = center();
        draw_circle_lines(
            c.x,
            c.y,
            radius() * 1.25,
            radius() * 0.35,
            fade(BLACK, 0.25),
        );
    }

    fn player_pos(&self) -> Vec2 {
        self.player_screen()
    }

    fn player_size(&self) -> f32 {
        PLAYER_SIZE
    }

    fn danger(&self) -> bool {
        let pa = self.player_angle();
        let half = self.player_half_angle();
        self.blocks
            .iter()
            .any(|b| b.z > 0.0 && b.z < 4.0 && b.gap_to(pa) < half)
    }

    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32)) {
        ((0.02, 0.02, 0.06), (0.04, 0.04, 0.1))
    }

    fn accent(&self) -> Color {
        Color::new(0.3, 0.85, 1.0, 1.0)
    }

    fn bg_speed(&self) -> f32 {
        0.0
    }

    fn hints(&self) -> [(&'static str, &'static str); 2] {
        [
            (
                "Drag left / right to spin the tunnel",
                "Arrows / A-D spin the tunnel",
            ),
            ("Line up with the gaps!", "Line up with the gaps!"),
        ]
    }
}
