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
    /// Seconds a block needs to fall the full screen height at the start.
    pub fall_time: f32,
    /// Seconds between rain spawns at the start (on a 960px-wide screen).
    pub spawn_interval: f32,
    /// How fast difficulty ramps; 1.0 is the Classic pace.
    pub ramp: f32,
    /// Seconds of rain between pattern waves.
    pub wave_every: f32,
    /// Multiplier applied to all score gains.
    pub score_mult: f32,
    /// Chance per rain spawn that a power-up (shield / slow-mo / magnet) appears.
    pub powerup_chance: f32,
    /// Chance per rain spawn that a coin appears.
    pub coin_chance: f32,
    /// Blocks drift sideways and bounce off walls.
    pub drifting: bool,
    /// Run length in seconds, if the mode is timed.
    pub time_limit: Option<f32>,
    /// Menu / HUD accent.
    pub color: (f32, f32, f32),
    /// Base color of falling blocks (shifts toward red as waves climb).
    pub block: (f32, f32, f32),
    /// Background gradient (top, bottom).
    pub sky: ((f32, f32, f32), (f32, f32, f32)),
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
                tagline: "One hit and you're scrap metal!",
                lives: 1,
                fall_time: 2.6,
                spawn_interval: 0.62,
                ramp: 1.0,
                wave_every: 16.0,
                score_mult: 1.0,
                powerup_chance: 0.05,
                coin_chance: 0.14,
                drifting: false,
                time_limit: None,
                color: (1.0, 0.55, 0.2),
                block: (0.45, 0.42, 0.95),
                sky: ((0.13, 0.12, 0.3), (0.32, 0.18, 0.38)),
            },
            Mode::Hardcore => ModeConfig {
                name: "Hardcore",
                tagline: "Faster, meaner, no power-ups. 2x score.",
                lives: 1,
                fall_time: 1.9,
                spawn_interval: 0.42,
                ramp: 1.5,
                wave_every: 11.0,
                score_mult: 2.0,
                powerup_chance: 0.0,
                coin_chance: 0.1,
                drifting: false,
                time_limit: None,
                color: (1.0, 0.3, 0.35),
                block: (0.95, 0.3, 0.4),
                sky: ((0.18, 0.05, 0.1), (0.38, 0.1, 0.12)),
            },
            Mode::Zen => ModeConfig {
                name: "Zen",
                tagline: "Three hearts, gentle pace. Just chill.",
                lives: 3,
                fall_time: 3.3,
                spawn_interval: 0.85,
                ramp: 0.55,
                wave_every: 22.0,
                score_mult: 0.5,
                powerup_chance: 0.07,
                coin_chance: 0.16,
                drifting: false,
                time_limit: None,
                color: (0.3, 0.85, 0.7),
                block: (0.35, 0.7, 0.95),
                sky: ((0.06, 0.2, 0.25), (0.15, 0.35, 0.32)),
            },
            Mode::GoldRush => ModeConfig {
                name: "Gold Rush",
                tagline: "60 seconds. Grab every coin you can!",
                lives: 1,
                fall_time: 2.5,
                spawn_interval: 0.7,
                ramp: 1.2,
                wave_every: 14.0,
                score_mult: 1.0,
                powerup_chance: 0.05,
                coin_chance: 0.7,
                drifting: false,
                time_limit: Some(60.0),
                color: (1.0, 0.8, 0.2),
                block: (0.45, 0.55, 0.8),
                sky: ((0.2, 0.13, 0.05), (0.4, 0.26, 0.1)),
            },
            Mode::Chaos => ModeConfig {
                name: "Chaos",
                tagline: "Wild blocks that drift and bounce. 1.5x score.",
                lives: 1,
                fall_time: 2.8,
                spawn_interval: 0.66,
                ramp: 1.1,
                wave_every: 15.0,
                score_mult: 1.5,
                powerup_chance: 0.06,
                coin_chance: 0.14,
                drifting: true,
                time_limit: None,
                color: (0.75, 0.45, 1.0),
                block: (0.8, 0.4, 1.0),
                sky: ((0.12, 0.05, 0.25), (0.25, 0.1, 0.4)),
            },
        }
    }
}

/// Cosmetic player variants, unlocked with coins collected across runs.
pub struct Skin {
    pub name: &'static str,
    pub body: (f32, f32, f32),
    pub trail: (f32, f32, f32),
    pub price: u32,
}

pub const SKINS: [Skin; 8] = [
    Skin {
        name: "Rusty",
        body: (0.93, 0.5, 0.22),
        trail: (1.0, 0.7, 0.3),
        price: 0,
    },
    Skin {
        name: "Minty",
        body: (0.3, 0.88, 0.62),
        trail: (0.4, 1.0, 0.9),
        price: 40,
    },
    Skin {
        name: "Bubble",
        body: (1.0, 0.5, 0.7),
        trail: (1.0, 0.75, 0.9),
        price: 80,
    },
    Skin {
        name: "Sky",
        body: (0.35, 0.65, 1.0),
        trail: (0.6, 0.85, 1.0),
        price: 150,
    },
    Skin {
        name: "Lemon",
        body: (1.0, 0.86, 0.25),
        trail: (1.0, 1.0, 0.6),
        price: 250,
    },
    Skin {
        name: "Grape",
        body: (0.62, 0.4, 0.95),
        trail: (0.9, 0.6, 1.0),
        price: 400,
    },
    Skin {
        name: "Snow",
        body: (0.93, 0.95, 1.0),
        trail: (0.7, 0.85, 1.0),
        price: 600,
    },
    Skin {
        name: "Midnight",
        body: (0.2, 0.22, 0.35),
        trail: (0.4, 1.0, 0.9),
        price: 1000,
    },
];
