//! The "scapes": self-contained game styles that share one interface.
//!
//! `Game` owns everything common to all of them (menus, HUD, coins, combos,
//! lives, power-up timers, effects and sound). A scape simulates its own
//! world, reads [`Input`], and reports what happened through [`Event`]s.

use macroquad::prelude::*;

use crate::art::Look;

pub mod gravity;
pub mod orbit;
pub mod rain;
pub mod sea;
pub mod tunnel;

/// Controls for this frame, already merged across keyboard, mouse and touch.
#[derive(Default, Clone, Copy)]
pub struct Input {
    /// Keyboard steering, -1 (left) to 1 (right).
    pub dir: f32,
    /// A finger or mouse button is held for a drag.
    pub dragging: bool,
    /// Horizontal drag movement this frame, in virtual pixels.
    pub drag_dx: f32,
    /// A quick tap was released this frame.
    pub tap: bool,
    /// One-touch action pressed this frame: touch / click down, or Space / Up / W.
    pub action: bool,
    /// One-touch action held: touch / mouse down, or Space / Up / W down.
    pub hold: bool,
    /// Dash request: tap, Space / Shift, right click or a second finger.
    pub dash: bool,
}

/// Run-wide numbers shared by every scape.
pub struct Stats {
    pub time: f32,
    pub score: f32,
    pub coins: u32,
    pub lives: u32,
    pub max_lives: u32,
    pub combo: u32,
    pub combo_timer: f32,
    pub close_calls: u32,
    /// Seconds of post-hit invulnerability left; scapes skip collisions while > 0.
    pub invuln: f32,
    pub shield: bool,
    /// Seconds of double coins left.
    pub coin_x2: f32,
    /// Seconds the character keeps smiling.
    pub happy: f32,
    pub score_mult: f32,
    /// Points per second just for surviving.
    pub survival_rate: f32,
    pub time_limit: Option<f32>,
    /// Short HUD label such as "WAVE 2" or "320 m".
    pub label: String,
}

impl Stats {
    pub fn new(lives: u32, score_mult: f32) -> Self {
        Stats {
            time: 0.0,
            score: 0.0,
            coins: 0,
            lives,
            max_lives: lives,
            combo: 0,
            combo_timer: 0.0,
            close_calls: 0,
            invuln: 0.0,
            shield: false,
            coin_x2: 0.0,
            happy: 0.0,
            score_mult,
            survival_rate: 10.0,
            time_limit: None,
            label: String::new(),
        }
    }

    pub fn combo_mult(&self) -> f32 {
        1.0 + 0.2 * self.combo as f32
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Power {
    Shield,
    CoinX2,
    Slow,
    Magnet,
}

impl Power {
    pub fn color(self) -> Color {
        match self {
            Power::Shield => SKYBLUE,
            Power::CoinX2 => Color::new(1.0, 0.75, 0.1, 1.0),
            Power::Slow => VIOLET,
            Power::Magnet => Color::new(1.0, 0.4, 0.4, 1.0),
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Power::Shield => "S",
            Power::CoinX2 => "x2",
            Power::Slow => "T",
            Power::Magnet => "M",
        }
    }

    /// Random pick between shield and double coins, the power-ups every scape has.
    pub fn common() -> Power {
        if rand::gen_range(0, 2) == 0 {
            Power::Shield
        } else {
            Power::CoinX2
        }
    }
}

pub enum Event {
    Coin(Vec2),
    /// A near miss at this position.
    Close(Vec2),
    /// The player touched something dangerous.
    Hit {
        at: Vec2,
        color: Color,
    },
    Power(Power, Vec2),
    Banner(String, String),
    /// Extra points with a big popup (e.g. "WAVE CLEAR").
    Bonus {
        text: String,
        points: f32,
    },
    Dash(Vec2),
}

pub trait Scape {
    fn update(&mut self, input: &Input, dt: f32, stats: &mut Stats, events: &mut Vec<Event>);
    /// Keeps the world moving (slowly) behind the death beat and results screen.
    fn coast(&mut self, dt: f32);
    fn draw(&self, look: &Look, stats: &Stats, show_player: bool);
    /// Where popups and particles about the player should appear.
    fn player_pos(&self) -> Vec2;
    /// Size of the player's character, for the shield bubble.
    fn player_size(&self) -> f32;
    /// Something is about to hit the player (the character looks scared).
    fn danger(&self) -> bool;
    /// Background gradient (top, bottom).
    fn sky(&self) -> ((f32, f32, f32), (f32, f32, f32));
    fn accent(&self) -> Color;
    /// Speed of the background bubbles.
    fn bg_speed(&self) -> f32;
    /// Control hints shown during the first runs: (touch, keyboard).
    fn hints(&self) -> [(&'static str, &'static str); 2];
}

/// A coin, or a power-up when `power` is set, floating in a scape's world.
pub struct Pickup {
    pub pos: Vec2,
    pub power: Option<Power>,
    pub age: f32,
}

impl Pickup {
    pub fn coin(pos: Vec2) -> Self {
        Pickup {
            pos,
            power: None,
            age: rand::gen_range(0.0, 1.0),
        }
    }

    pub fn power(pos: Vec2, power: Power) -> Self {
        Pickup {
            pos,
            power: Some(power),
            age: 0.0,
        }
    }

    pub fn draw(&self) {
        match self.power {
            None => crate::art::draw_coin(self.pos, self.age, 15.0),
            Some(p) => {
                crate::art::draw_power_bubble(self.pos, self.age, p.color(), p.letter(), 20.0)
            }
        }
    }

    pub fn event(&self) -> Event {
        match self.power {
            None => Event::Coin(self.pos),
            Some(p) => Event::Power(p, self.pos),
        }
    }
}

/// Collects pickups within `reach` of `at`, reporting each as an event.
pub fn collect_pickups(pickups: &mut Vec<Pickup>, at: Vec2, reach: f32, events: &mut Vec<Event>) {
    pickups.retain(|p| {
        let hit = p.pos.distance(at) < reach;
        if hit {
            events.push(p.event());
        }
        !hit
    });
}
