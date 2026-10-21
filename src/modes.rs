//! Game variants. Each mode is a set of tuning knobs consumed by `game.rs`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Classic,
    Hardcore,
    Zen,
    GoldRush,
    Chaos,
}

pub struct ModeConfig {
    pub name: &'static str,
    pub tagline: &'static str,
    /// Hits you can take before the run ends.
    pub lives: u32,
    /// Base falling speed (px/s) at level 1.
    pub fall_speed: f32,
    /// Seconds between obstacle spawns at level 1.
    pub spawn_interval: f32,
    /// Seconds of survival needed to gain a level.
    pub seconds_per_level: f32,
    /// Multiplier applied to all score gains.
    pub score_mult: f32,
    /// Chance per spawn tick that a pickup (shield / slow-mo) appears.
    pub powerup_chance: f32,
    /// Chance per spawn tick that a coin appears.
    pub coin_chance: f32,
    /// Obstacles drift sideways and bounce off walls.
    pub drifting: bool,
    /// Run length in seconds, if the mode is timed.
    pub time_limit: Option<f32>,
    /// Menu / HUD accent.
    pub color: (f32, f32, f32),
    /// Base color of falling blocks (shifts toward red as levels climb).
    pub block: (f32, f32, f32),
}

impl Mode {
    pub const ALL: [Mode; 5] = [
        Mode::Classic,
        Mode::Hardcore,
        Mode::Zen,
        Mode::GoldRush,
        Mode::Chaos,
    ];

    pub fn index(self) -> usize {
        Mode::ALL.iter().position(|m| *m == self).unwrap()
    }

    pub fn config(self) -> ModeConfig {
        match self {
            Mode::Classic => ModeConfig {
                name: "Classic",
                tagline: "The original. One hit and you're scrap metal.",
                lives: 1,
                fall_speed: 230.0,
                spawn_interval: 0.55,
                seconds_per_level: 15.0,
                score_mult: 1.0,
                powerup_chance: 0.04,
                coin_chance: 0.08,
                drifting: false,
                time_limit: None,
                color: (1.0, 0.55, 0.15),
                block: (0.48, 0.39, 0.93),
            },
            Mode::Hardcore => ModeConfig {
                name: "Hardcore",
                tagline: "Faster, denser, no power-ups. Double score.",
                lives: 1,
                fall_speed: 330.0,
                spawn_interval: 0.36,
                seconds_per_level: 10.0,
                score_mult: 2.0,
                powerup_chance: 0.0,
                coin_chance: 0.05,
                drifting: false,
                time_limit: None,
                color: (0.95, 0.2, 0.25),
                block: (0.95, 0.3, 0.35),
            },
            Mode::Zen => ModeConfig {
                name: "Zen",
                tagline: "Three lives, gentle pace. Just relax and dodge.",
                lives: 3,
                fall_speed: 170.0,
                spawn_interval: 0.75,
                seconds_per_level: 30.0,
                score_mult: 0.5,
                powerup_chance: 0.06,
                coin_chance: 0.1,
                drifting: false,
                time_limit: None,
                color: (0.35, 0.85, 0.7),
                block: (0.35, 0.75, 0.9),
            },
            Mode::GoldRush => ModeConfig {
                name: "Gold Rush",
                tagline: "60 seconds. Grab as many coins as you can!",
                lives: 1,
                fall_speed: 240.0,
                spawn_interval: 0.6,
                seconds_per_level: 15.0,
                score_mult: 1.0,
                powerup_chance: 0.03,
                coin_chance: 0.55,
                drifting: false,
                time_limit: Some(60.0),
                color: (1.0, 0.85, 0.2),
                block: (0.45, 0.55, 0.75),
            },
            Mode::Chaos => ModeConfig {
                name: "Chaos",
                tagline: "Blocks of every size drift and bounce. 1.5x score.",
                lives: 1,
                fall_speed: 210.0,
                spawn_interval: 0.6,
                seconds_per_level: 15.0,
                score_mult: 1.5,
                powerup_chance: 0.05,
                coin_chance: 0.08,
                drifting: true,
                time_limit: None,
                color: (0.75, 0.4, 1.0),
                block: (0.75, 0.4, 1.0),
            },
        }
    }
}

/// Cosmetic player variants.
pub struct Skin {
    pub name: &'static str,
    pub body: (f32, f32, f32),
    pub trail: (f32, f32, f32),
}

pub const SKINS: [Skin; 6] = [
    Skin {
        name: "Rust",
        body: (0.87, 0.45, 0.2),
        trail: (1.0, 0.6, 0.2),
    },
    Skin {
        name: "Chrome",
        body: (0.85, 0.88, 0.95),
        trail: (0.6, 0.7, 1.0),
    },
    Skin {
        name: "Neon",
        body: (0.2, 1.0, 0.55),
        trail: (0.1, 1.0, 0.9),
    },
    Skin {
        name: "Ember",
        body: (1.0, 0.25, 0.3),
        trail: (1.0, 0.8, 0.1),
    },
    Skin {
        name: "Violet",
        body: (0.7, 0.45, 1.0),
        trail: (1.0, 0.4, 0.9),
    },
    Skin {
        name: "Gold",
        body: (1.0, 0.82, 0.25),
        trail: (1.0, 1.0, 0.6),
    },
];
