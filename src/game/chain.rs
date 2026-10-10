//! One rolling chain of balls on one curve (the original keeps one
//! `CurveMgr` per curve).
//!
//! Balls are stored tail first (lowest path position first).  Positions are
//! path indices: consecutive path points are about one pixel apart, so a
//! ball occupies [`BALL_D`] positions.

use super::{BALL_D, Power};
use crate::data::curve::CurvePath;
use crate::data::levels::CurveSettings;
use bevy::math::Vec2;
use rand::Rng;
use std::sync::Arc;

/// Two balls closer than this (centre to centre) are touching.
pub const TOUCH: f32 = BALL_D + 0.5;
/// Ticks for an inserted ball to slide from the impact point into the chain.
pub const INSERT_TICKS: f32 = 12.0;

#[derive(Clone, Debug)]
pub struct Ball {
    pub id: u32,
    pub color: u8,
    pub pos: f32,
    pub power: Option<Power>,
    pub power_ticks: i32,
    /// Insertion animation: impact position and progress (0..1).
    pub insert: Option<(Vec2, f32)>,
    /// Speed of a segment rolling back towards a matching colour (<= 0).
    pub back_vel: f32,
    /// Gap bonus earned by the shot that inserted this ball.
    pub gap_bonus: i32,
    pub gap_count: i32,
    /// Combo level of a chain reaction started when this ball's segment merges.
    pub combo: i32,
    /// Shot ball that should explode as part of the shot's match check.
    pub from_shot: bool,
}

impl Ball {
    pub fn new(id: u32, color: u8, pos: f32) -> Self {
        Self {
            id,
            color,
            pos,
            power: None,
            power_ticks: 0,
            insert: None,
            back_vel: 0.0,
            gap_bonus: 0,
            gap_count: 0,
            combo: 0,
            from_shot: false,
        }
    }

    pub fn inserting(&self) -> bool {
        self.insert.is_some()
    }
}

pub struct Chain {
    pub path: Arc<CurvePath>,
    pub cfg: CurveSettings,
    pub balls: Vec<Ball>,
    pub colors: u8,
    pub spawned: i32,
    pub roll_in: bool,
    pub lost: bool,
    last_color: Option<u8>,
    run_len: i32,
}

/// Result of one movement tick.
#[derive(Default)]
pub struct TickResult {
    /// Indices of balls whose segment just collided with the one behind
    /// after rolling back (a chain reaction may follow).
    pub merged: Vec<usize>,
    pub spawned: bool,
}

impl Chain {
    pub fn new(path: Arc<CurvePath>, cfg: CurveSettings, colors: u8) -> Self {
        Self {
            path,
            cfg,
            balls: Vec::new(),
            colors: colors.clamp(2, 6),
            spawned: 0,
            roll_in: true,
            lost: false,
            last_color: None,
            run_len: 0,
        }
    }

    pub fn len(&self) -> f32 {
        self.path.end()
    }

    pub fn head_pos(&self) -> Option<f32> {
        self.balls.last().map(|b| b.pos)
    }

    /// Progress of the head along the path, in percent.
    pub fn progress(&self) -> f32 {
        self.head_pos().map(|p| p.max(0.0) * 100.0 / self.len().max(1.0)).unwrap_or(0.0)
    }

    pub fn screen_pos(&self, pos: f32) -> Vec2 {
        let (x, y) = self.path.pos(pos);
        Vec2::new(x, y)
    }

    /// Drawn position of a ball (accounts for the insertion slide).
    pub fn ball_draw_pos(&self, b: &Ball) -> Vec2 {
        let target = self.screen_pos(b.pos);
        match b.insert {
            Some((from, t)) => from.lerp(target, t),
            None => target,
        }
    }

    pub fn visible(&self, b: &Ball) -> bool {
        b.pos >= 0.0 && !self.path.point(b.pos).in_tunnel
    }

    pub fn touching(&self, i: usize) -> bool {
        i + 1 < self.balls.len() && self.balls[i + 1].pos - self.balls[i].pos <= TOUCH
    }

    /// Index range [lo, hi] of the touching segment containing `i`.
    pub fn segment(&self, i: usize) -> (usize, usize) {
        let mut lo = i;
        while lo > 0 && self.touching(lo - 1) {
            lo -= 1;
        }
        let mut hi = i;
        while self.touching(hi) {
            hi += 1;
        }
        (lo, hi)
    }

    /// Same-colour touching run around `i` that would explode (3+ balls).
    pub fn find_match(&self, i: usize) -> Option<(usize, usize)> {
        let b = self.balls.get(i)?;
        if b.inserting() {
            return None;
        }
        let c = b.color;
        let mut lo = i;
        while lo > 0 && self.touching(lo - 1) && self.balls[lo - 1].color == c && !self.balls[lo - 1].inserting() {
            lo -= 1;
        }
        let mut hi = i;
        while self.touching(hi) && self.balls[hi + 1].color == c && !self.balls[hi + 1].inserting() {
            hi += 1;
        }
        (hi - lo + 1 >= 3).then_some((lo, hi))
    }

