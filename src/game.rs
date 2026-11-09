//! The game shell: menu, shared run state (score, coins, combos, lives,
//! power-ups), HUD, pause and results. The actual gameplay of each style
//! lives in `scape/`.

use macroquad::prelude::*;

use crate::art::{self, Look, Mood, ANIMALS};
use crate::audio::{Audio, Sfx};
use crate::fx::{fade, rgb, Background, Banner, FloatingTexts, Particles};
use crate::modes::{self, Mode, ScapeKind, SCORE_SLOTS};
use crate::scape::{self, Event, Input, Power, Scape, Stats};
use crate::storage;
use crate::ui::{self, shade, vh, vw, View};

const HIT_INVULN: f32 = 1.6;
const COMBO_WINDOW: f32 = 2.5;
const MAX_COMBO: u32 = 10;
const COIN_X2_TIME: f32 = 10.0;
/// A touch shorter than this that barely moves counts as a tap.
const TAP_TIME: f32 = 0.22;
const TAP_SLOP: f32 = 16.0;
/// Slow-motion beat between the fatal hit and the results screen.
const DEATH_TIME: f32 = 0.9;
/// Runs of each scape during which its control hints are shown.
const TUTORIAL_RUNS: u32 = 2;
/// Points worth one bonus coin at the end of a run.
const POINTS_PER_BONUS_COIN: f32 = 50.0;
/// Prices of the v0.3 color skins, refunded once now that skins are animals.
const OLD_SKIN_PRICES: [u32; 8] = [0, 40, 80, 150, 250, 400, 600, 1000];

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

/// Tracks a press for drags and taps.
#[derive(Default)]
struct Pointer {
    active: bool,
    last: Vec2,
    down_at: f32,
    travel: f32,
}

/// One play session of one scape.
struct Run {
    kind: ScapeKind,
    mode: Mode,
    scape: Box<dyn Scape>,
    stats: Stats,
    best_before: u32,
    passed_best: bool,
    death_timer: Option<f32>,
    out_of_time: bool,
    over_time: f32,
    bonus_coins: u32,
}

impl Run {
    fn new(kind: ScapeKind, mode: Mode, best_before: u32) -> Self {
        let (scape, stats): (Box<dyn Scape>, Stats) = match kind {
            ScapeKind::Rain => {
                let s = scape::rain::Rain::new(mode);
                let stats = s.stats();
                (Box::new(s), stats)
            }
            ScapeKind::Tunnel => {
                let s = scape::tunnel::Tunnel::new();
                let stats = s.stats();
                (Box::new(s), stats)
            }
            ScapeKind::Orbit => {
                let s = scape::orbit::Orbit::new();
                let stats = s.stats();
                (Box::new(s), stats)
            }
            ScapeKind::Gravity => {
                let s = scape::gravity::Gravity::new();
                let stats = s.stats();
                (Box::new(s), stats)
            }
            ScapeKind::Sea => {
                let s = scape::sea::Sea::new();
                let stats = s.stats();
                (Box::new(s), stats)
            }
        };
        Run {
            kind,
            mode,
            scape,
            stats,
            best_before,
            passed_best: false,
            death_timer: None,
            out_of_time: false,
            over_time: 0.0,
            bonus_coins: 0,
        }
    }

    fn alive(&self) -> bool {
        self.death_timer.is_none()
    }

    fn slot(&self) -> usize {
        modes::score_slot(self.kind, self.mode)
    }

    fn title(&self) -> String {
        match self.kind {
            ScapeKind::Rain => format!("Rain · {}", self.mode.config().name),
            other => other.info().name.to_string(),
        }
    }
}

pub struct Game {
    phase: Phase,
    kind: ScapeKind,
    variant: usize,
    animal: usize,
    browse: usize,
    unlocked: u32,
    wallet: u32,
    runs: [u32; ScapeKind::ALL.len()],
    touch_seen: bool,
    high_scores: [u32; SCORE_SLOTS],
    audio: Audio,
    particles: Particles,
    texts: FloatingTexts,
    banner: Banner,
    background: Background,
    run: Run,
    pointer: Pointer,
    events: Vec<Event>,
    shake: f32,
    flash: f32,
    view: View,
    clock: f32,
    blink_in: f32,
}

