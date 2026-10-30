use std::f32::consts::PI;

use macroquad::prelude::*;

use crate::audio::{Audio, Sfx};
use crate::fx::{fade, rgb, Background, Banner, FloatingTexts, Particles};
use crate::modes::{Mode, ModeConfig, SKINS};
use crate::storage;
use crate::ui::{self, shade, vh, vw, View};

const PLAYER_SIZE: f32 = 58.0;
/// Shrinks the player's hitbox so near misses feel fair.
const HITBOX_INSET: f32 = 10.0;
const DASH_TIME: f32 = 0.13;
const DASH_COOLDOWN: f32 = 0.8;
const HIT_INVULN: f32 = 1.6;
const NEAR_MISS_DIST: f32 = 26.0;
const COMBO_WINDOW: f32 = 2.5;
const MAX_COMBO: u32 = 10;
const SLOW_TIME: f32 = 4.0;
const MAGNET_TIME: f32 = 7.0;
const MAGNET_RANGE: f32 = 280.0;
const TRAIL_LEN: usize = 10;
/// Finger movement is amplified a little so small drags cover the screen.
const DRAG_GAIN: f32 = 1.35;
/// A touch shorter than this that barely moves counts as a tap (dash).
const TAP_TIME: f32 = 0.22;
const TAP_SLOP: f32 = 16.0;
/// Slow-motion beat between the fatal hit and the results screen.
const DEATH_TIME: f32 = 0.9;
/// Seconds a falling block is telegraphed above the screen before it enters.
const WARN_TIME: f32 = 0.45;
/// Runs during which the control hints are shown.
const TUTORIAL_RUNS: u32 = 3;

fn player_y() -> f32 {
    vh() - if ui::is_portrait() { 170.0 } else { 100.0 }
}

fn floor_y() -> f32 {
    player_y() + PLAYER_SIZE / 2.0 + 4.0
}

fn max_speed() -> f32 {
    vw() * 0.75
}

fn pause_button() -> Rect {
    Rect::new(vw() - 74.0, 14.0, 60.0, 60.0)
}

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
    Magnet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Normal,
    Happy,
    Scared,
    Blink,
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

struct Pickup {
    pos: Vec2,
    kind: PickupKind,
    age: f32,
}

struct Player {
    x: f32,
    vx: f32,
    /// Where touch / mouse dragging wants the player to be.
    target: Option<f32>,
    facing: f32,
    dash_time: f32,
    dash_cooldown: f32,
    invuln: f32,
    shield: bool,
    magnet: f32,
    happy: f32,
    blink_in: f32,
    trail: Vec<f32>,
    trail_clock: f32,
}

impl Player {
    fn new() -> Self {
        Player {
            x: vw() / 2.0,
            vx: 0.0,
            target: None,
            facing: 1.0,
            dash_time: 0.0,
            dash_cooldown: 0.0,
            invuln: 0.0,
            shield: false,
            magnet: 0.0,
            happy: 0.0,
            blink_in: 2.0,
            trail: Vec::new(),
            trail_clock: 0.0,
        }
    }

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

/// Tracks a drag gesture for relative touch / mouse control.
#[derive(Default)]
struct Pointer {
    active: bool,
    last: Vec2,
    down_at: f32,
    travel: f32,
}

/// Everything that belongs to a single play session.
struct Run {
    mode: Mode,
    cfg: ModeConfig,
    player: Player,
    obstacles: Vec<Obstacle>,
    pickups: Vec<Pickup>,
    schedule: Vec<Scheduled>,
    time: f32,
    score: f32,
    coins: u32,
    lives: u32,
    rain_timer: f32,
    wave: u32,
    next_wave_at: f32,
    wave_end_at: Option<f32>,
    breather_until: f32,
    slow_time: f32,
    combo: u32,
    combo_timer: f32,
    near_misses: u32,
    best_before: u32,
    passed_best: bool,
    out_of_time: bool,
    death_timer: Option<f32>,
    over_time: f32,
}

impl Run {
    fn new(mode: Mode, best_before: u32) -> Self {
        let cfg = mode.config();
        Run {
            mode,
            lives: cfg.lives,
            next_wave_at: cfg.wave_every * 0.65,
            cfg,
            player: Player::new(),
            obstacles: Vec::new(),
            pickups: Vec::new(),
            schedule: Vec::new(),
            time: 0.0,
            score: 0.0,
            coins: 0,
            rain_timer: 1.0,
            wave: 0,
            wave_end_at: None,
            breather_until: 0.0,
            slow_time: 0.0,
            combo: 0,
            combo_timer: 0.0,
            near_misses: 0,
            best_before,
            passed_best: false,
            out_of_time: false,
            death_timer: None,
            over_time: 0.0,
        }
    }

    fn alive(&self) -> bool {
        self.death_timer.is_none()
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

    fn world_time_scale(&self) -> f32 {
        if !self.alive() {
            0.3
        } else if self.slow_time > 0.0 {
            0.45
        } else {
            1.0
        }
    }

    fn combo_mult(&self) -> f32 {
        1.0 + 0.2 * self.combo as f32
    }

    fn time_left(&self) -> Option<f32> {
        self.cfg
            .time_limit
            .map(|limit| (limit - self.time).max(0.0))
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

    fn spawn_coin(&mut self, x: f32, y: f32) {
        self.pickups.push(Pickup {
            pos: vec2(x, y),
            kind: PickupKind::Coin,
            age: rand::gen_range(0.0, 1.0),
        });
    }
}

pub struct Game {
    phase: Phase,
    menu_index: usize,
    skin: usize,
    browse_skin: usize,
    unlocked: u32,
    wallet: u32,
    runs_played: u32,
    touch_seen: bool,
    high_scores: [u32; Mode::ALL.len()],
    audio: Audio,
    particles: Particles,
    texts: FloatingTexts,
    banner: Banner,
    background: Background,
    run: Run,
    pointer: Pointer,
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
        // The first skin is always unlocked.
        let unlocked = storage::get(storage::SLOT_UNLOCKED).unwrap_or(1.0) as u32 | 1;
        let mut skin =
            (storage::get(storage::SLOT_SKIN).unwrap_or(0.0) as usize).min(SKINS.len() - 1);
        if unlocked & (1 << skin) == 0 {
            skin = 0;
        }
        let muted = storage::get(storage::SLOT_MUTED).unwrap_or(0.0) != 0.0;
        let view = View::compute();
        Game {
            phase: Phase::Menu,
            menu_index: 0,
            skin,
            browse_skin: skin,
            unlocked,
            wallet: storage::get(storage::SLOT_WALLET).unwrap_or(0.0) as u32,
            runs_played: storage::get(storage::SLOT_RUNS).unwrap_or(0.0) as u32,
            touch_seen: false,
            high_scores,
            audio: Audio::load(muted).await,
            particles: Particles::default(),
            texts: FloatingTexts::default(),
            banner: Banner::default(),
            background: Background::new(),
            run: Run::new(Mode::Classic, 0),
            pointer: Pointer::default(),
            shake: 0.0,
            flash: 0.0,
            view,
            clock: 0.0,
        }
    }

    pub fn frame(&mut self) {
        // Clamp dt so a stalled tab or window drag doesn't teleport everything.
        let dt = get_frame_time().min(1.0 / 30.0);
        self.clock += dt;
        self.view = View::compute();
        if !touches().is_empty() {
            self.touch_seen = true;
        }
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
            self.banner.update(dt);
            let bg_speed = match self.phase {
                Phase::Playing | Phase::GameOver => {
                    self.run.fall_speed() * 0.35 * self.run.world_time_scale()
                }
                _ => 40.0,
            };
            self.background.update(dt, bg_speed);
            self.shake = (self.shake - dt * 40.0).max(0.0);
            self.flash = (self.flash - dt * 2.5).max(0.0);
        }

        clear_background(BLACK);
        // Smooth, rumbly shake instead of per-frame jitter.
        let shake = vec2((self.clock * 53.0).sin(), (self.clock * 41.0).cos()) * self.shake;
        self.view.apply(shake);

        let sky = if self.phase == Phase::Menu {
            self.selected_mode().config().sky
        } else {
            self.run.cfg.sky
        };
        self.background.draw(sky, self.clock);

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
                vw(),
                vh(),
                Color::new(1.0, 0.35, 0.3, self.flash * 0.35),
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
        let muted = !self.audio.muted();
        self.audio.set_muted(muted);
        storage::set(storage::SLOT_MUTED, if muted { 1.0 } else { 0.0 });
        self.audio.play(Sfx::Select);
    }

