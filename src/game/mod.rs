//! Gameplay (the original `Board`, `CurveMgr`, `Ball`, `Bullet` and `Gun`).

pub mod board;
pub mod chain;
pub mod draw;
pub mod gun;

pub use board::{Board, BoardState};

/// Fixed simulation rate of the original (`SexyAppBase` updates at 100 Hz).
pub const TICKS_PER_SEC: f32 = 100.0;

/// Ball diameter, in pixels and in path points.
pub const BALL_D: f32 = 32.0;

/// Ball colours in resource order (blue, yellow, red, green, purple, white).
pub const COLOR_NAMES: [&str; 6] = ["BLUE", "YELLOW", "RED", "GREEN", "PURPLE", "WHITE"];

/// Text/effect colour for each ball colour.
pub const BALL_RGB: [(u8, u8, u8); 6] =
    [(0x40, 0x80, 0xff), (0xff, 0xff, 0x00), (0xff, 0x40, 0x40), (0x40, 0xff, 0x40), (0xff, 0x40, 0xff), (0xff, 0xff, 0xff)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Bomb,
    Slow,
    Accuracy,
    Backwards,
}

impl Power {
    pub fn image_suffix(self) -> &'static str {
        match self {
            Power::Bomb => "BOMB",
            Power::Slow => "SLOW",
            Power::Accuracy => "ACCURACY",
            Power::Backwards => "BACKWARDS",
        }
    }
}

/// Sounds requested by the simulation; played by the Bevy audio system.
#[derive(Clone, Debug)]
pub struct SoundReq {
    pub id: &'static str,
    /// Pitch shift in semitones.
    pub pitch: f32,
    pub volume: f32,
}

impl SoundReq {
    pub fn new(id: &'static str) -> Self {
        Self { id, pitch: 0.0, volume: 1.0 }
    }
}
