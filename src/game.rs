use macroquad::prelude::*;

use crate::audio::{Audio, Sfx};
use crate::fx::{fade, rgb, FloatingTexts, Particles, Starfield};
use crate::modes::{Mode, ModeConfig, SKINS};
use crate::storage;
use crate::ui::{self, View, VIRTUAL_H, VIRTUAL_W};

const PLAYER_SIZE: f32 = 34.0;
const PLAYER_Y: f32 = VIRTUAL_H - 70.0;
const MAX_SPEED: f32 = 560.0;
const ACCEL: f32 = 5200.0;
const WALL_BOUNCE: f32 = 0.45;
const DASH_SPEED: f32 = 1500.0;
const DASH_TIME: f32 = 0.14;
const DASH_COOLDOWN: f32 = 0.9;
const HIT_INVULN: f32 = 1.6;
const NEAR_MISS_DIST: f32 = 18.0;
const COMBO_WINDOW: f32 = 3.0;
const MAX_COMBO: u32 = 8;
const SLOW_TIME: f32 = 4.0;
const TRAIL_LEN: usize = 10;
/// Shrinks the player's hitbox a little so near misses feel fair.
const HITBOX_INSET: f32 = 5.0;

const BG: Color = Color::new(0.04, 0.04, 0.08, 1.0);
const PAUSE_BUTTON: Rect = Rect {
    x: VIRTUAL_W - 56.0,
    y: 12.0,
    w: 44.0,
    h: 44.0,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Menu,
    Playing,
    Paused,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickupKind {
    Coin,
    Shield,
    Slow,
}

struct Obstacle {
    rect: Rect,
    vel: Vec2,
    color: Color,
    passed: bool,
}

struct Pickup {
    pos: Vec2,
    kind: PickupKind,
    age: f32,
}

struct Player {
    x: f32,
    vx: f32,
    facing: f32,
    dash_time: f32,
    dash_cooldown: f32,
    invuln: f32,
    shield: bool,
    trail: Vec<f32>,
}

impl Player {
    fn new() -> Self {
        Player {
            x: VIRTUAL_W / 2.0,
            vx: 0.0,
            facing: 1.0,
            dash_time: 0.0,
            dash_cooldown: 0.0,
            invuln: 0.0,
            shield: false,
            trail: Vec::new(),
        }
    }

    fn hitbox(&self) -> Rect {
        let s = PLAYER_SIZE - HITBOX_INSET * 2.0;
        Rect::new(self.x - s / 2.0, PLAYER_Y - s / 2.0, s, s)
    }

    fn center(&self) -> Vec2 {
        vec2(self.x, PLAYER_Y)
    }

    fn is_dashing(&self) -> bool {
        self.dash_time > 0.0
    }
}

/// Everything that belongs to a single play session.
struct Run {
    mode: Mode,
    cfg: ModeConfig,
    player: Player,
    obstacles: Vec<Obstacle>,
    pickups: Vec<Pickup>,
    time: f32,
    score: f32,
    coins: u32,
    lives: u32,
    spawn_timer: f32,
    slow_time: f32,
    combo: u32,
    combo_timer: f32,
    near_misses: u32,
    out_of_time: bool,
    new_best: bool,
    over_time: f32,
}

impl Run {
    fn new(mode: Mode) -> Self {
        let cfg = mode.config();
        Run {
            mode,
            lives: cfg.lives,
            cfg,
            player: Player::new(),
            obstacles: Vec::new(),
            pickups: Vec::new(),
            time: 0.0,
            score: 0.0,
            coins: 0,
            spawn_timer: 0.8,
            slow_time: 0.0,
            combo: 0,
            combo_timer: 0.0,
            near_misses: 0,
            out_of_time: false,
            new_best: false,
            over_time: 0.0,
        }
    }

    /// Continuous difficulty: 0 at the start, +1 per `seconds_per_level`.
    fn difficulty(&self) -> f32 {
        self.time / self.cfg.seconds_per_level
    }

    fn level(&self) -> u32 {
        1 + self.difficulty() as u32
    }

    fn fall_speed(&self) -> f32 {
        self.cfg.fall_speed * (1.0 + 0.12 * self.difficulty()).min(2.6)
    }

    fn world_time_scale(&self) -> f32 {
        if self.slow_time > 0.0 {
            0.45
        } else {
            1.0
        }
    }

    fn combo_mult(&self) -> f32 {
        1.0 + 0.25 * self.combo as f32
    }

    fn time_left(&self) -> Option<f32> {
        self.cfg
            .time_limit
            .map(|limit| (limit - self.time).max(0.0))
    }

    fn obstacle_color(&self) -> Color {
        // Shift from the mode's accent toward hot red as the level climbs.
        let t = (self.difficulty() / 8.0).min(1.0);
        let (r, g, b) = self.cfg.block;
        Color::new(
            r + (1.0 - r) * t,
            g * (1.0 - 0.7 * t),
            b * (1.0 - 0.6 * t),
            1.0,
        )
    }
}

pub struct Game {
    phase: Phase,
    menu_index: usize,
    skin: usize,
    high_scores: [u32; Mode::ALL.len()],
    audio: Audio,
    particles: Particles,
    texts: FloatingTexts,
    stars: Starfield,
    run: Run,
    shake: f32,
    flash: f32,
    view: View,
    clock: f32,
}

impl Game {
    pub async fn new() -> Self {
        let mut high_scores = [0; Mode::ALL.len()];
        for (i, hs) in high_scores.iter_mut().enumerate() {
            *hs = storage::get(storage::high_score_slot(i)).unwrap_or(0.0) as u32;
        }
        let skin = (storage::get(storage::SLOT_SKIN).unwrap_or(0.0) as usize).min(SKINS.len() - 1);
        let muted = storage::get(storage::SLOT_MUTED).unwrap_or(0.0) != 0.0;
        Game {
            phase: Phase::Menu,
            menu_index: 0,
            skin,
            high_scores,
            audio: Audio::load(muted).await,
            particles: Particles::default(),
            texts: FloatingTexts::default(),
            stars: Starfield::new(),
            run: Run::new(Mode::Classic),
            shake: 0.0,
            flash: 0.0,
            view: View::compute(),
            clock: 0.0,
        }
    }

    pub fn frame(&mut self) {
        // Clamp dt so a stalled tab or window drag doesn't teleport everything.
        let dt = get_frame_time().min(1.0 / 30.0);
        self.clock += dt;
        self.view = View::compute();
        self.handle_global_keys();

        match self.phase {
            Phase::Menu => self.update_menu_keys(),
            Phase::Playing => self.update_playing(dt),
            Phase::Paused => self.update_paused_keys(),
            Phase::GameOver => self.update_game_over(dt),
        }
        if self.phase != Phase::Paused {
            self.particles.update(dt);
            self.texts.update(dt);
            let star_speed = match self.phase {
                Phase::Playing | Phase::GameOver => {
                    self.run.fall_speed() * 0.5 * self.run.world_time_scale()
                }
                _ => 60.0,
            };
            self.stars.update(dt, star_speed);
            self.shake = (self.shake - dt * 30.0).max(0.0);
            self.flash = (self.flash - dt * 2.5).max(0.0);
        }

        clear_background(BLACK);
        let shake = vec2(rand::gen_range(-1.0, 1.0), rand::gen_range(-1.0, 1.0)) * self.shake;
        self.view.apply(shake);
        draw_rectangle(-20.0, -20.0, VIRTUAL_W + 40.0, VIRTUAL_H + 40.0, BG);

        match self.phase {
            Phase::Menu => self.draw_menu(),
            Phase::Playing => {
                self.draw_world();
                self.draw_hud();
            }
            Phase::Paused => {
                self.draw_world();
                self.draw_hud();
                self.draw_pause();
            }
            Phase::GameOver => {
                self.draw_world();
                self.draw_game_over();
            }
        }

        if self.flash > 0.0 {
            draw_rectangle(
                0.0,
                0.0,
                VIRTUAL_W,
                VIRTUAL_H,
                Color::new(1.0, 0.3, 0.2, self.flash * 0.35),
            );
        }
        set_default_camera();
    }

    fn handle_global_keys(&mut self) {
        if is_key_pressed(KeyCode::M) {
            self.toggle_mute();
        }
        if is_key_pressed(KeyCode::F) || is_key_pressed(KeyCode::F11) {
            toggle_fullscreen();
        }
    }

    fn toggle_mute(&mut self) {
        self.audio.muted = !self.audio.muted;
        storage::set(
            storage::SLOT_MUTED,
            if self.audio.muted { 1.0 } else { 0.0 },
        );
        self.audio.play(Sfx::Select);
    }

    fn selected_mode(&self) -> Mode {
        Mode::ALL[self.menu_index]
    }

    fn start_run(&mut self, mode: Mode) {
        self.run = Run::new(mode);
        self.particles.clear();
        self.texts.clear();
        self.phase = Phase::Playing;
        self.audio.play(Sfx::Start);
    }

    fn change_skin(&mut self, delta: i32) {
        let n = SKINS.len() as i32;
        self.skin = ((self.skin as i32 + delta).rem_euclid(n)) as usize;
        storage::set(storage::SLOT_SKIN, self.skin as f64);
        self.audio.play(Sfx::Select);
    }

    // ---------------------------------------------------------------- menu

    fn update_menu_keys(&mut self) {
        let n = Mode::ALL.len();
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.menu_index = (self.menu_index + n - 1) % n;
            self.audio.play(Sfx::Select);
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.menu_index = (self.menu_index + 1) % n;
            self.audio.play(Sfx::Select);
        }
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.change_skin(-1);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.change_skin(1);
        }
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
            self.start_run(self.selected_mode());
        }
    }

    fn draw_menu(&mut self) {
        self.stars.draw(WHITE);
        let cx = VIRTUAL_W / 2.0;
        let accent = rgb(self.selected_mode().config().color, 1.0);

        // Title with a little bounce.
        let bob = (self.clock * 2.0).sin() * 4.0;
        ui::text_shadow_centered(
            "RUSTY SCAPE",
            cx,
            96.0 + bob,
            72.0,
            Color::new(1.0, 0.6, 0.2, 1.0),
        );
        ui::text_centered(
            "Dodge the falling blocks!",
            cx,
            134.0,
            24.0,
            Color::new(0.8, 0.8, 0.85, 1.0),
        );

        // Mode list.
        let row_w = 380.0;
        let row_h = 42.0;
        let list_y = 160.0;
        for (i, mode) in Mode::ALL.iter().enumerate() {
            let cfg = mode.config();
            let rect = Rect::new(
                cx - row_w / 2.0,
                list_y + i as f32 * (row_h + 8.0),
                row_w,
                row_h,
            );
            let selected = i == self.menu_index;
            let color = rgb(cfg.color, 1.0);
            if ui::clicked_in(&self.view, rect) {
                if selected {
                    self.start_run(*mode);
                    return;
                }
                self.menu_index = i;
                self.audio.play(Sfx::Select);
            }
            let bg = if selected {
                Color::new(color.r * 0.3, color.g * 0.3, color.b * 0.3, 0.95)
            } else {
                Color::new(0.08, 0.08, 0.12, 0.95)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
            if selected {
                draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, color);
                draw_rectangle(rect.x, rect.y, 6.0, rect.h, color);
            }
            ui::text(
                cfg.name,
                rect.x + 20.0,
                rect.y + 29.0,
                26.0,
                if selected { WHITE } else { GRAY },
            );
            let best = format!("Best {}", self.high_scores[i]);
            let w = ui::text_width(&best, 20.0);
            ui::text(
                &best,
                rect.right() - w - 14.0,
                rect.y + 28.0,
                20.0,
                fade(color, if selected { 1.0 } else { 0.6 }),
            );
        }

        let after_list = list_y + Mode::ALL.len() as f32 * (row_h + 8.0);
        let cfg = self.selected_mode().config();
        ui::text_centered(cfg.tagline, cx, after_list + 20.0, 22.0, fade(accent, 0.9));

        // Skin picker.
        let skin = &SKINS[self.skin];
        let skin_y = after_list + 44.0;
        let left = Rect::new(cx - 150.0, skin_y, 40.0, 40.0);
        let right = Rect::new(cx + 110.0, skin_y, 40.0, 40.0);
        if ui::button(&self.view, left, "<", accent) {
            self.change_skin(-1);
        }
        if ui::button(&self.view, right, ">", accent) {
            self.change_skin(1);
        }
        let s = 30.0;
        draw_rectangle(cx - 70.0, skin_y + 5.0, s, s, rgb(skin.body, 1.0));
        draw_rectangle_lines(cx - 70.0, skin_y + 5.0, s, s, 2.0, rgb(skin.trail, 1.0));
        ui::text(
            &format!("Skin: {}", skin.name),
            cx - 28.0,
            skin_y + 28.0,
            24.0,
            WHITE,
        );

        let play = Rect::new(cx - 90.0, skin_y + 56.0, 180.0, 48.0);
        if ui::button(&self.view, play, "PLAY", accent) {
            self.start_run(self.selected_mode());
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let quit = Rect::new(VIRTUAL_W - 110.0, VIRTUAL_H - 50.0, 96.0, 36.0);
            if ui::button(&self.view, quit, "Quit", GRAY) {
                macroquad::miniquad::window::order_quit();
            }
        }
        let mute = Rect::new(14.0, VIRTUAL_H - 50.0, 110.0, 36.0);
        let mute_label = if self.audio.muted {
            "Sound: off"
        } else {
            "Sound: on"
        };
        if ui::button(&self.view, mute, mute_label, GRAY) {
            self.toggle_mute();
        }

        ui::text_centered(
            "Up/Down: mode   Left/Right: skin   Enter: play   M: mute   F: fullscreen",
            cx,
            VIRTUAL_H - 62.0,
            18.0,
            Color::new(0.6, 0.6, 0.65, 1.0),
        );
        ui::text_centered(
            "In game: arrows / A-D move, Space dashes through blocks. Touch: drag, 2nd finger to dash",
            cx,
            VIRTUAL_H - 22.0,
            16.0,
            Color::new(0.5, 0.5, 0.55, 1.0),
        );
    }

    // ------------------------------------------------------------- playing

    fn update_playing(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::Escape)
            || is_key_pressed(KeyCode::P)
            || ui::clicked_in(&self.view, PAUSE_BUTTON)
        {
            self.phase = Phase::Paused;
            return;
        }

        self.run.time += dt;
        if let Some(left) = self.run.time_left() {
            if left <= 0.0 {
                self.run.out_of_time = true;
                self.end_run();
                return;
            }
        }

        self.update_player(dt);
        let world_dt = dt * self.run.world_time_scale();
        self.spawn(world_dt);
        self.move_world(world_dt);
        self.check_collisions();

        let run = &mut self.run;
        run.slow_time = (run.slow_time - dt).max(0.0);
        run.combo_timer -= dt;
        if run.combo_timer <= 0.0 {
            run.combo = 0;
        }
        let survival = if run.cfg.time_limit.is_some() {
            5.0
        } else {
            10.0
        };
        run.score += survival * dt * run.cfg.score_mult * run.combo_mult();
    }

    fn read_move_input(&self) -> (f32, Option<f32>) {
        let mut dir = 0.0;
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            dir -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            dir += 1.0;
        }
        // Mouse drag or the first touch steers toward the pointer.
        let pointer = if is_mouse_button_down(MouseButton::Left) {
            let m = self.view.mouse();
            (!PAUSE_BUTTON.contains(m)).then_some(m.x)
        } else {
            None
        };
        (dir, pointer)
    }

    fn dash_pressed() -> bool {
        let key = [
            KeyCode::Space,
            KeyCode::LeftShift,
            KeyCode::RightShift,
            KeyCode::Up,
            KeyCode::W,
        ]
        .iter()
        .any(|k| is_key_pressed(*k));
        let touches = touches();
        let second_finger =
            touches.len() >= 2 && touches.iter().any(|t| t.phase == TouchPhase::Started);
        key || second_finger || is_mouse_button_pressed(MouseButton::Right)
    }

    fn update_player(&mut self, dt: f32) {
        let (dir, pointer) = self.read_move_input();
        let dash = Self::dash_pressed();
        let p = &mut self.run.player;

        p.dash_cooldown = (p.dash_cooldown - dt).max(0.0);
        p.invuln = (p.invuln - dt).max(0.0);

        if dash && p.dash_cooldown <= 0.0 {
            if dir != 0.0 {
                p.facing = dir;
            } else if let Some(target) = pointer {
                if (target - p.x).abs() > 4.0 {
                    p.facing = (target - p.x).signum();
                }
            }
            p.dash_time = DASH_TIME;
            p.dash_cooldown = DASH_COOLDOWN;
            self.audio.play(Sfx::Dash);
            self.particles
                .burst(p.center(), 10, rgb(SKINS[self.skin].trail, 0.8), 220.0);
        }

        if p.dash_time > 0.0 {
            p.dash_time -= dt;
            p.vx = p.facing * DASH_SPEED;
        } else if dir != 0.0 {
            p.vx = (p.vx + dir * ACCEL * dt).clamp(-MAX_SPEED, MAX_SPEED);
            p.facing = dir;
        } else if let Some(target) = pointer {
            let desired = ((target - p.x) * 12.0).clamp(-MAX_SPEED, MAX_SPEED);
            p.vx += (desired - p.vx).clamp(-ACCEL * dt, ACCEL * dt);
            if desired.abs() > 20.0 {
                p.facing = desired.signum();
            }
        } else {
            let friction = ACCEL * dt;
            p.vx = if p.vx.abs() <= friction {
                0.0
            } else {
                p.vx - p.vx.signum() * friction
            };
        }

        p.x += p.vx * dt;
        let half = PLAYER_SIZE / 2.0;
        if p.x < half {
            p.x = half;
            p.vx = -p.vx * WALL_BOUNCE;
        } else if p.x > VIRTUAL_W - half {
            p.x = VIRTUAL_W - half;
            p.vx = -p.vx * WALL_BOUNCE;
        }

        p.trail.insert(0, p.x);
        p.trail.truncate(TRAIL_LEN);
    }

    fn spawn(&mut self, dt: f32) {
        let run = &mut self.run;
        run.spawn_timer -= dt;
        if run.spawn_timer > 0.0 {
            return;
        }
        let d = run.difficulty();
        run.spawn_timer =
            (run.cfg.spawn_interval / (1.0 + 0.15 * d)).max(0.12) * rand::gen_range(0.7, 1.3);

        let (w, h) = if run.cfg.drifting {
            let w = rand::gen_range(18.0, 150.0);
            (w, rand::gen_range(18.0, 60.0f32).min(w * 1.5))
        } else if d > 2.0 && rand::gen_range(0.0, 1.0) < 0.12 {
            // Occasional wide slab once things heat up.
            (rand::gen_range(110.0, 200.0), 26.0)
        } else {
            let s = rand::gen_range(28.0, 58.0);
            (s, s * rand::gen_range(0.8, 1.2))
        };
        let speed =
            run.fall_speed() * rand::gen_range(0.85, 1.25) * if w > 100.0 { 0.75 } else { 1.0 };
        let vx = if run.cfg.drifting {
            rand::gen_range(-140.0, 140.0)
        } else {
            0.0
        };
        let color = run.obstacle_color();
        run.obstacles.push(Obstacle {
            rect: Rect::new(rand::gen_range(0.0, VIRTUAL_W - w), -h, w, h),
            vel: vec2(vx, speed),
            color,
            passed: false,
        });

        let roll = rand::gen_range(0.0, 1.0);
        let kind = if roll < run.cfg.coin_chance {
            Some(PickupKind::Coin)
        } else if roll < run.cfg.coin_chance + run.cfg.powerup_chance {
            Some(if rand::gen_range(0, 2) == 0 {
                PickupKind::Shield
            } else {
                PickupKind::Slow
            })
        } else {
            None
        };
        if let Some(kind) = kind {
            run.pickups.push(Pickup {
                pos: vec2(rand::gen_range(30.0, VIRTUAL_W - 30.0), -20.0),
                kind,
                age: 0.0,
            });
        }
    }

    fn move_world(&mut self, dt: f32) {
        let run = &mut self.run;
        for o in &mut run.obstacles {
            o.rect.x += o.vel.x * dt;
            o.rect.y += o.vel.y * dt;
            if o.rect.x < 0.0 || o.rect.right() > VIRTUAL_W {
                o.vel.x = -o.vel.x;
                o.rect.x = o.rect.x.clamp(0.0, VIRTUAL_W - o.rect.w);
            }
        }
        run.obstacles.retain(|o| o.rect.y < VIRTUAL_H + 10.0);

        let pickup_speed = 150.0 + run.fall_speed() * 0.2;
        for p in &mut run.pickups {
            p.age += dt;
            p.pos.y += pickup_speed * dt;
            p.pos.x += (p.age * 3.0).sin() * 40.0 * dt;
        }
        run.pickups.retain(|p| p.pos.y < VIRTUAL_H + 30.0);
    }

    fn check_collisions(&mut self) {
        let hitbox = self.run.player.hitbox();
        let phasing = self.run.player.is_dashing() || self.run.player.invuln > 0.0;

        // Obstacles.
        let mut hit_index = None;
        let mut near_miss_count = 0;
        for (i, o) in self.run.obstacles.iter_mut().enumerate() {
            if o.rect.overlaps(&hitbox) {
                if !phasing {
                    hit_index = Some(i);
                    break;
                }
                continue;
            }
            // Near miss: the block passes the player's row within a whisker.
            if !o.passed && o.rect.y > hitbox.bottom() {
                o.passed = true;
                let gap = (o.rect.x - hitbox.right()).max(hitbox.x - o.rect.right());
                if (0.0..NEAR_MISS_DIST).contains(&gap) && !phasing {
                    near_miss_count += 1;
                }
            }
        }
        for _ in 0..near_miss_count {
            let run = &mut self.run;
            run.combo = (run.combo + 1).min(MAX_COMBO);
            run.combo_timer = COMBO_WINDOW;
            run.near_misses += 1;
            let bonus = (15.0 * run.cfg.score_mult * run.combo as f32).round();
            run.score += bonus;
            self.texts.spawn(
                format!("CLOSE! +{bonus}"),
                vec2(hitbox.center().x, PLAYER_Y - 40.0),
                YELLOW,
            );
            self.audio.play(Sfx::NearMiss);
        }
        if let Some(i) = hit_index {
            let o = self.run.obstacles.swap_remove(i);
            self.on_hit(o);
            if self.phase != Phase::Playing {
                return;
            }
        }

        // Pickups.
        let center = self.run.player.center();
        let mut collected = Vec::new();
        self.run.pickups.retain(|p| {
            let hit = p.pos.distance(center) < PLAYER_SIZE * 0.5 + 16.0;
            if hit {
                collected.push(p.kind);
            }
            !hit
        });
        for kind in collected {
            self.collect(kind);
        }
    }

    fn on_hit(&mut self, obstacle: Obstacle) {
        let center = self.run.player.center();
        self.particles
            .burst(obstacle.rect.center(), 18, obstacle.color, 380.0);
        self.shake = 10.0;

        if self.run.player.shield {
            self.run.player.shield = false;
            self.run.player.invuln = 0.8;
            self.particles.burst(center, 20, SKYBLUE, 300.0);
            self.texts
                .spawn("SHIELD BROKEN", center - vec2(0.0, 40.0), SKYBLUE);
            self.audio.play(Sfx::Hit);
            return;
        }

        self.run.lives -= 1;
        self.run.combo = 0;
        self.flash = 1.0;
        self.audio.play(Sfx::Hit);
        self.particles
            .burst(center, 30, rgb(SKINS[self.skin].body, 1.0), 450.0);
        if self.run.lives == 0 {
            self.end_run();
        } else {
            self.run.player.invuln = HIT_INVULN;
            self.texts.spawn(
                format!("{} LEFT", self.run.lives),
                center - vec2(0.0, 40.0),
                RED,
            );
        }
    }

    fn collect(&mut self, kind: PickupKind) {
        let center = self.run.player.center();
        let run = &mut self.run;
        match kind {
            PickupKind::Coin => {
                let value = (25.0 * run.cfg.score_mult * run.combo_mult()).round();
                run.score += value;
                run.coins += 1;
                self.texts
                    .spawn(format!("+{value}"), center - vec2(0.0, 40.0), GOLD);
                self.particles.burst(center, 10, GOLD, 200.0);
                self.audio.play(Sfx::Coin);
            }
            PickupKind::Shield => {
                run.player.shield = true;
                self.texts
                    .spawn("SHIELD", center - vec2(0.0, 40.0), SKYBLUE);
                self.audio.play(Sfx::PowerUp);
            }
            PickupKind::Slow => {
                run.slow_time = SLOW_TIME;
                self.texts
                    .spawn("SLOW-MO", center - vec2(0.0, 40.0), VIOLET);
                self.audio.play(Sfx::PowerUp);
            }
        }
    }

    fn end_run(&mut self) {
        let score = self.run.score as u32;
        let i = self.run.mode.index();
        self.run.new_best = score > self.high_scores[i];
        if self.run.new_best {
            self.high_scores[i] = score;
            storage::set(storage::high_score_slot(i), score as f64);
        }
        self.run.over_time = 0.0;
        self.phase = Phase::GameOver;
        self.audio.play(Sfx::GameOver);
    }

    // ----------------------------------------------------------- game over

    fn update_game_over(&mut self, dt: f32) {
        self.run.over_time += dt;
        // Let the world keep falling behind the results screen.
        self.move_world(dt * 0.5);
        // Brief input lock so a frantic keypress doesn't skip the results.
        if self.run.over_time < 0.6 {
            return;
        }
        if is_key_pressed(KeyCode::Space)
            || is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::R)
        {
            self.start_run(self.run.mode);
        } else if is_key_pressed(KeyCode::Escape) {
            self.phase = Phase::Menu;
        }
    }

    // ------------------------------------------------------------- drawing

    fn draw_world(&self) {
        let run = &self.run;
        let tint = if run.slow_time > 0.0 {
            Color::new(0.5, 0.4, 1.0, 1.0)
        } else {
            rgb(run.cfg.color, 1.0)
        };
        self.stars.draw(tint);

        // Floor line.
        let floor_y = PLAYER_Y + PLAYER_SIZE / 2.0 + 6.0;
        draw_line(
            0.0,
            floor_y,
            VIRTUAL_W,
            floor_y,
            2.0,
            fade(rgb(run.cfg.color, 1.0), 0.35),
        );

        for o in &run.obstacles {
            let r = o.rect;
            draw_rectangle(r.x, r.y, r.w, r.h, o.color);
            draw_rectangle(r.x, r.y, r.w, 4.0, Color::new(1.0, 1.0, 1.0, 0.25));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.0, 0.0, 0.0, 0.4));
        }

        for p in &run.pickups {
            let pulse = 1.0 + (p.age * 8.0).sin() * 0.12;
            match p.kind {
                PickupKind::Coin => {
                    // Fake a spinning coin by squashing its width.
                    let w = 12.0 * (p.age * 5.0).cos().abs().max(0.2) * pulse;
                    draw_ellipse(p.pos.x, p.pos.y, w, 12.0 * pulse, 0.0, GOLD);
                    draw_ellipse_lines(p.pos.x, p.pos.y, w, 12.0 * pulse, 0.0, 2.0, ORANGE);
                }
                PickupKind::Shield => {
                    draw_circle(p.pos.x, p.pos.y, 14.0 * pulse, fade(SKYBLUE, 0.35));
                    draw_circle_lines(p.pos.x, p.pos.y, 14.0 * pulse, 3.0, SKYBLUE);
                    ui::text_centered("S", p.pos.x, p.pos.y + 7.0, 20.0, WHITE);
                }
                PickupKind::Slow => {
                    draw_circle(p.pos.x, p.pos.y, 14.0 * pulse, fade(VIOLET, 0.35));
                    draw_circle_lines(p.pos.x, p.pos.y, 14.0 * pulse, 3.0, VIOLET);
                    ui::text_centered("T", p.pos.x, p.pos.y + 7.0, 20.0, WHITE);
                }
            }
        }

        if self.phase != Phase::GameOver {
            self.draw_player();
        }
        self.particles.draw();
        self.texts.draw();

        if run.slow_time > 0.0 {
            draw_rectangle(
                0.0,
                0.0,
                VIRTUAL_W,
                VIRTUAL_H,
                Color::new(0.3, 0.2, 0.8, 0.08),
            );
        }
    }

    fn draw_player(&self) {
        let p = &self.run.player;
        let skin = &SKINS[self.skin];

        // Blink while invulnerable after a hit.
        if p.invuln > 0.0 && !p.is_dashing() && (p.invuln * 12.0) as i32 % 2 == 0 {
            return;
        }

        for (i, x) in p.trail.iter().enumerate().skip(1) {
            let t = 1.0 - i as f32 / TRAIL_LEN as f32;
            let s = PLAYER_SIZE * (0.4 + 0.5 * t);
            let a = if p.is_dashing() { 0.6 } else { 0.25 } * t;
            draw_rectangle(x - s / 2.0, PLAYER_Y - s / 2.0, s, s, rgb(skin.trail, a));
        }

        // Squash & stretch with speed.
        let speed = (p.vx.abs() / MAX_SPEED).min(1.5);
        let w = PLAYER_SIZE * (1.0 + 0.18 * speed);
        let h = PLAYER_SIZE * (1.0 - 0.12 * speed);
        let x = p.x - w / 2.0;
        let y = PLAYER_Y + PLAYER_SIZE / 2.0 - h;
        let alpha = if p.is_dashing() { 0.55 } else { 1.0 };
        draw_rectangle(x, y, w, h, rgb(skin.body, alpha));
        draw_rectangle(x, y, w, 5.0, Color::new(1.0, 1.0, 1.0, 0.3 * alpha));
        draw_rectangle_lines(x, y, w, h, 2.0, rgb(skin.trail, alpha));

        // Eyes look where we're heading.
        let look = p.facing * 4.0;
        let eye = Color::new(0.05, 0.05, 0.1, alpha);
        draw_rectangle(p.x - 9.0 + look, y + h * 0.3, 5.0, 8.0, eye);
        draw_rectangle(p.x + 4.0 + look, y + h * 0.3, 5.0, 8.0, eye);

        if p.shield {
            let r = PLAYER_SIZE * 0.85 + (self.clock * 6.0).sin() * 2.0;
            draw_circle_lines(p.x, PLAYER_Y, r, 3.0, fade(SKYBLUE, 0.8));
        }

        // Dash cooldown bar under the player.
        if p.dash_cooldown > 0.0 {
            let t = 1.0 - p.dash_cooldown / DASH_COOLDOWN;
            let bw = PLAYER_SIZE + 6.0;
            let by = PLAYER_Y + PLAYER_SIZE / 2.0 + 12.0;
            draw_rectangle(p.x - bw / 2.0, by, bw, 4.0, Color::new(1.0, 1.0, 1.0, 0.15));
            draw_rectangle(p.x - bw / 2.0, by, bw * t, 4.0, rgb(skin.trail, 0.9));
        }
    }

    fn draw_hud(&self) {
        let run = &self.run;
        let accent = rgb(run.cfg.color, 1.0);
        ui::text(&format!("{}", run.score as u32), 18.0, 46.0, 40.0, WHITE);
        ui::text(
            &format!("{}  -  Level {}", run.cfg.name, run.level()),
            20.0,
            72.0,
            20.0,
            fade(accent, 0.9),
        );
        if run.coins > 0 {
            ui::text(&format!("Coins {}", run.coins), 20.0, 96.0, 20.0, GOLD);
        }

        // Lives.
        if run.cfg.lives > 1 {
            for i in 0..run.cfg.lives {
                let x = VIRTUAL_W - 80.0 - i as f32 * 26.0;
                let c = if i < run.lives {
                    RED
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.15)
                };
                draw_rectangle(x, 24.0, 18.0, 18.0, c);
            }
        }

        if let Some(left) = run.time_left() {
            let c = if left < 10.0 { RED } else { WHITE };
            ui::text_shadow_centered(&format!("{:.1}", left), VIRTUAL_W / 2.0, 52.0, 44.0, c);
        }

        if run.combo > 0 {
            let a = (run.combo_timer / COMBO_WINDOW).clamp(0.3, 1.0);
            ui::text_shadow_centered(
                &format!("COMBO x{:.2}", run.combo_mult()),
                VIRTUAL_W / 2.0,
                if run.cfg.time_limit.is_some() {
                    84.0
                } else {
                    52.0
                },
                26.0,
                fade(YELLOW, a),
            );
        }

        // Pause button (handy on touch screens).
        let b = PAUSE_BUTTON;
        draw_rectangle(b.x, b.y, b.w, b.h, Color::new(1.0, 1.0, 1.0, 0.08));
        draw_rectangle(b.x + 14.0, b.y + 12.0, 5.0, 20.0, WHITE);
        draw_rectangle(b.x + 25.0, b.y + 12.0, 5.0, 20.0, WHITE);
    }

    fn draw_pause(&mut self) {
        draw_rectangle(
            0.0,
            0.0,
            VIRTUAL_W,
            VIRTUAL_H,
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
        let cx = VIRTUAL_W / 2.0;
        let accent = rgb(self.run.cfg.color, 1.0);
        ui::text_shadow_centered("PAUSED", cx, 230.0, 64.0, WHITE);
        let resume = Rect::new(cx - 110.0, 280.0, 220.0, 50.0);
        let menu = Rect::new(cx - 110.0, 345.0, 220.0, 50.0);
        let resume_clicked = ui::button(&self.view, resume, "Resume", accent);
        let menu_clicked = ui::button(&self.view, menu, "Main menu", accent);
        ui::text_centered("P / Esc to resume   Q for menu", cx, 440.0, 20.0, GRAY);
        if resume_clicked {
            self.phase = Phase::Playing;
        } else if menu_clicked {
            self.phase = Phase::Menu;
        }
    }

    fn update_paused_keys(&mut self) {
        if is_key_pressed(KeyCode::P)
            || is_key_pressed(KeyCode::Escape)
            || is_key_pressed(KeyCode::Space)
        {
            self.phase = Phase::Playing;
        } else if is_key_pressed(KeyCode::Q) || is_key_pressed(KeyCode::Backspace) {
            self.phase = Phase::Menu;
        }
    }

    fn draw_game_over(&mut self) {
        let run = &self.run;
        let t = (run.over_time * 3.0).min(1.0);
        draw_rectangle(
            0.0,
            0.0,
            VIRTUAL_W,
            VIRTUAL_H,
            Color::new(0.0, 0.0, 0.0, 0.55 * t),
        );
        let cx = VIRTUAL_W / 2.0;
        let accent = rgb(run.cfg.color, 1.0);

        let title = if run.out_of_time {
            "TIME'S UP!"
        } else {
            "CRUSHED!"
        };
        ui::text_shadow_centered(title, cx, 170.0 - (1.0 - t) * 40.0, 72.0, fade(accent, t));
        ui::text_centered(run.cfg.name, cx, 205.0, 24.0, fade(GRAY, t));
        ui::text_shadow_centered(
            &format!("{}", run.score as u32),
            cx,
            280.0,
            64.0,
            fade(WHITE, t),
        );

        let best = self.high_scores[run.mode.index()];
        if run.new_best {
            let pulse = 0.75 + 0.25 * (self.clock * 8.0).sin();
            ui::text_centered("NEW HIGH SCORE!", cx, 320.0, 28.0, fade(GREEN, t * pulse));
        } else {
            ui::text_centered(&format!("Best: {best}"), cx, 320.0, 26.0, fade(GREEN, t));
        }
        ui::text_centered(
            &format!(
                "Survived {:.1}s   Level {}   Coins {}   Near misses {}",
                run.time,
                run.level(),
                run.coins,
                run.near_misses
            ),
            cx,
            360.0,
            20.0,
            fade(Color::new(0.8, 0.8, 0.85, 1.0), t),
        );

        if run.over_time < 0.6 {
            return;
        }
        let retry = Rect::new(cx - 230.0, 400.0, 220.0, 52.0);
        let menu = Rect::new(cx + 10.0, 400.0, 220.0, 52.0);
        if ui::button(&self.view, retry, "Retry (Space)", accent) {
            self.start_run(self.run.mode);
        } else if ui::button(&self.view, menu, "Menu (Esc)", accent) {
            self.phase = Phase::Menu;
        }
    }
}

fn toggle_fullscreen() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static FULLSCREEN: AtomicBool = AtomicBool::new(false);
    let next = !FULLSCREEN.load(Ordering::Relaxed);
    FULLSCREEN.store(next, Ordering::Relaxed);
    set_fullscreen(next);
}