    fn selected_mode(&self) -> Mode {
        Mode::ALL[self.menu_index]
    }

    fn start_run(&mut self, mode: Mode) {
        self.run = Run::new(mode, self.high_scores[mode.index()]);
        self.particles.clear();
        self.texts.clear();
        self.banner = Banner::default();
        self.pointer = Pointer::default();
        self.phase = Phase::Playing;
        self.runs_played += 1;
        storage::set(storage::SLOT_RUNS, self.runs_played as f64);
        self.audio.play(Sfx::Start);
    }

    fn is_unlocked(&self, skin: usize) -> bool {
        self.unlocked & (1 << skin) != 0
    }

    fn browse(&mut self, delta: i32) {
        let n = SKINS.len() as i32;
        self.browse_skin = (self.browse_skin as i32 + delta).rem_euclid(n) as usize;
        if self.is_unlocked(self.browse_skin) {
            self.skin = self.browse_skin;
            storage::set(storage::SLOT_SKIN, self.skin as f64);
        }
        self.audio.play(Sfx::Select);
    }

    fn try_buy(&mut self, at: Vec2) {
        let skin = self.browse_skin;
        let price = SKINS[skin].price;
        if self.is_unlocked(skin) || self.wallet < price {
            return;
        }
        self.wallet -= price;
        self.unlocked |= 1 << skin;
        self.skin = skin;
        storage::set(storage::SLOT_WALLET, self.wallet as f64);
        storage::set(storage::SLOT_UNLOCKED, self.unlocked as f64);
        storage::set(storage::SLOT_SKIN, skin as f64);
        self.particles
            .burst(at, 40, rgb(SKINS[skin].trail, 1.0), 420.0);
        self.particles.burst(at, 20, GOLD, 300.0);
        self.audio.play(Sfx::Unlock);
        storage::vibrate(40);
    }

