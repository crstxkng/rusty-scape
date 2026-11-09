//! The scapes on the menu, and the variants of Rain Scape (the falling-blocks
//! game). Each Rain variant is a set of tuning knobs consumed by `scape/rain.rs`.

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
                coin_chance: 0.22,
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
                coin_chance: 0.18,
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
                coin_chance: 0.24,
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
                coin_chance: 0.22,
                drifting: true,
                time_limit: None,
                color: (0.75, 0.45, 1.0),
                block: (0.8, 0.4, 1.0),
                sky: ((0.12, 0.05, 0.25), (0.25, 0.1, 0.4)),
            },
        }
    }
}

/// The game styles listed on the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScapeKind {
    Rain,
    Tunnel,
    Orbit,
    Gravity,
    Sea,
}

pub struct ScapeInfo {
    pub name: &'static str,
    pub tagline: &'static str,
    pub color: (f32, f32, f32),
    pub sky: ((f32, f32, f32), (f32, f32, f32)),
}

impl ScapeKind {
    pub const ALL: [ScapeKind; 5] = [
        ScapeKind::Rain,
        ScapeKind::Tunnel,
        ScapeKind::Orbit,
        ScapeKind::Gravity,
        ScapeKind::Sea,
    ];

    pub fn info(self) -> ScapeInfo {
        match self {
            ScapeKind::Rain => ScapeInfo {
                name: "Rain Scape",
                tagline: "Dodge the falling blocks!",
                color: (1.0, 0.55, 0.2),
                sky: ((0.13, 0.12, 0.3), (0.32, 0.18, 0.38)),
            },
            ScapeKind::Tunnel => ScapeInfo {
                name: "Tunnel Scape",
                tagline: "Spin the tunnel, slip through the gaps!",
                color: (0.25, 0.85, 1.0),
                sky: ((0.03, 0.05, 0.15), (0.08, 0.1, 0.28)),
            },
            ScapeKind::Orbit => ScapeInfo {
                name: "Orbit Scape",
                tagline: "Tap to hop between orbits!",
                color: (1.0, 0.45, 0.75),
                sky: ((0.06, 0.03, 0.15), (0.18, 0.06, 0.25)),
            },
            ScapeKind::Gravity => ScapeInfo {
                name: "Gravity Scape",
                tagline: "Tap to flip between floor and ceiling!",
                color: (0.4, 1.0, 0.55),
                sky: ((0.05, 0.16, 0.14), (0.1, 0.3, 0.22)),
            },
            ScapeKind::Sea => ScapeInfo {
                name: "Deep Sea Scape",
                tagline: "Hold to swim up, let go to sink!",
                color: (0.3, 0.75, 1.0),
                sky: ((0.1, 0.45, 0.7), (0.02, 0.12, 0.3)),
            },
        }
    }

    pub fn index(self) -> usize {
        ScapeKind::ALL.iter().position(|k| *k == self).unwrap()
    }
}

/// Number of high-score slots: one per Rain variant plus one per other scape.
pub const SCORE_SLOTS: usize = Mode::ALL.len() + ScapeKind::ALL.len() - 1;

/// High-score slot for a scape (and Rain variant). Rain keeps slots 0-4 so
/// existing high scores survive.
pub fn score_slot(kind: ScapeKind, mode: Mode) -> usize {
    match kind {
        ScapeKind::Rain => mode.index(),
        other => Mode::ALL.len() + other.index() - 1,
    }
}