impl Game {
    pub async fn new() -> Self {
        let mut high_scores = [0; SCORE_SLOTS];
        for (i, hs) in high_scores.iter_mut().enumerate() {
            *hs = storage::get(storage::high_score_slot(i)).unwrap_or(0.0) as u32;
        }
        let mut wallet = storage::get(storage::SLOT_WALLET).unwrap_or(0.0) as u32;

        // Skins became animals: refund any colors bought in v0.3, once.
        let unlocked = match storage::get(storage::SLOT_ANIMALS) {
            Some(bits) => bits as u32 | 1,
            None => {
                let old = storage::get(storage::SLOT_UNLOCKED).unwrap_or(1.0) as u32;
                let refund: u32 = (1..OLD_SKIN_PRICES.len())
                    .filter(|i| old & (1 << i) != 0)
                    .map(|i| OLD_SKIN_PRICES[i])
                    .sum();
                wallet += refund;
                storage::set(storage::SLOT_WALLET, wallet as f64);
                storage::set(storage::SLOT_ANIMALS, 1.0);
                1
            }
        };
        let mut animal =
            (storage::get(storage::SLOT_ANIMAL).unwrap_or(0.0) as usize).min(ANIMALS.len() - 1);
        if unlocked & (1 << animal) == 0 {
            animal = 0;
        }
        let mut runs = [0; ScapeKind::ALL.len()];
        for (i, r) in runs.iter_mut().enumerate() {
            *r = storage::get(storage::runs_slot(i)).unwrap_or(0.0) as u32;
        }
        let kind = ScapeKind::ALL[(storage::get(storage::SLOT_SCAPE).unwrap_or(0.0) as usize)
            .min(ScapeKind::ALL.len() - 1)];
        let variant =
            (storage::get(storage::SLOT_VARIANT).unwrap_or(0.0) as usize).min(Mode::ALL.len() - 1);
        let muted = storage::get(storage::SLOT_MUTED).unwrap_or(0.0) != 0.0;
        let view = View::compute();
        Game {
            phase: Phase::Menu,
            kind,
            variant,
            animal,
            browse: animal,
            unlocked,
            wallet,
            runs,
            touch_seen: false,
            high_scores,
            audio: Audio::load(muted).await,
            particles: Particles::default(),
            texts: FloatingTexts::default(),
            banner: Banner::default(),
            background: Background::new(),
            run: Run::new(ScapeKind::Rain, Mode::Classic, 0),
            pointer: Pointer::default(),
            events: Vec::new(),
            shake: 0.0,
            flash: 0.0,
            view,
            clock: 0.0,
            blink_in: 2.0,
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
                Phase::Menu => 40.0,
                _ => self.run.scape.bg_speed(),
            };
            self.background.update(dt, bg_speed);
            self.shake = (self.shake - dt * 40.0).max(0.0);
            self.flash = (self.flash - dt * 2.5).max(0.0);
            self.blink_in -= dt;
            if self.blink_in < -0.12 {
                self.blink_in = rand::gen_range(1.5, 4.0);
            }
        }

        clear_background(BLACK);
        // Smooth, rumbly shake instead of per-frame jitter.
        let shake = vec2((self.clock * 53.0).sin(), (self.clock * 41.0).cos()) * self.shake;
        self.view.apply(shake);

        let sky = if self.phase == Phase::Menu {
            if self.kind == ScapeKind::Rain {
                Mode::ALL[self.variant].config().sky
            } else {
                self.kind.info().sky
            }
        } else {
            self.run.scape.sky()
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
        Mode::ALL[self.variant]
    }

    /// Starts the selected scape with the equipped animal (browsing a locked
    /// one doesn't block playing).
    fn start_run(&mut self) {
        self.browse = self.animal;
        let (kind, mode) = (self.kind, self.selected_mode());
        self.run = Run::new(kind, mode, self.high_scores[modes::score_slot(kind, mode)]);
        self.particles.clear();
        self.texts.clear();
        self.banner = Banner::default();
        self.pointer = Pointer::default();
        self.phase = Phase::Playing;
        let i = kind.index();
        self.runs[i] += 1;
        storage::set(storage::runs_slot(i), self.runs[i] as f64);
        storage::set(storage::SLOT_SCAPE, i as f64);
        storage::set(storage::SLOT_VARIANT, self.variant as f64);
        self.audio.play(Sfx::Start);
    }

    fn is_unlocked(&self, animal: usize) -> bool {
        self.unlocked & (1 << animal) != 0
    }

    fn browse_animal(&mut self, delta: i32) {
        let n = ANIMALS.len() as i32;
        self.browse = (self.browse as i32 + delta).rem_euclid(n) as usize;
        if self.is_unlocked(self.browse) {
            self.animal = self.browse;
            storage::set(storage::SLOT_ANIMAL, self.animal as f64);
        }
        self.audio.play(Sfx::Select);
    }