    /// Cheapest locked skin, to show "next unlock" progress.
    fn next_unlock(&self) -> Option<usize> {
        (0..SKINS.len())
            .filter(|i| !self.is_unlocked(*i))
            .min_by_key(|i| SKINS[*i].price)
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
            self.browse(-1);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.browse(1);
        }
        if is_key_pressed(KeyCode::B) {
            self.try_buy(vec2(vw() / 2.0, vh() / 2.0));
        }
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
            self.start_run(self.selected_mode());
        }
    }

    fn draw_menu(&mut self) {
        let (w, h) = (vw(), vh());
        let portrait = ui::is_portrait();
        let cfg = self.selected_mode().config();
        let accent = rgb(cfg.color, 1.0);

        // Top bar: sound toggle, (desktop) quit, wallet.
        let sound = Rect::new(14.0, 14.0, 60.0, 52.0);
        if ui::button(&self.view, sound, "", Color::new(1.0, 1.0, 1.0, 0.15)) {
            self.toggle_mute();
        }
        draw_speaker(sound.center() + vec2(0.0, 2.0), !self.audio.muted());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let quit = Rect::new(84.0, 14.0, 90.0, 52.0);
            if ui::button(&self.view, quit, "Quit", Color::new(1.0, 1.0, 1.0, 0.15)) {
                macroquad::miniquad::window::order_quit();
            }
        }
        draw_wallet(vec2(w - 20.0, 48.0), self.wallet);

        // Two columns on wide screens, one stacked column on phones.
        let (left_cx, right_cx, top) = if portrait {
            (w / 2.0, w / 2.0, ((h - 800.0) * 0.3).max(0.0) + 60.0)
        } else {
            (w * 0.3, w * 0.68, 70.0)
        };

        // Title and bouncing mascot.
        let bob = (self.clock * 2.5).sin() * 5.0;
        let title_y = top + if portrait { 50.0 } else { 70.0 };
        ui::text_shadow_centered(
            "RUSTY",
            left_cx,
            title_y + bob,
            70.0,
            Color::new(1.0, 0.62, 0.25, 1.0),
        );
        ui::text_shadow_centered(
            "SCAPE",
            left_cx,
            title_y + 62.0 + bob,
            70.0,
            Color::new(1.0, 0.85, 0.35, 1.0),
        );
        let skin = &SKINS[self.browse_skin];
        let mascot_y = title_y + if portrait { 190.0 } else { 175.0 };
        let hop = ((self.clock * 3.0).sin().abs()) * 14.0;
        let mascot = if self.clock % 3.5 < 0.15 {
            Mood::Blink
        } else {
            Mood::Happy
        };
        let look = vec2((self.clock * 0.8).sin(), 0.2);
        draw_buddy(
            left_cx,
            mascot_y - hop,
            84.0,
            84.0,
            rgb(skin.body, 1.0),
            look,
            mascot,
            1.0,
        );
        draw_ellipse(
            left_cx,
            mascot_y + 6.0,
            40.0 - hop * 0.8,
            7.0,
            0.0,
            Color::new(0.0, 0.0, 0.0, 0.2),
        );
        if !self.is_unlocked(self.browse_skin) {
            draw_lock(vec2(left_cx + 40.0, mascot_y - 80.0 - hop));
        }

        // Skin picker under the mascot.
        let picker_y = mascot_y + 18.0;
        let prev = Rect::new(left_cx - 150.0, picker_y, 56.0, 52.0);
        let next = Rect::new(left_cx + 94.0, picker_y, 56.0, 52.0);
        if ui::button(&self.view, prev, "<", shade(accent, 0.8)) {
            self.browse(-1);
        }
        if ui::button(&self.view, next, ">", shade(accent, 0.8)) {
            self.browse(1);
        }
        ui::text_shadow_centered(skin.name, left_cx, picker_y + 36.0, 30.0, WHITE);

        // Mode list.
        let row_w = if portrait {
            (w - 40.0).min(440.0)
        } else {
            (w * 0.5).min(440.0)
        };
        let row_h = 52.0;
        let list_y = if portrait {
            picker_y + 74.0
        } else {
            top + 20.0
        };
        for (i, mode) in Mode::ALL.iter().enumerate() {
            let mcfg = mode.config();
            let rect = Rect::new(
                right_cx - row_w / 2.0,
                list_y + i as f32 * (row_h + 8.0),
                row_w,
                row_h,
            );
            let selected = i == self.menu_index;
            let color = rgb(mcfg.color, 1.0);
            if ui::clicked_in(&self.view, rect) {
                if selected {
                    self.start_run(*mode);
                    return;
                }
                self.menu_index = i;
                self.audio.play(Sfx::Select);
            }
            let pop = if selected { 4.0 } else { 0.0 };
            let card = Rect::new(rect.x - pop, rect.y, rect.w + pop * 2.0, rect.h);
            if selected {
                ui::rounded_rect(
                    card.x,
                    card.y + 4.0,
                    card.w,
                    card.h,
                    16.0,
                    shade(color, 0.4),
                );
                ui::rounded_rect(card.x, card.y, card.w, card.h, 16.0, color);
            } else {
                ui::rounded_rect(
                    card.x,
                    card.y,
                    card.w,
                    card.h,
                    16.0,
                    Color::new(1.0, 1.0, 1.0, 0.12),
                );
            }
            let name_color = if selected {
                WHITE
            } else {
                Color::new(1.0, 1.0, 1.0, 0.75)
            };
            ui::text(mcfg.name, card.x + 22.0, card.y + 35.0, 28.0, name_color);
            let best = format!("{}", self.high_scores[i]);
            let bw = ui::text_width(&best, 24.0);
            ui::text(
                &best,
                card.right() - bw - 20.0,
                card.y + 34.0,
                24.0,
                name_color,
            );
            draw_trophy(
                vec2(card.right() - bw - 38.0, card.y + 25.0),
                fade(name_color, 0.9),
            );
        }
        let after_list = list_y + Mode::ALL.len() as f32 * (row_h + 8.0);
        ui::text_centered(
            cfg.tagline,
            right_cx,
            after_list + 22.0,
            22.0,
            Color::new(1.0, 1.0, 1.0, 0.85),
        );

        // Main action: PLAY, or BUY when browsing a locked skin.
        let play_y = if portrait {
            after_list + 48.0
        } else {
            picker_y + 76.0
        };
        let play = Rect::new(left_cx - 130.0, play_y, 260.0, 72.0);
        if self.is_unlocked(self.browse_skin) {
            let pulse = 1.0 + (self.clock * 4.0).sin() * 0.04;
            let grow = Rect::new(
                play.x - play.w * (pulse - 1.0) / 2.0,
                play.y,
                play.w * pulse,
                play.h,
            );
            if ui::button(&self.view, grow, "PLAY", Color::new(0.25, 0.8, 0.4, 1.0)) {
                self.start_run(self.selected_mode());
            }
        } else {
            let price = skin.price;
            let affordable = self.wallet >= price;
            let label = if affordable {
                format!("UNLOCK {price}")
            } else {
                format!("NEED {price}")
            };
            let color = if affordable {
                Color::new(1.0, 0.75, 0.15, 1.0)
            } else {
                Color::new(0.45, 0.45, 0.5, 1.0)
            };
            if ui::button(&self.view, play, &label, color) && affordable {
                self.try_buy(vec2(left_cx, mascot_y - 40.0));
            }
            if !affordable {
                ui::text_centered(
                    "Collect coins in any mode!",
                    left_cx,
                    play.bottom() + 30.0,
                    20.0,
                    Color::new(1.0, 0.9, 0.5, 0.9),
                );
            }
        }

        if !portrait && !self.touch_seen {
            ui::text_centered(
                "Arrows: choose   Enter: play   M: sound   F: fullscreen",
                w / 2.0,
                h - 14.0,
                18.0,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
    }

    // ------------------------------------------------------------- playing

    fn update_playing(&mut self, dt: f32) {
        if self.run.alive()
            && (is_key_pressed(KeyCode::Escape)
                || is_key_pressed(KeyCode::P)
                || ui::clicked_in(&self.view, pause_button()))
        {
            self.phase = Phase::Paused;
            self.pointer.active = false;
            self.run.player.target = None;
            return;
        }

        // Dramatic slow-motion beat after the fatal hit.
        if let Some(t) = self.run.death_timer {
            let left = t - dt;
            self.run.death_timer = Some(left);
            self.move_world(dt * self.run.world_time_scale());
            if left <= 0.0 {
                self.end_run();
            }
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
        self.direct(world_dt);
        self.move_world(world_dt);
        self.check_collisions();
        if !self.run.alive() {
            return;
        }

        let run = &mut self.run;
        run.slow_time = (run.slow_time - dt).max(0.0);
        run.player.magnet = (run.player.magnet - dt).max(0.0);
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

        if !run.passed_best && run.best_before > 0 && run.score as u32 > run.best_before {
            run.passed_best = true;
            self.banner
                .show("NEW BEST!", "keep going!", Color::new(0.4, 1.0, 0.5, 1.0));
            self.audio.play(Sfx::NewBest);
            storage::vibrate(30);
        }
    }

    /// Reads drag gestures. Returns true when a tap (dash) was released.
    fn update_pointer(&mut self) -> bool {
        let m = self.view.mouse();
        let mut tapped = false;
        if is_mouse_button_pressed(MouseButton::Left) && !pause_button().contains(m) {
            self.pointer = Pointer {
                active: true,
                last: m,
                down_at: self.clock,
                travel: 0.0,
            };
            self.run.player.target = Some(self.run.player.x);
        }
        if self.pointer.active {
            if is_mouse_button_down(MouseButton::Left) {
                let delta = m - self.pointer.last;
                self.pointer.travel += delta.length();
                self.pointer.last = m;
                let half = PLAYER_SIZE / 2.0;
                let player = &mut self.run.player;
                let target = player.target.unwrap_or(player.x) + delta.x * DRAG_GAIN;
                player.target = Some(target.clamp(half, vw() - half));
            } else {
                tapped =
                    self.clock - self.pointer.down_at < TAP_TIME && self.pointer.travel < TAP_SLOP;
                self.pointer.active = false;
                self.run.player.target = None;
            }
        }
        tapped
    }

    fn update_player(&mut self, dt: f32) {
        let mut dir = 0.0;
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            dir -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            dir += 1.0;
        }
        let tapped = self.update_pointer();
        let dash = tapped
            || [
                KeyCode::Space,
                KeyCode::LeftShift,
                KeyCode::RightShift,
                KeyCode::Up,
                KeyCode::W,
            ]
            .iter()
            .any(|k| is_key_pressed(*k))
            || is_mouse_button_pressed(MouseButton::Right)
            || {
                let t = touches();
                t.len() >= 2 && t.iter().any(|t| t.phase == TouchPhase::Started)
            };

        let max = max_speed();
        let accel = max * 9.0;
        let p = &mut self.run.player;
        p.dash_cooldown = (p.dash_cooldown - dt).max(0.0);
        p.invuln = (p.invuln - dt).max(0.0);
        p.happy = (p.happy - dt).max(0.0);
        p.blink_in -= dt;
        if p.blink_in < -0.12 {
            p.blink_in = rand::gen_range(1.5, 4.0);
        }

        if dash && p.dash_cooldown <= 0.0 {
            if dir != 0.0 {
                p.facing = dir;
            } else if p.vx.abs() > 30.0 {
                p.facing = p.vx.signum();
            }
            p.dash_time = DASH_TIME;
            p.dash_cooldown = DASH_COOLDOWN;
            // Dragging keeps control after the dash: move the target along.
            if let Some(t) = p.target.as_mut() {
                *t += p.facing * vw() * 0.2;
            }
            self.audio.play(Sfx::Dash);
            storage::vibrate(12);
            self.particles
                .burst(p.center(), 12, rgb(SKINS[self.skin].trail, 0.9), 260.0);
        }

        if p.dash_time > 0.0 {
            p.dash_time -= dt;
            p.vx = p.facing * vw() * 1.7;
        } else if dir != 0.0 {
            // Snappy turnarounds: braking is stronger than accelerating.
            let a = if p.vx * dir < 0.0 { accel * 2.0 } else { accel };
            p.vx = (p.vx + dir * a * dt).clamp(-max, max);
            p.facing = dir;
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
        let half = PLAYER_SIZE / 2.0;
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
    fn direct(&mut self, dt: f32) {
        let now = self.run.time;

        // Release queued pattern blocks.
        let player_x = self.run.player.x;
        let mut i = 0;
        while i < self.run.schedule.len() {
            if self.run.schedule[i].at <= now {
                let s = self.run.schedule.swap_remove(i);
                let x = if s.aimed { player_x } else { s.x };
                self.run.spawn_block(x, s.w, s.h);
            } else {
                i += 1;
            }
        }

        // Wave cleared: reward, breathe, then rain again.
        if let Some(end) = self.run.wave_end_at {
            if now >= end && self.run.schedule.is_empty() {
                self.wave_cleared();
            }
            return;
        }

        if now >= self.run.next_wave_at {
            self.queue_wave();
            return;
        }

        if now < self.run.breather_until {
            return;
        }
        self.rain(dt);
    }

    fn rain(&mut self, dt: f32) {
        let run = &mut self.run;
        run.rain_timer -= dt;
        if run.rain_timer > 0.0 {
            return;
        }
        run.rain_timer = run.rain_interval() * rand::gen_range(0.7, 1.3);
        let d = run.difficulty();
        let (w, h) = if run.cfg.drifting {
            (rand::gen_range(40.0, 190.0), rand::gen_range(40.0, 90.0))
        } else if d > 1.5 && rand::gen_range(0.0, 1.0) < 0.12 {
            // Occasional wide slab once things heat up.
            (rand::gen_range(150.0, 240.0f32).min(vw() * 0.5), 48.0)
        } else {
            let s = rand::gen_range(52.0, 84.0);
            (s, s * rand::gen_range(0.85, 1.15))
        };
        let cx = rand::gen_range(w / 2.0, vw() - w / 2.0);
        run.spawn_block(cx, w, h);

        let roll = rand::gen_range(0.0, 1.0);
        let y = -30.0;
        if roll < run.cfg.coin_chance {
            run.spawn_coin(rand::gen_range(30.0, vw() - 30.0), y);
        } else if roll < run.cfg.coin_chance + run.cfg.powerup_chance {
            let kind = match rand::gen_range(0, 3) {
                0 => PickupKind::Shield,
                1 => PickupKind::Slow,
                _ => PickupKind::Magnet,
            };
            run.pickups.push(Pickup {
                pos: vec2(rand::gen_range(40.0, vw() - 40.0), y),
                kind,
                age: 0.0,
            });
        }
    }

    fn queue_wave(&mut self) {
        let run = &mut self.run;
        run.wave += 1;
        let wave = run.wave;
        let t0 = run.time;
        let intensity = (wave as f32 - 1.0).min(8.0);
        let w = vw();
        let pattern = match wave {
            1 => 0,
            2 => 1,
            3 => 2,
            _ => rand::gen_range(0, 5),
        };
        let mut push = |at: f32, x: f32, bw: f32, bh: f32, aimed: bool| {
            run.schedule.push(Scheduled {
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
        run.wave_end_at = Some(last_at + run.fall_time() + WARN_TIME);
        self.banner
            .show(format!("WAVE {wave}"), name, rgb(self.run.cfg.color, 1.0));
        self.audio.play(Sfx::Wave);
    }

    fn wave_cleared(&mut self) {
        let run = &mut self.run;
        run.wave_end_at = None;
        run.breather_until = run.time + 1.4;
        run.next_wave_at = run.time + run.cfg.wave_every;
        let bonus = (50.0 * run.wave as f32 * run.cfg.score_mult).round();
        run.score += bonus;
        // A rainbow of coins as a reward.
        let n = 7;
        for i in 0..n {
            let t = i as f32 / (n - 1) as f32;
            let x = vw() * (0.15 + 0.7 * t);
            let y = -30.0 - (t * PI).sin() * 90.0;
            run.spawn_coin(x, y);
        }
        self.texts.spawn_sized(
            format!("WAVE CLEAR +{bonus}"),
            vec2(vw() / 2.0, vh() * 0.42),
            Color::new(0.5, 1.0, 0.6, 1.0),
            40.0,
        );
        self.run.player.happy = 1.2;
        self.audio.play(Sfx::PowerUp);
    }

    fn move_world(&mut self, dt: f32) {
        let run = &mut self.run;
        let w = vw();
        for o in &mut run.obstacles {
            o.rect.x += o.vel.x * dt;
            o.rect.y += o.vel.y * dt;
            if o.rect.x < 0.0 || o.rect.right() > w {
                o.vel.x = -o.vel.x;
                o.rect.x = o.rect.x.clamp(0.0, (w - o.rect.w).max(0.0));
            }
        }
        let bottom = vh() + 10.0;
        run.obstacles.retain(|o| o.rect.y < bottom);

        let pickup_speed = 160.0 + run.fall_speed() * 0.2;
        let center = run.player.center();
        let magnet = run.player.magnet > 0.0;
        for p in &mut run.pickups {
            p.age += dt;
            p.pos.y += pickup_speed * dt;
            p.pos.x += (p.age * 3.0).sin() * 40.0 * dt;
            if magnet && p.kind == PickupKind::Coin {
                let to = center - p.pos;
                let d = to.length();
                if d < MAGNET_RANGE && d > 1.0 {
                    p.pos += to / d * 700.0 * dt;
                }
            }
        }
        run.pickups.retain(|p| p.pos.y < bottom + 30.0);
    }

    fn check_collisions(&mut self) {
        let hitbox = self.run.player.hitbox();
        let phasing = self.run.player.is_dashing() || self.run.player.invuln > 0.0;

        let mut hit_index = None;
        let mut near_misses = 0;
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
                    near_misses += 1;
                }
            }
        }
        for _ in 0..near_misses {
            let run = &mut self.run;
            run.combo = (run.combo + 1).min(MAX_COMBO);
            run.combo_timer = COMBO_WINDOW;
            run.near_misses += 1;
            run.player.happy = 0.6;
            let bonus = (10.0 * run.cfg.score_mult * run.combo as f32).round();
            run.score += bonus;
            self.texts.spawn(
                format!("CLOSE! +{bonus}"),
                vec2(hitbox.center().x, player_y() - 60.0),
                YELLOW,
            );
            self.audio.play(Sfx::NearMiss);
            storage::vibrate(8);
        }
        if let Some(i) = hit_index {
            let o = self.run.obstacles.swap_remove(i);
            self.on_hit(o);
            if !self.run.alive() {
                return;
            }
        }

        let center = self.run.player.center();
        let reach = PLAYER_SIZE * 0.5 + 24.0;
        let mut collected = Vec::new();
        self.run.pickups.retain(|p| {
            let hit = p.pos.distance(center) < reach;
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
            .burst(obstacle.rect.center(), 22, obstacle.color, 420.0);
        self.shake = 12.0;

        if self.run.player.shield {
            self.run.player.shield = false;
            self.run.player.invuln = 0.8;
            self.particles.burst(center, 24, SKYBLUE, 320.0);
            self.texts
                .spawn("SHIELD SAVED YOU!", center - vec2(0.0, 60.0), SKYBLUE);
            self.audio.play(Sfx::Hit);
            storage::vibrate(50);
            return;
        }

        self.run.lives -= 1;
        self.run.combo = 0;
        self.flash = 1.0;
        self.audio.play(Sfx::Hit);
        storage::vibrate(120);
        self.particles
            .burst(center, 40, rgb(SKINS[self.skin].body, 1.0), 520.0);
        if self.run.lives == 0 {
            self.shake = 18.0;
            self.run.death_timer = Some(DEATH_TIME);
            self.pointer.active = false;
        } else {
            self.run.player.invuln = HIT_INVULN;
            let left = self.run.lives;
            let msg = if left == 1 {
                "LAST HEART!".to_string()
            } else {
                format!("{left} HEARTS LEFT")
            };
            self.texts.spawn(
                msg,
                center - vec2(0.0, 60.0),
                Color::new(1.0, 0.4, 0.45, 1.0),
            );
        }
    }

    fn collect(&mut self, kind: PickupKind) {
        let center = self.run.player.center();
        let label_at = center - vec2(0.0, 60.0);
        let run = &mut self.run;
        run.player.happy = 0.5;
        match kind {
            PickupKind::Coin => {
                let value = (20.0 * run.cfg.score_mult * run.combo_mult()).round();
                run.score += value;
                run.coins += 1;
                self.texts.spawn(format!("+{value}"), label_at, GOLD);
                self.particles.burst(center, 10, GOLD, 220.0);
                self.audio.play(Sfx::Coin);
                storage::vibrate(6);
            }
            PickupKind::Shield => {
                run.player.shield = true;
                self.texts.spawn("SHIELD!", label_at, SKYBLUE);
                self.audio.play(Sfx::PowerUp);
            }
            PickupKind::Slow => {
                run.slow_time = SLOW_TIME;
                self.texts.spawn("SLOW-MO!", label_at, VIOLET);
                self.audio.play(Sfx::PowerUp);
            }
            PickupKind::Magnet => {
                run.player.magnet = MAGNET_TIME;
                self.texts
                    .spawn("MAGNET!", label_at, Color::new(1.0, 0.4, 0.4, 1.0));
                self.audio.play(Sfx::PowerUp);
            }
        }
    }

    fn end_run(&mut self) {
        let score = self.run.score as u32;
        let i = self.run.mode.index();
        if score > self.high_scores[i] {
            self.high_scores[i] = score;
            storage::set(storage::high_score_slot(i), score as f64);
        }
        self.wallet += self.run.coins;
        storage::set(storage::SLOT_WALLET, self.wallet as f64);
        self.run.over_time = 0.0;
        self.phase = Phase::GameOver;
        self.audio.play(if self.new_best() {
            Sfx::NewBest
        } else {
            Sfx::GameOver
        });
    }

    fn new_best(&self) -> bool {
        self.run.score as u32 > self.run.best_before
    }

    // ----------------------------------------------------------- game over

    fn update_game_over(&mut self, dt: f32) {
        self.run.over_time += dt;
        // Let the world keep falling behind the results screen.
        self.move_world(dt * 0.4);
        // Brief input lock so a frantic keypress doesn't skip the results.
        if self.run.over_time < 0.7 {
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
        let (w, h) = (vw(), vh());

        // Ground.
        let fy = floor_y();
        let ground = shade(rgb(run.cfg.sky.1, 1.0), 0.6);
        draw_rectangle(-30.0, fy, w + 60.0, h - fy + 30.0, ground);
        draw_rectangle(-30.0, fy, w + 60.0, 5.0, fade(rgb(run.cfg.color, 1.0), 0.8));

        // Telegraph incoming blocks: a faint lane plus a pulsing arrow.
        let pulse = 0.6 + 0.4 * (self.clock * 14.0).sin();
        for o in &run.obstacles {
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

        let target = run.player.center();
        for o in &run.obstacles {
            if o.rect.bottom() < 0.0 {
                continue;
            }
            draw_block(o, target);
        }

        for p in &run.pickups {
            draw_pickup(p);
        }

        if run.alive() && self.phase != Phase::GameOver {
            self.draw_player();
        }
        self.particles.draw();
        self.texts.draw();
        self.banner.draw();

        if run.slow_time > 0.0 {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.4, 0.3, 1.0, 0.08));
        }
    }

    fn mood(&self) -> Mood {
        let p = &self.run.player;
        if p.blink_in < 0.0 {
            return Mood::Blink;
        }
        if p.happy > 0.0 {
            return Mood::Happy;
        }
        // Look scared when something is about to land on us.
        let danger = self.run.obstacles.iter().any(|o| {
            let dy = player_y() - o.rect.bottom();
            (0.0..170.0).contains(&dy) && o.rect.x < p.x + 60.0 && o.rect.right() > p.x - 60.0
        });
        if danger {
            Mood::Scared
        } else {
            Mood::Normal
        }
    }

    fn draw_player(&self) {
        let p = &self.run.player;
        let skin = &SKINS[self.skin];
        let py = player_y();

        // Blink while invulnerable after a hit.
        if p.invuln > 0.0 && !p.is_dashing() && (p.invuln * 12.0) as i32 % 2 == 0 {
            return;
        }

        for (i, x) in p.trail.iter().enumerate().skip(1) {
            let t = 1.0 - i as f32 / TRAIL_LEN as f32;
            let r = PLAYER_SIZE * (0.15 + 0.3 * t);
            let a = if p.is_dashing() { 0.6 } else { 0.22 } * t;
            draw_circle(*x, py + 4.0, r, rgb(skin.trail, a));
        }

        // Shadow on the ground.
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
        let w = PLAYER_SIZE * (1.0 + 0.16 * speed);
        let h = PLAYER_SIZE * (1.0 - 0.1 * speed);
        let alpha = if p.is_dashing() { 0.6 } else { 1.0 };
        let look = vec2(p.facing * (0.4 + 0.6 * speed.min(1.0)), -0.2);
        draw_buddy(
            p.x,
            floor_y() - 2.0,
            w,
            h,
            rgb(skin.body, alpha),
            look,
            self.mood(),
            alpha,
        );

        if p.shield {
            let r = PLAYER_SIZE * 0.82 + (self.clock * 6.0).sin() * 2.5;
            draw_circle(p.x, py, r, fade(SKYBLUE, 0.12));
            draw_circle_lines(p.x, py, r, 3.0, fade(SKYBLUE, 0.85));
        }
        if p.magnet > 0.0 {
            let r = 40.0 + (self.clock * 3.0).fract() * 60.0;
            draw_circle_lines(
                p.x,
                py,
                r,
                2.0,
                Color::new(1.0, 0.4, 0.4, 0.5 * (1.0 - (r - 40.0) / 60.0)),
            );
        }

        // Dash cooldown bar under the player.
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
                rgb(skin.trail, 0.95),
            );
        }
    }

    fn draw_hud(&self) {
        let run = &self.run;
        let w = vw();
        let accent = rgb(run.cfg.color, 1.0);

        // Score, centered and big.
        let score = run.score as u32;
        let pop = 1.0
            + 0.15
                * (run.combo_timer / COMBO_WINDOW).clamp(0.0, 1.0)
                * (run.combo > 0) as u8 as f32;
        ui::text_shadow_centered(&format!("{score}"), w / 2.0, 62.0, 54.0 * pop, WHITE);
        let best_line = if run.passed_best {
            "NEW BEST!".to_string()
        } else {
            format!("BEST {}", run.best_before)
        };
        let best_color = if run.passed_best {
            Color::new(0.5, 1.0, 0.6, 1.0)
        } else {
            Color::new(1.0, 1.0, 1.0, 0.6)
        };
        if run.best_before > 0 || run.passed_best {
            ui::text_centered(&best_line, w / 2.0, 88.0, 20.0, best_color);
        }

        // Combo meter.
        if run.combo > 0 {
            let t = (run.combo_timer / COMBO_WINDOW).clamp(0.0, 1.0);
            let label = format!("x{:.1} COMBO", run.combo_mult());
            ui::text_shadow_centered(&label, w / 2.0, 120.0, 28.0, YELLOW);
            ui::rounded_rect(
                w / 2.0 - 60.0,
                128.0,
                120.0,
                8.0,
                4.0,
                Color::new(0.0, 0.0, 0.0, 0.3),
            );
            ui::rounded_rect(
                w / 2.0 - 60.0,
                128.0,
                (120.0 * t).max(8.0),
                8.0,
                4.0,
                YELLOW,
            );
        }

        // Left: wave and coins. Lives below.
        let wave = format!("WAVE {}", run.wave.max(1));
        ui::text(&wave, 18.0, 40.0, 26.0, accent);
        draw_coin_icon(vec2(30.0, 62.0), 11.0);
        ui::text(&format!("{}", run.coins), 48.0, 71.0, 24.0, GOLD);
        if run.cfg.lives > 1 {
            for i in 0..run.cfg.lives {
                let c = if i < run.lives {
                    Color::new(1.0, 0.3, 0.4, 1.0)
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.18)
                };
                draw_heart(vec2(30.0 + i as f32 * 30.0, 92.0), 24.0, c);
            }
        }

        if let Some(left) = run.time_left() {
            let c = if left < 10.0 {
                Color::new(1.0, 0.4, 0.4, 1.0)
            } else {
                WHITE
            };
            ui::text_shadow_centered(&format!("{:.0}s", left.ceil()), w - 120.0, 58.0, 40.0, c);
        }

        // Pause button.
        let b = pause_button();
        ui::rounded_rect(b.x, b.y, b.w, b.h, 14.0, Color::new(1.0, 1.0, 1.0, 0.15));
        ui::rounded_rect(b.x + 19.0, b.y + 16.0, 8.0, 28.0, 3.0, WHITE);
        ui::rounded_rect(b.x + 33.0, b.y + 16.0, 8.0, 28.0, 3.0, WHITE);

        // Control hints for the first few runs.
        if self.runs_played <= TUTORIAL_RUNS && run.time < 4.5 {
            let a = (4.5 - run.time).min(1.0);
            let touch = self.touch_seen || ui::is_portrait();
            let (l1, l2) = if touch {
                ("Drag anywhere to move", "Tap to dash through blocks")
            } else {
                ("Arrows / A-D to move", "Space to dash through blocks")
            };
            let y = vh() * 0.55;
            ui::text_shadow_centered(l1, w / 2.0, y, 30.0, fade(WHITE, a));
            ui::text_shadow_centered(
                l2,
                w / 2.0,
                y + 36.0,
                24.0,
                fade(Color::new(1.0, 0.9, 0.5, 1.0), a),
            );
        }
    }

    fn draw_pause(&mut self) {
        let (w, h) = (vw(), vh());
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
        let cx = w / 2.0;
        let accent = rgb(self.run.cfg.color, 1.0);
        let y = h * 0.36;
        ui::text_shadow_centered("PAUSED", cx, y, 72.0, WHITE);
        let resume = Rect::new(cx - 130.0, y + 40.0, 260.0, 72.0);
        let menu = Rect::new(cx - 130.0, y + 130.0, 260.0, 60.0);
        let resume_clicked = ui::button(
            &self.view,
            resume,
            "RESUME",
            Color::new(0.25, 0.8, 0.4, 1.0),
        );
        let menu_clicked = ui::button(&self.view, menu, "Menu", shade(accent, 0.8));
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
        let (w, h) = (vw(), vh());
        let run = &self.run;
        let t = (run.over_time * 3.0).min(1.0);
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5 * t));
        let cx = w / 2.0;
        let accent = rgb(run.cfg.color, 1.0);
        let new_best = self.new_best();

        // Results card.
        let card_w = (w - 32.0).min(460.0);
        let card_h = 400.0;
        let top = ((h - card_h) / 2.0 - 30.0).max(20.0) + (1.0 - t) * 60.0;
        ui::rounded_rect(
            cx - card_w / 2.0,
            top + 6.0,
            card_w,
            card_h,
            26.0,
            Color::new(0.0, 0.0, 0.0, 0.3 * t),
        );
        ui::rounded_rect(
            cx - card_w / 2.0,
            top,
            card_w,
            card_h,
            26.0,
            fade(Color::new(0.12, 0.11, 0.22, 0.96), t),
        );

        let title = if run.out_of_time {
            "TIME'S UP!"
        } else if new_best {
            "AMAZING!"
        } else {
            "OUCH!"
        };
        ui::text_shadow_centered(title, cx, top + 66.0, 56.0, fade(accent, t));
        ui::text_shadow_centered(
            &format!("{}", run.score as u32),
            cx,
            top + 140.0,
            72.0,
            fade(WHITE, t),
        );
        if new_best {
            let pulse = 0.75 + 0.25 * (self.clock * 8.0).sin();
            ui::text_centered(
                "NEW HIGH SCORE!",
                cx,
                top + 178.0,
                28.0,
                fade(Color::new(0.4, 1.0, 0.5, 1.0), t * pulse),
            );
        } else {
            let best = self.high_scores[run.mode.index()];
            ui::text_centered(
                &format!("Best {best}"),
                cx,
                top + 178.0,
                26.0,
                fade(Color::new(1.0, 1.0, 1.0, 0.7), t),
            );
        }

        // Stats row.
        let stats = format!(
            "Wave {}   ·   {:.0}s   ·   {} close call{}",
            run.wave.max(1),
            run.time,
            run.near_misses,
            if run.near_misses == 1 { "" } else { "s" }
        );
        ui::text_centered(
            &stats,
            cx,
            top + 214.0,
            20.0,
            fade(Color::new(1.0, 1.0, 1.0, 0.75), t),
        );

        // Coins earned and progress toward the next skin.
        draw_coin_icon(vec2(cx - 70.0, top + 245.0), 13.0);
        ui::text(
            &format!("+{}  ({})", run.coins, self.wallet),
            cx - 50.0,
            top + 255.0,
            26.0,
            fade(GOLD, t),
        );
        if let Some(next) = self.next_unlock() {
            let price = SKINS[next].price.max(1);
            let p = (self.wallet as f32 / price as f32).min(1.0);
            let bw = card_w - 80.0;
            let bx = cx - bw / 2.0;
            ui::rounded_rect(
                bx,
                top + 272.0,
                bw,
                12.0,
                6.0,
                Color::new(0.0, 0.0, 0.0, 0.35 * t),
            );
            ui::rounded_rect(
                bx,
                top + 272.0,
                (bw * p).max(12.0),
                12.0,
                6.0,
                fade(GOLD, t),
            );
            let label = if p >= 1.0 {
                format!("{} ready to unlock!", SKINS[next].name)
            } else {
                format!(
                    "{} more coins for {}",
                    price - self.wallet.min(price),
                    SKINS[next].name
                )
            };
            ui::text_centered(
                &label,
                cx,
                top + 306.0,
                20.0,
                fade(Color::new(1.0, 0.9, 0.6, 1.0), t),
            );
        }

        if run.over_time < 0.7 {
            return;
        }
        let retry = Rect::new(
            cx - card_w / 2.0 + 24.0,
            top + card_h - 82.0,
            card_w * 0.58,
            64.0,
        );
        let menu = Rect::new(
            retry.right() + 12.0,
            retry.y,
            cx + card_w / 2.0 - 24.0 - retry.right() - 12.0,
            64.0,
        );
        if ui::button(&self.view, retry, "RETRY", Color::new(0.25, 0.8, 0.4, 1.0)) {
            self.start_run(self.run.mode);
        } else if ui::button(&self.view, menu, "Menu", shade(accent, 0.8)) {
            self.phase = Phase::Menu;
        }
    }
}