    pub fn next_color(&mut self, rng: &mut impl Rng) -> u8 {
        let n = self.colors;
        let c = match self.last_color {
            None => rng.random_range(0..n),
            Some(last) => {
                let roll = rng.random_range(0..100);
                if (self.run_len == 1 && rng.random_range(0..100) >= self.cfg.single) || roll < self.cfg.repeat {
                    last
                } else {
                    let mut c = rng.random_range(0..n - 1);
                    if c >= last {
                        c += 1;
                    }
                    c
                }
            }
        };
        if Some(c) == self.last_color {
            self.run_len += 1;
        } else {
            self.run_len = 1;
        }
        self.last_color = Some(c);
        c
    }

    /// Pushes balls forward so that none overlaps the previous one, starting at `from`.
    fn propagate(&mut self, from: usize, merged: &mut Vec<usize>) {
        for i in from.max(1)..self.balls.len() {
            let min = self.balls[i - 1].pos + BALL_D;
            if self.balls[i].pos < min {
                if self.balls[i].back_vel < 0.0 {
                    // A segment rolling back hit the pushed one.
                    let (_, hi) = self.segment(i);
                    for b in &mut self.balls[i..=hi] {
                        b.back_vel = 0.0;
                    }
                    merged.push(i);
                }
                self.balls[i].pos = min;
            } else {
                break;
            }
        }
    }

    /// Inserts a shot ball at index `idx`; returns its index.
    pub fn insert(&mut self, mut ball: Ball, idx: usize, from: Vec2) -> usize {
        let idx = idx.min(self.balls.len());
        ball.insert = Some((from, 0.0));
        ball.back_vel = 0.0;
        self.balls.insert(idx, ball);
        let mut merged = Vec::new();
        self.propagate(idx + 1, &mut merged);
        idx
    }

    pub fn spawn_allowed(&self) -> bool {
        !self.lost && (self.cfg.balls <= 0 || self.spawned < self.cfg.balls)
    }

    /// Advances the chain one tick with pusher speed `v` (negative = backwards).
    pub fn tick(&mut self, v: f32, can_spawn: bool, next_id: &mut u32, rng: &mut impl Rng) -> TickResult {
        let mut res = TickResult::default();

        // Insertion animations.
        for b in &mut self.balls {
            if let Some((from, t)) = b.insert {
                let t = t + 1.0 / INSERT_TICKS;
                b.insert = if t >= 1.0 { None } else { Some((from, t)) };
            }
            if b.power.is_some() {
                b.power_ticks -= 1;
                if b.power_ticks <= 0 {
                    b.power = None;
                }
            }
        }

        // New balls enter at the start of the path.
        if can_spawn && v > 0.0 && self.spawn_allowed() {
            let need = self.balls.first().map(|b| b.pos >= 0.0).unwrap_or(true);
            if need {
                let pos = self.balls.first().map(|b| b.pos - BALL_D).unwrap_or(-BALL_D);
                let color = self.next_color(rng);
                *next_id += 1;
                self.balls.insert(0, Ball::new(*next_id, color, pos));
                self.spawned += 1;
                res.spawned = true;
            }
        }
        if self.balls.is_empty() {
            return res;
        }

        // Pusher: the tail segment moves at v.
        if v >= 0.0 {
            self.balls[0].pos += v;
            self.propagate(1, &mut res.merged);
        } else {
            let (_, hi) = self.segment(0);
            for b in &mut self.balls[..=hi] {
                b.pos += v;
            }
        }

        // Segments roll back towards a matching colour behind them.
        let mut i = 0;
        while i + 1 < self.balls.len() {
            if self.touching(i) {
                i += 1;
                continue;
            }
            let (_, hi) = self.segment(i + 1);
            let attract = self.balls[i].color == self.balls[i + 1].color && !self.balls[i].inserting() && !self.balls[i + 1].inserting();
            if attract {
                let vel = (self.balls[i + 1].back_vel - self.cfg.merge_speed).max(-self.cfg.max_speed.min(12.0));
                for b in &mut self.balls[i + 1..=hi] {
                    b.back_vel = vel;
                    b.pos += vel;
                }
                let min = self.balls[i].pos + BALL_D;
                if self.balls[i + 1].pos <= min {
                    let d = min - self.balls[i + 1].pos;
                    for b in &mut self.balls[i + 1..=hi] {
                        b.pos += d;
                        b.back_vel = 0.0;
                    }
                    res.merged.push(i + 1);
                }
            } else {
                for b in &mut self.balls[i + 1..=hi] {
                    b.back_vel = 0.0;
                }
            }
            i = hi;
        }
        res
    }

    /// Removes balls [lo, hi] and returns them.
    pub fn remove(&mut self, lo: usize, hi: usize) -> Vec<Ball> {
        self.balls.drain(lo..=hi).collect()
    }
}