    fn try_buy(&mut self, at: Vec2) {
        let a = self.browse;
        let price = ANIMALS[a].price;
        if self.is_unlocked(a) || self.wallet < price {
            return;
        }
        self.wallet -= price;
        self.unlocked |= 1 << a;
        self.animal = a;
        storage::set(storage::SLOT_WALLET, self.wallet as f64);
        storage::set(storage::SLOT_ANIMALS, self.unlocked as f64);
        storage::set(storage::SLOT_ANIMAL, a as f64);
        self.particles
            .burst(at, 40, rgb(ANIMALS[a].trail, 1.0), 420.0);
        self.particles.burst(at, 20, GOLD, 300.0);
        self.audio.play(Sfx::Unlock);
        storage::vibrate(40);
    }

    /// Cheapest locked animal, to show "next unlock" progress.
    fn next_unlock(&self) -> Option<usize> {
        (0..ANIMALS.len())
            .filter(|i| !self.is_unlocked(*i))
            .min_by_key(|i| ANIMALS[*i].price)
    }

    fn mood(&self) -> Mood {
        if self.blink_in < 0.0 {
            Mood::Blink
        } else if self.run.stats.happy > 0.0 {
            Mood::Happy
        } else if self.phase == Phase::Playing && self.run.scape.danger() {
            Mood::Scared
        } else {
            Mood::Normal
        }
    }

    // ---------------------------------------------------------------- menu