// ------------------------------------------------------------------ art

/// The player character: a squishy block with a face.
#[allow(clippy::too_many_arguments)]
fn draw_buddy(
    cx: f32,
    bottom: f32,
    w: f32,
    h: f32,
    body: Color,
    look: Vec2,
    mood: Mood,
    alpha: f32,
) {
    let x = cx - w / 2.0;
    let y = bottom - h;
    ui::chunky_rect(x, y, w, h, w * 0.3, body);

    let ink = Color::new(0.1, 0.08, 0.15, alpha);
    let eye_y = y + h * 0.42;
    let eye_dx = w * 0.2;
    let r = w * 0.13;
    for side in [-1.0, 1.0] {
        let ex = cx + side * eye_dx;
        match mood {
            Mood::Blink => {
                draw_line(ex - r, eye_y, ex + r, eye_y, 3.5, ink);
            }
            Mood::Happy => {
                // ^ ^ eyes.
                draw_line(ex - r, eye_y + r * 0.3, ex, eye_y - r * 0.5, 4.0, ink);
                draw_line(ex, eye_y - r * 0.5, ex + r, eye_y + r * 0.3, 4.0, ink);
            }
            Mood::Normal | Mood::Scared => {
                let er = if mood == Mood::Scared { r * 1.2 } else { r };
                draw_circle(ex, eye_y, er, Color::new(1.0, 1.0, 1.0, alpha));
                let pr = if mood == Mood::Scared {
                    r * 0.4
                } else {
                    r * 0.58
                };
                let off = look.clamp_length_max(1.0) * (er - pr) * 0.9;
                draw_circle(ex + off.x, eye_y + off.y, pr, ink);
                draw_circle(
                    ex + off.x - pr * 0.3,
                    eye_y + off.y - pr * 0.35,
                    pr * 0.3,
                    Color::new(1.0, 1.0, 1.0, alpha),
                );
            }
        }
    }

    // Rosy cheeks.
    let cheek = Color::new(1.0, 0.45, 0.5, 0.35 * alpha);
    draw_circle(cx - w * 0.31, y + h * 0.62, w * 0.07, cheek);
    draw_circle(cx + w * 0.31, y + h * 0.62, w * 0.07, cheek);

    let mouth_y = y + h * 0.64;
    match mood {
        Mood::Scared => draw_circle(cx, mouth_y + 2.0, w * 0.07, ink),
        Mood::Happy => half_disk(vec2(cx, mouth_y - 2.0), w * 0.13, ink),
        Mood::Normal | Mood::Blink => {
            let mw = w * 0.11;
            draw_line(
                cx - mw,
                mouth_y - 2.0,
                cx - mw * 0.4,
                mouth_y + 2.0,
                3.0,
                ink,
            );
            draw_line(
                cx - mw * 0.4,
                mouth_y + 2.0,
                cx + mw * 0.4,
                mouth_y + 2.0,
                3.0,
                ink,
            );
            draw_line(
                cx + mw * 0.4,
                mouth_y + 2.0,
                cx + mw,
                mouth_y - 2.0,
                3.0,
                ink,
            );
        }
    }
}

/// Lower half of a disk (a big open smile).
fn half_disk(c: Vec2, r: f32, color: Color) {
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

/// A falling block: chunky and grumpy, eyeing the player.
fn draw_block(o: &Obstacle, target: Vec2) {
    let r = o.rect;
    let radius = (r.w.min(r.h) * 0.25).min(16.0);
    ui::chunky_rect(r.x, r.y, r.w, r.h, radius, o.color);

    let size = r.w.min(r.h);
    if size < 38.0 {
        return;
    }
    let c = r.center();
    let eye_r = (size * 0.13).min(10.0);
    let spread = (r.w * 0.18).clamp(eye_r * 1.6, 26.0);
    let ink = Color::new(0.08, 0.06, 0.14, 1.0);
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
            ink,
        );
        // Grumpy brows slanting toward the middle.
        let by = ey - eye_r - 3.0;
        draw_line(
            ex - side * eye_r,
            by - 4.0,
            ex + side * eye_r * 0.6,
            by + 2.0,
            3.5,
            ink,
        );
    }
    let mw = (r.w * 0.1).min(12.0);
    draw_line(
        c.x - mw,
        c.y + size * 0.25,
        c.x + mw,
        c.y + size * 0.25,
        3.0,
        ink,
    );
}

fn draw_pickup(p: &Pickup) {
    let pulse = 1.0 + (p.age * 8.0).sin() * 0.1;
    let (x, y) = (p.pos.x, p.pos.y);
    match p.kind {
        PickupKind::Coin => {
            // Fake a spinning coin by squashing its width.
            let spin = (p.age * 4.0).cos().abs().max(0.25);
            let r = 15.0 * pulse;
            draw_ellipse(x, y, r * spin, r, 0.0, Color::new(0.85, 0.55, 0.05, 1.0));
            draw_ellipse(x, y, r * spin * 0.75, r * 0.75, 0.0, GOLD);
            draw_ellipse(
                x - r * spin * 0.25,
                y - r * 0.3,
                r * spin * 0.2,
                r * 0.22,
                0.0,
                Color::new(1.0, 1.0, 0.8, 0.8),
            );
        }
        PickupKind::Shield | PickupKind::Slow | PickupKind::Magnet => {
            let (color, label) = match p.kind {
                PickupKind::Shield => (SKYBLUE, "S"),
                PickupKind::Slow => (VIOLET, "T"),
                _ => (Color::new(1.0, 0.4, 0.4, 1.0), "M"),
            };
            let r = 20.0 * pulse;
            draw_circle(x, y, r + 8.0, fade(color, 0.2));
            draw_circle(x, y, r, shade(color, 0.6));
            draw_circle(x, y, r - 3.0, color);
            ui::text_centered(label, x, y + 9.0, 26.0, WHITE);
        }
    }
}