    fn update_menu_keys(&mut self) {
        let n = ScapeKind::ALL.len();
        let i = self.kind.index();
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            self.kind = ScapeKind::ALL[(i + n - 1) % n];
            self.audio.play(Sfx::Select);
        }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            self.kind = ScapeKind::ALL[(i + 1) % n];
            self.audio.play(Sfx::Select);
        }
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            self.browse_animal(-1);
        }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            self.browse_animal(1);
        }
        if is_key_pressed(KeyCode::V) && self.kind == ScapeKind::Rain {
            self.variant = (self.variant + 1) % Mode::ALL.len();
            self.audio.play(Sfx::Select);
        }
        if is_key_pressed(KeyCode::B) {
            self.try_buy(vec2(vw() / 2.0, vh() / 2.0));
        }
        if (is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter))
            && self.is_unlocked(self.browse)
        {
            self.start_run();
        }
    }

    fn draw_menu(&mut self) {
        let (w, h) = (vw(), vh());
        let portrait = ui::is_portrait();
        let compact = portrait && h < 1000.0;
        let info = self.kind.info();
        let accent = rgb(info.color, 1.0);

        // Top bar: sound toggle, (desktop) quit, wallet.
        let sound = Rect::new(14.0, 14.0, 60.0, 52.0);
        if ui::button(&self.view, sound, "", Color::new(1.0, 1.0, 1.0, 0.15)) {
            self.toggle_mute();
        }
        art::speaker(sound.center() + vec2(0.0, 2.0), !self.audio.muted());
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
            (w / 2.0, w / 2.0, ((h - 860.0) * 0.3).max(0.0) + 60.0)
        } else {
            (w * 0.29, w * 0.68, 66.0)
        };

        // Title and the bouncing animal.
        let bob = (self.clock * 2.5).sin() * 5.0;
        let title_size = if compact { 56.0 } else { 66.0 };
        let title_y = top + if portrait { 46.0 } else { 62.0 };
        ui::text_shadow_centered(
            "RUSTY",
            left_cx,
            title_y + bob,
            title_size,
            Color::new(1.0, 0.62, 0.25, 1.0),
        );
        ui::text_shadow_centered(
            "SCAPE",
            left_cx,
            title_y + title_size * 0.88 + bob,
            title_size,
            Color::new(1.0, 0.85, 0.35, 1.0),
        );

        let animal = &ANIMALS[self.browse];
        let pet_size = if compact { 76.0 } else { 92.0 };
        let pet_y = title_y + title_size * 0.88 + 24.0 + pet_size * 0.75;
        let hop = (self.clock * 3.0).sin().abs() * 12.0;
        let mut look = Look::new(self.browse, self.clock);
        look.mood = if self.blink_in < 0.0 {
            Mood::Blink
        } else {
            Mood::Happy
        };
        look.gaze = vec2((self.clock * 0.8).sin(), 0.2);
        draw_ellipse(
            left_cx,
            pet_y + pet_size * 0.5 + 4.0,
            pet_size * 0.45 - hop * 0.8,
            7.0,
            0.0,
            Color::new(0.0, 0.0, 0.0, 0.2),
        );
        art::with_transform(vec2(left_cx, pet_y - hop), 0.0, false, || {
            art::draw_animal(&look, pet_size)
        });
        if !self.is_unlocked(self.browse) {
            art::lock(vec2(
                left_cx + pet_size * 0.55,
                pet_y - pet_size * 0.5 - hop,
            ));
        }

        // Animal picker.
        let picker_y = pet_y + pet_size * 0.5 + 16.0;
        let prev = Rect::new(left_cx - 150.0, picker_y, 56.0, 52.0);
        let next = Rect::new(left_cx + 94.0, picker_y, 56.0, 52.0);
        if ui::button(&self.view, prev, "<", shade(accent, 0.8)) {
            self.browse_animal(-1);
        }
        if ui::button(&self.view, next, ">", shade(accent, 0.8)) {
            self.browse_animal(1);
        }
        ui::text_shadow_centered(animal.name, left_cx, picker_y + 36.0, 30.0, WHITE);

        // Scape list, plus the Rain variant picker.
        let row_w = if portrait {
            (w - 40.0).min(440.0)
        } else {
            (w * 0.5).min(440.0)
        };
        let row_h = if compact { 44.0 } else { 50.0 };
        let list_y = if portrait {
            picker_y + 70.0
        } else {
            top + 10.0
        };
        for (i, kind) in ScapeKind::ALL.iter().enumerate() {
            let kinfo = kind.info();
            let rect = Rect::new(
                right_cx - row_w / 2.0,
                list_y + i as f32 * (row_h + 8.0),
                row_w,
                row_h,
            );
            let selected = *kind == self.kind;
            let color = rgb(kinfo.color, 1.0);
            if ui::clicked_in(&self.view, rect) {
                if selected {
                    self.start_run();
                    return;
                }
                self.kind = *kind;
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
            let text_y = card.y + row_h * 0.5 + 10.0;
            ui::text(kinfo.name, card.x + 20.0, text_y, 27.0, name_color);
            let slot = modes::score_slot(*kind, self.selected_mode());
            let best = format!("{}", self.high_scores[slot]);
            let bw = ui::text_width(&best, 24.0);
            ui::text(&best, card.right() - bw - 18.0, text_y, 24.0, name_color);
            art::trophy(
                vec2(card.right() - bw - 36.0, text_y - 9.0),
                fade(name_color, 0.9),
            );
        }
        let mut below = list_y + ScapeKind::ALL.len() as f32 * (row_h + 8.0);
        if self.kind == ScapeKind::Rain {
            let mode = self.selected_mode().config();
            let chip_w = 300.0f32.min(row_w);
            let prev = Rect::new(right_cx - chip_w / 2.0, below, 48.0, 44.0);
            let next = Rect::new(right_cx + chip_w / 2.0 - 48.0, below, 48.0, 44.0);
            let mcolor = rgb(mode.color, 1.0);
            if ui::button(&self.view, prev, "<", shade(mcolor, 0.8)) {
                self.variant = (self.variant + Mode::ALL.len() - 1) % Mode::ALL.len();
                self.audio.play(Sfx::Select);
            }
            if ui::button(&self.view, next, ">", shade(mcolor, 0.8)) {
                self.variant = (self.variant + 1) % Mode::ALL.len();
                self.audio.play(Sfx::Select);
            }
            ui::text_shadow_centered(mode.name, right_cx, below + 31.0, 28.0, mcolor);
            ui::text_centered(
                mode.tagline,
                right_cx,
                below + 72.0,
                20.0,
                Color::new(1.0, 1.0, 1.0, 0.85),
            );
            below += 82.0;
        } else {
            ui::text_centered(
                info.tagline,
                right_cx,
                below + 24.0,
                22.0,
                Color::new(1.0, 1.0, 1.0, 0.85),
            );
            below += 40.0;
        }

        // Main action: PLAY, or UNLOCK when browsing a locked animal.
        let play_y = if portrait {
            below + 14.0
        } else {
            picker_y + 70.0
        };
        let play = Rect::new(left_cx - 130.0, play_y, 260.0, 72.0);
        if self.is_unlocked(self.browse) {
            let pulse = 1.0 + (self.clock * 4.0).sin() * 0.04;
            let grow = Rect::new(
                play.x - play.w * (pulse - 1.0) / 2.0,
                play.y,
                play.w * pulse,
                play.h,
            );
            if ui::button(&self.view, grow, "PLAY", Color::new(0.25, 0.8, 0.4, 1.0)) {
                self.start_run();
            }
        } else {
            let price = animal.price;
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
                self.try_buy(vec2(left_cx, pet_y));
            }
            if !affordable {
                ui::text_centered(
                    "Collect coins in any scape!",
                    left_cx,
                    play.bottom() + 28.0,
                    20.0,
                    Color::new(1.0, 0.9, 0.5, 0.9),
                );
            }
        }

        if !portrait && !self.touch_seen {
            ui::text_centered(
                "Up/Down: scape   V: variant   Left/Right: animal   Enter: play   M: sound",
                w / 2.0,
                h - 14.0,
                18.0,
                Color::new(1.0, 1.0, 1.0, 0.5),
            );
        }
    }

    // ------------------------------------------------------------- playing

    fn read_input(&mut self) -> Input {
        let m = self.view.mouse();
        let mut input = Input::default();
        let pressed = is_mouse_button_pressed(MouseButton::Left) && !pause_button().contains(m);
        if pressed {
            self.pointer = Pointer {
                active: true,
                last: m,
                down_at: self.clock,
                travel: 0.0,
            };
        }
        if self.pointer.active {
            if is_mouse_button_down(MouseButton::Left) {
                let d = m - self.pointer.last;
                self.pointer.travel += d.length();
                self.pointer.last = m;
                input.dragging = true;
                input.drag_dx = d.x;
            } else {
                input.tap =
                    self.clock - self.pointer.down_at < TAP_TIME && self.pointer.travel < TAP_SLOP;
                self.pointer.active = false;
            }
        }
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            input.dir -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            input.dir += 1.0;
        }
        let action_keys = [KeyCode::Space, KeyCode::Up, KeyCode::W];
        input.action = pressed || action_keys.iter().any(|k| is_key_pressed(*k));
        input.hold = input.dragging || action_keys.iter().any(|k| is_key_down(*k));
        let second_finger = {
            let t = touches();
            t.len() >= 2 && t.iter().any(|t| t.phase == TouchPhase::Started)
        };
        input.dash = input.tap
            || second_finger
            || is_mouse_button_pressed(MouseButton::Right)
            || [
                KeyCode::Space,
                KeyCode::LeftShift,
                KeyCode::RightShift,
                KeyCode::Up,
                KeyCode::W,
            ]
            .iter()
            .any(|k| is_key_pressed(*k));
        input
    }

    fn update_playing(&mut self, dt: f32) {
        if self.run.alive()
            && (is_key_pressed(KeyCode::Escape)
                || is_key_pressed(KeyCode::P)
                || ui::clicked_in(&self.view, pause_button()))
        {
            self.phase = Phase::Paused;
            self.pointer.active = false;
            return;
        }

        // Dramatic slow-motion beat after the fatal hit.
        if let Some(t) = self.run.death_timer {
            let left = t - dt;
            self.run.death_timer = Some(left);
            self.run.scape.coast(dt * 0.3);
            if left <= 0.0 {
                self.end_run();
            }
            return;
        }

        self.run.stats.time += dt;
        if let Some(limit) = self.run.stats.time_limit {
            if self.run.stats.time >= limit {
                self.run.out_of_time = true;
                self.end_run();
                return;
            }
        }

        let input = self.read_input();
        let mut events = std::mem::take(&mut self.events);
        self.run
            .scape
            .update(&input, dt, &mut self.run.stats, &mut events);
        for e in events.drain(..) {
            self.handle(e);
        }
        self.events = events;

        let alive = self.run.alive();
        let s = &mut self.run.stats;
        s.invuln = (s.invuln - dt).max(0.0);
        s.coin_x2 = (s.coin_x2 - dt).max(0.0);
        s.happy = (s.happy - dt).max(0.0);
        s.combo_timer -= dt;
        if s.combo_timer <= 0.0 {
            s.combo = 0;
        }
        if !alive {
            return;
        }
        s.score += s.survival_rate * dt * s.score_mult * s.combo_mult();

        if !self.run.passed_best
            && self.run.best_before > 0
            && self.run.stats.score as u32 > self.run.best_before
        {
            self.run.passed_best = true;
            self.banner
                .show("NEW BEST!", "keep going!", Color::new(0.4, 1.0, 0.5, 1.0));
            self.audio.play(Sfx::NewBest);
            storage::vibrate(30);
        }
    }

    /// Applies one gameplay event: scoring, coins, power-ups, feedback.
    fn handle(&mut self, event: Event) {
        let trail = rgb(ANIMALS[self.animal].trail, 1.0);
        let player = self.run.scape.player_pos();
        let label_at = player - vec2(0.0, 60.0);
        let dead = self.run.death_timer.is_some();
        let s = &mut self.run.stats;
        match event {
            Event::Coin(at) => {
                let n = if s.coin_x2 > 0.0 { 2 } else { 1 };
                s.coins += n;
                s.score += 20.0 * s.score_mult * s.combo_mult() * n as f32;
                s.happy = s.happy.max(0.4);
                self.texts
                    .spawn(format!("+{n}"), at - vec2(0.0, 30.0), GOLD);
                self.particles.burst(at, 8, GOLD, 200.0);
                self.audio.play(Sfx::Coin);
                storage::vibrate(6);
            }
            Event::Close(at) => {
                s.combo = (s.combo + 1).min(MAX_COMBO);
                s.combo_timer = COMBO_WINDOW;
                s.close_calls += 1;
                s.happy = 0.6;
                let bonus = (10.0 * s.score_mult * s.combo as f32).round();
                s.score += bonus;
                self.texts.spawn(format!("CLOSE! +{bonus}"), at, YELLOW);
                self.audio.play(Sfx::NearMiss);
                storage::vibrate(8);
            }
            Event::Hit { at, color } => {
                if s.invuln > 0.0 || dead {
                    return;
                }
                self.particles.burst(at, 22, color, 420.0);
                self.shake = 12.0;
                if s.shield {
                    s.shield = false;
                    s.invuln = 0.8;
                    self.particles.burst(at, 24, SKYBLUE, 320.0);
                    self.texts.spawn("SHIELD SAVED YOU!", label_at, SKYBLUE);
                    self.audio.play(Sfx::Hit);
                    storage::vibrate(50);
                    return;
                }
                s.lives = s.lives.saturating_sub(1);
                s.combo = 0;
                self.flash = 1.0;
                self.audio.play(Sfx::Hit);
                storage::vibrate(120);
                self.particles
                    .burst(at, 40, rgb(ANIMALS[self.animal].body, 1.0), 520.0);
                if s.lives == 0 {
                    self.shake = 18.0;
                    self.run.death_timer = Some(DEATH_TIME);
                    self.pointer.active = false;
                } else {
                    s.invuln = HIT_INVULN;
                    let msg = if s.lives == 1 {
                        "LAST HEART!".to_string()
                    } else {
                        format!("{} HEARTS LEFT", s.lives)
                    };
                    self.texts
                        .spawn(msg, label_at, Color::new(1.0, 0.4, 0.45, 1.0));
                }
            }
            Event::Power(power, at) => {
                s.happy = 0.8;
                let label = match power {
                    Power::Shield => {
                        s.shield = true;
                        "SHIELD!"
                    }
                    Power::CoinX2 => {
                        s.coin_x2 = COIN_X2_TIME;
                        "COINS x2!"
                    }
                    Power::Slow => "SLOW-MO!",
                    Power::Magnet => "MAGNET!",
                };
                self.texts.spawn_sized(label, label_at, power.color(), 32.0);
                self.particles.burst(at, 16, power.color(), 260.0);
                self.audio.play(Sfx::PowerUp);
            }
            Event::Banner(title, subtitle) => {
                self.banner.show(title, subtitle, self.run.scape.accent());
                self.audio.play(Sfx::Wave);
            }
            Event::Bonus { text, points } => {
                s.score += points;
                s.happy = 1.2;
                self.texts.spawn_sized(
                    text,
                    vec2(vw() / 2.0, vh() * 0.42),
                    Color::new(0.5, 1.0, 0.6, 1.0),
                    40.0,
                );
                self.audio.play(Sfx::PowerUp);
            }
            Event::Dash(at) => {
                self.particles.burst(at, 12, fade(trail, 0.9), 260.0);
                self.audio.play(Sfx::Dash);
                storage::vibrate(12);
            }
        }
    }

    fn end_run(&mut self) {
        let score = self.run.stats.score as u32;
        let slot = self.run.slot();
        if score > self.high_scores[slot] {
            self.high_scores[slot] = score;
            storage::set(storage::high_score_slot(slot), score as f64);
        }
        // Every run pays: the coins you grabbed plus a bonus for your score.
        self.run.bonus_coins = (score as f32 / POINTS_PER_BONUS_COIN) as u32;
        self.wallet += self.run.stats.coins + self.run.bonus_coins;
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
        self.run.stats.score as u32 > self.run.best_before
    }

    // ----------------------------------------------------------- game over

    fn update_game_over(&mut self, dt: f32) {
        self.run.over_time += dt;
        // Let the world keep moving behind the results screen.
        self.run.scape.coast(dt * 0.4);
        // Brief input lock so a frantic keypress doesn't skip the results.
        if self.run.over_time < 0.7 {
            return;
        }
        if is_key_pressed(KeyCode::Space)
            || is_key_pressed(KeyCode::Enter)
            || is_key_pressed(KeyCode::R)
        {
            self.start_run();
        } else if is_key_pressed(KeyCode::Escape) {
            self.phase = Phase::Menu;
        }
    }

    // ------------------------------------------------------------- drawing

    fn draw_world(&self) {
        let stats = &self.run.stats;
        let mut look = Look::new(self.animal, self.clock);
        look.mood = self.mood();
        // Blink while invulnerable after a hit.
        if stats.invuln > 0.0 && (stats.invuln * 12.0) as i32 % 2 == 0 {
            look.alpha = 0.35;
        }
        let show_player = self.run.alive() && self.phase != Phase::GameOver;
        self.run.scape.draw(&look, stats, show_player);

        if show_player && stats.shield {
            let p = self.run.scape.player_pos();
            let r = self.run.scape.player_size() * 0.85 + (self.clock * 6.0).sin() * 2.5;
            draw_circle(p.x, p.y, r, fade(SKYBLUE, 0.12));
            draw_circle_lines(p.x, p.y, r, 3.0, fade(SKYBLUE, 0.85));
        }
        self.particles.draw();
        self.texts.draw();
        self.banner.draw();
    }

    fn draw_hud(&self) {
        let run = &self.run;
        let s = &run.stats;
        let w = vw();
        let accent = run.scape.accent();

        // Score, centered and big.
        let score = s.score as u32;
        let pop = if s.combo > 0 {
            1.0 + 0.15 * (s.combo_timer / COMBO_WINDOW).clamp(0.0, 1.0)
        } else {
            1.0
        };
        ui::text_shadow_centered(&format!("{score}"), w / 2.0, 62.0, 54.0 * pop, WHITE);
        if run.best_before > 0 || run.passed_best {
            let (line, color) = if run.passed_best {
                ("NEW BEST!".to_string(), Color::new(0.5, 1.0, 0.6, 1.0))
            } else {
                (
                    format!("BEST {}", run.best_before),
                    Color::new(1.0, 1.0, 1.0, 0.6),
                )
            };
            ui::text_centered(&line, w / 2.0, 88.0, 20.0, color);
        }

        // Combo meter.
        if s.combo > 0 {
            let t = (s.combo_timer / COMBO_WINDOW).clamp(0.0, 1.0);
            ui::text_shadow_centered(
                &format!("x{:.1} COMBO", s.combo_mult()),
                w / 2.0,
                120.0,
                28.0,
                YELLOW,
            );
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

        // Left: label (wave / distance), coins, hearts.
        ui::text(&s.label, 18.0, 40.0, 26.0, accent);
        art::coin_icon(vec2(30.0, 62.0), 11.0);
        let coins = format!("{}", s.coins);
        ui::text(&coins, 48.0, 71.0, 24.0, GOLD);
        if s.coin_x2 > 0.0 {
            let x = 56.0 + ui::text_width(&coins, 24.0);
            let blink = s.coin_x2 > 2.0 || (s.coin_x2 * 6.0) as i32 % 2 == 0;
            if blink {
                ui::rounded_rect(x, 50.0, 44.0, 26.0, 10.0, Color::new(1.0, 0.75, 0.1, 1.0));
                ui::text_centered("x2", x + 22.0, 70.0, 22.0, WHITE);
            }
        }
        if s.max_lives > 1 {
            for i in 0..s.max_lives {
                let c = if i < s.lives {
                    Color::new(1.0, 0.3, 0.4, 1.0)
                } else {
                    Color::new(1.0, 1.0, 1.0, 0.18)
                };
                art::heart(vec2(30.0 + i as f32 * 30.0, 94.0), 24.0, c);
            }
        }

        if let Some(limit) = s.time_limit {
            let left = (limit - s.time).max(0.0);
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

        // Control hints for the first runs of each scape.
        if self.runs[run.kind.index()] <= TUTORIAL_RUNS && s.time < 4.5 {
            let a = (4.5 - s.time).min(1.0);
            let touch = self.touch_seen || ui::is_portrait();
            let [l1, l2] = run.scape.hints();
            let pick = |l: (&'static str, &'static str)| if touch { l.0 } else { l.1 };
            let y = vh() * 0.62;
            ui::text_shadow_centered(pick(l1), w / 2.0, y, 30.0, fade(WHITE, a));
            ui::text_shadow_centered(
                pick(l2),
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
        let accent = self.run.scape.accent();
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
        let s = &run.stats;
        let t = (run.over_time * 3.0).min(1.0);
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5 * t));
        let cx = w / 2.0;
        let accent = run.scape.accent();
        let new_best = self.new_best();

        // Results card.
        let card_w = (w - 32.0).min(460.0);
        let card_h = 430.0;
        let top = ((h - card_h) / 2.0 - 20.0).max(16.0) + (1.0 - t) * 60.0;
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
        ui::text_shadow_centered(title, cx, top + 62.0, 54.0, fade(accent, t));
        ui::text_centered(
            &run.title(),
            cx,
            top + 92.0,
            20.0,
            fade(Color::new(1.0, 1.0, 1.0, 0.6), t),
        );
        ui::text_shadow_centered(
            &format!("{}", s.score as u32),
            cx,
            top + 160.0,
            70.0,
            fade(WHITE, t),
        );
        if new_best {
            let pulse = 0.75 + 0.25 * (self.clock * 8.0).sin();
            ui::text_centered(
                "NEW HIGH SCORE!",
                cx,
                top + 196.0,
                28.0,
                fade(Color::new(0.4, 1.0, 0.5, 1.0), t * pulse),
            );
        } else {
            let best = self.high_scores[run.slot()];
            ui::text_centered(
                &format!("Best {best}"),
                cx,
                top + 196.0,
                26.0,
                fade(Color::new(1.0, 1.0, 1.0, 0.7), t),
            );
        }

        let stats = format!(
            "{}   ·   {:.0}s   ·   {} close call{}",
            s.label,
            s.time,
            s.close_calls,
            if s.close_calls == 1 { "" } else { "s" }
        );
        ui::text_centered(
            &stats,
            cx,
            top + 230.0,
            20.0,
            fade(Color::new(1.0, 1.0, 1.0, 0.75), t),
        );

        // Coins earned and progress toward the next animal.
        let earned = format!("+{} coins   +{} bonus", s.coins, run.bonus_coins);
        let ew = ui::text_width(&earned, 24.0);
        art::coin_icon(vec2(cx - ew / 2.0 - 18.0, top + 262.0), 13.0);
        ui::text(&earned, cx - ew / 2.0, top + 271.0, 24.0, fade(GOLD, t));
        if let Some(next) = self.next_unlock() {
            let price = ANIMALS[next].price.max(1);
            let p = (self.wallet as f32 / price as f32).min(1.0);
            let bw = card_w - 80.0;
            let bx = cx - bw / 2.0;
            ui::rounded_rect(
                bx,
                top + 290.0,
                bw,
                12.0,
                6.0,
                Color::new(0.0, 0.0, 0.0, 0.35 * t),
            );
            ui::rounded_rect(
                bx,
                top + 290.0,
                (bw * p).max(12.0),
                12.0,
                6.0,
                fade(GOLD, t),
            );
            let label = if p >= 1.0 {
                format!("{} is ready to unlock!", ANIMALS[next].name)
            } else {
                format!(
                    "{} more coins for {}",
                    price - self.wallet.min(price),
                    ANIMALS[next].name
                )
            };
            ui::text_centered(
                &label,
                cx,
                top + 324.0,
                20.0,
                fade(Color::new(1.0, 0.9, 0.6, 1.0), t),
            );
        } else {
            ui::text_centered(
                "You've unlocked every animal!",
                cx,
                top + 310.0,
                20.0,
                fade(GOLD, t),
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
            self.start_run();
        } else if ui::button(&self.view, menu, "Menu", shade(accent, 0.8)) {
            self.phase = Phase::Menu;
        }
    }
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
    art::coin_icon(vec2(x + 26.0, right_top.y - 8.0), 14.0);
    ui::text(&label, x + 46.0, right_top.y + 3.0, 30.0, GOLD);
}

fn toggle_fullscreen() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static FULLSCREEN: AtomicBool = AtomicBool::new(false);
    let next = !FULLSCREEN.load(Ordering::Relaxed);
    FULLSCREEN.store(next, Ordering::Relaxed);
    set_fullscreen(next);
}