fn draw_coin_icon(c: Vec2, r: f32) {
    draw_circle(c.x, c.y, r, Color::new(0.85, 0.55, 0.05, 1.0));
    draw_circle(c.x, c.y, r * 0.75, GOLD);
}

fn draw_wallet(right_top: Vec2, coins: u32) {
    let label = format!("{coins}");
    let tw = ui::text_width(&label, 30.0);
    let w = tw + 64.0;
    let x = right_top.x - w;
    ui::rounded_rect(
        x,
        right_top.y - 34.0,
        w,
        52.0,
        16.0,
        Color::new(0.0, 0.0, 0.0, 0.25),
    );
    draw_coin_icon(vec2(x + 26.0, right_top.y - 8.0), 14.0);
    ui::text(&label, x + 46.0, right_top.y + 3.0, 30.0, GOLD);
}

fn draw_heart(c: Vec2, s: f32, color: Color) {
    draw_circle(c.x - s * 0.24, c.y - s * 0.1, s * 0.28, color);
    draw_circle(c.x + s * 0.24, c.y - s * 0.1, s * 0.28, color);
    draw_triangle(
        vec2(c.x - s * 0.5, c.y),
        vec2(c.x + s * 0.5, c.y),
        vec2(c.x, c.y + s * 0.5),
        color,
    );
}

fn draw_trophy(c: Vec2, color: Color) {
    ui::rounded_rect(c.x - 8.0, c.y - 9.0, 16.0, 12.0, 4.0, color);
    draw_rectangle(c.x - 2.0, c.y + 2.0, 4.0, 5.0, color);
    draw_rectangle(c.x - 6.0, c.y + 6.0, 12.0, 3.0, color);
}

fn draw_lock(c: Vec2) {
    let gold = Color::new(1.0, 0.8, 0.2, 1.0);
    draw_circle_lines(c.x, c.y - 8.0, 10.0, 4.0, gold);
    ui::rounded_rect(c.x - 15.0, c.y - 6.0, 30.0, 24.0, 6.0, gold);
    draw_circle(c.x, c.y + 4.0, 3.5, Color::new(0.3, 0.2, 0.05, 1.0));
}

fn draw_speaker(c: Vec2, on: bool) {
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

fn toggle_fullscreen() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static FULLSCREEN: AtomicBool = AtomicBool::new(false);
    let next = !FULLSCREEN.load(Ordering::Relaxed);
    FULLSCREEN.store(next, Ordering::Relaxed);
    set_fullscreen(next);
}
