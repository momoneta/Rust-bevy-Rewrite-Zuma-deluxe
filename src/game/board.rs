//! The playing field (`Sexy::Board`): chains, frog, bullets, scoring.
//!
//! Scoring follows the original `FUN_004e8140`/`FUN_004e8540`:
//! * a group of `n` balls in combo step `c` scores `10*n + 100*c + gap bonus`;
//! * five or more consecutive scoring shots add `(chain + 5) * 10` (CHAIN BONUS);
//! * shooting through gaps gives `((500 * (300 - g)) / 3000) * 10` (min 10)
//!   where `g` is the gap width minus two balls, times the number of gaps;
//! * a coin gives `max(500, (points still needed / 600) * 100)`.

use super::chain::{Ball, Chain};
use super::gun::Gun;
use super::{BALL_D, BALL_RGB, Power, SoundReq};
use crate::data::curve::CurvePath;
use crate::data::levels::LevelDesc;
use bevy::math::Vec2;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::sync::Arc;

pub const EXTRA_LIFE_EVERY: i32 = 50_000;
const COIN_TICKS: i32 = 1000;
const POWER_TICKS: i32 = 1600;
const SLOW_TICKS: i32 = 800;
const BACK_TICKS: i32 = 300;
const ACCURACY_TICKS: i32 = 1000;
const BOMB_RADIUS: f32 = 60.0;

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: Vec2,
    pub vel: Vec2,
    pub color: u8,
    pub angle: f32,
    min_gap: Option<f32>,
    gaps: Vec<(usize, u32, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectKind {
    /// Ball pop, tinted with the ball colour.
    Pop(u8),
    /// Bomb explosion.
    Blast,
    Sparkle,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub kind: EffectKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub age: i32,
    pub life: i32,
    pub variant: usize,
}

#[derive(Clone, Debug)]
pub struct FloatText {
    pub text: String,
    pub pos: Vec2,
    pub color: (u8, u8, u8),
    pub age: i32,
    pub delay: i32,
}

#[derive(Clone, Debug)]
pub struct Coin {
    pub pos: Vec2,
    pub age: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardState {
    Intro(i32),
    Playing,
    /// Level cleared: the light runs along the track.
    Won(i32),
    /// The chain reached the hole: balls are sucked in.
    Lost(i32),
    Done(Outcome),
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub max_combo: i32,
    pub max_chain: i32,
    pub gaps: i32,
    pub coins: i32,
    pub balls: i32,
    pub play_ticks: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BoardInput {
    pub aim: Vec2,
    pub fire: bool,
    pub swap: bool,
}

pub struct Board {
    pub level: LevelDesc,
    pub title: String,
    pub chains: Vec<Chain>,
    pub gun: Gun,
    pub bullets: Vec<Bullet>,
    pub effects: Vec<Effect>,
    pub texts: Vec<FloatText>,
    pub coin: Option<Coin>,
    pub score: i32,
    pub level_score: i32,
    pub target: i32,
    pub bar_full: bool,
    pub state: BoardState,
    pub ticks: i32,
    pub slow_ticks: i32,
    pub back_ticks: i32,
    pub acc_ticks: i32,
    pub danger: bool,
    pub chain_count: i32,
    pub stats: Stats,
    pub extra_lives: i32,
    pub sounds: Vec<SoundReq>,
    /// Position of the end-of-level light on each curve.
    pub trail: f32,
    /// Skull cover animation frame (0 = closed .. 11 = open) per curve.
    pub hole_open: Vec<f32>,
    zuma_back_left: f32,
    zuma_slow_ticks: i32,
    next_life: i32,
    last_coin: i32,
    next_id: u32,
    rng: SmallRng,
}

impl Board {
    pub fn new(level: LevelDesc, paths: Vec<Arc<CurvePath>>, colors: u8, score: i32, title: String) -> Self {
        let s = &level.settings;
        let chains = paths
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let cfg = s.curves[i.min(2)].clone();
                let n = if cfg.colors_set { cfg.colors as u8 } else { colors };
                Chain::new(p, cfg, n)
            })
            .collect::<Vec<_>>();
        let gun = Gun::new(Vec2::new(level.graphics.gun.0 as f32, level.graphics.gun.1 as f32), s.reload_delay);
        let target = s.curves[0].score.max(1);
        Self {
            title,
            chains,
            gun,
            bullets: Vec::new(),
            effects: Vec::new(),
            texts: Vec::new(),
            coin: None,
            score,
            level_score: 0,
            target,
            bar_full: false,
            state: BoardState::Intro(0),
            ticks: 0,
            slow_ticks: 0,
            back_ticks: 0,
            acc_ticks: 0,
            danger: false,
            chain_count: 0,
            stats: Stats::default(),
            extra_lives: 0,
            sounds: Vec::new(),
            trail: 0.0,
            hole_open: Vec::new(),
            zuma_back_left: 0.0,
            zuma_slow_ticks: 0,
            next_life: (score / EXTRA_LIFE_EVERY + 1) * EXTRA_LIFE_EVERY,
            last_coin: 0,
            next_id: 0,
            rng: SmallRng::from_os_rng(),
            level,
        }
    }

    fn sound(&mut self, id: &'static str) {
        self.sounds.push(SoundReq::new(id));
    }

    /// Fill ratio of the Zuma gauge (0..1).
    pub fn bar_progress(&self) -> f32 {
        (self.level_score as f32 / self.target as f32).clamp(0.0, 1.0)
    }

    pub fn seconds(&self) -> i32 {
        self.stats.play_ticks / 100
    }

    /// Colours still present on the board.
    fn colors_on_board(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        for c in &self.chains {
            for b in &c.balls {
                if !v.contains(&b.color) {
                    v.push(b.color);
                }
            }
        }
        v
    }

    fn random_gun_color(&mut self) -> Option<u8> {
        let present = self.colors_on_board();
        if present.is_empty() {
            if self.bar_full {
                return None;
            }
            let n = self.chains.first().map(|c| c.colors).unwrap_or(4);
            return Some(self.rng.random_range(0..n));
        }
        Some(present[self.rng.random_range(0..present.len())])
    }

    pub fn update(&mut self, input: &BoardInput) {
        self.ticks += 1;
        self.update_effects();
        self.update_holes();
        match self.state {
            BoardState::Intro(t) => {
                self.gun.aim_at(input.aim);
                if t == 0 {
                    self.sound("SOUND_FROGLAND");
                }
                if t >= 60 {
                    self.state = BoardState::Playing;
                    self.sound("SOUND_ROLLING");
                } else {
                    self.state = BoardState::Intro(t + 1);
                }
            }
            BoardState::Playing => self.update_playing(input),
            BoardState::Won(t) => {
                self.gun.aim_at(input.aim);
                self.update_bullets();
                let max_len = self.chains.iter().map(|c| c.len()).fold(0.0, f32::max);
                self.trail += 12.0;
                if t == 0 {
                    self.sound("SOUND_TRAIL_LIGHT");
                }
                if t % 6 == 0 {
                    for ci in 0..self.chains.len() {
                        if self.trail < self.chains[ci].len() {
                            let p = self.chains[ci].screen_pos(self.trail);
                            self.sparkles(p, 2);
                        }
                    }
                }
                if self.trail >= max_len && t < 10_000 {
                    self.sound("SOUND_TRAIL_LIGHT_END");
                    self.state = BoardState::Won(10_000);
                } else if t >= 10_060 {
                    self.state = BoardState::Done(Outcome::Won);
                } else {
                    self.state = BoardState::Won(t + 1);
                }
            }
            BoardState::Lost(t) => {
                let mut any = false;
                for c in &mut self.chains {
                    let end = c.len();
                    for b in &mut c.balls {
                        b.pos += 6.0 + t as f32 * 0.05;
                    }
                    c.balls.retain(|b| b.pos < end);
                    any |= !c.balls.is_empty();
                }
                if (!any && t > 60) || t > 600 {
                    self.state = BoardState::Done(Outcome::Lost);
                } else {
                    self.state = BoardState::Lost(t + 1);
                }
            }
            BoardState::Done(_) => {}
        }
    }

    fn update_playing(&mut self, input: &BoardInput) {
        self.stats.play_ticks += 1;
        self.slow_ticks = (self.slow_ticks - 1).max(0);
        self.back_ticks = (self.back_ticks - 1).max(0);
        self.acc_ticks = (self.acc_ticks - 1).max(0);
        self.zuma_slow_ticks = (self.zuma_slow_ticks - 1).max(0);

        // Frog.
        self.gun.aim_at(input.aim);
        if input.swap {
            self.gun.swap();
            if self.gun.state == super::gun::GunState::Ready {
                self.sound("SOUND_BUTTON2");
            }
        }
        if input.fire {
            self.fire();
        }
        let mut gun = std::mem::replace(&mut self.gun, Gun::new(Vec2::ZERO, 0));
        gun.tick(|| self.random_gun_color());
        self.gun = gun;
        // Replace frog balls whose colour has vanished from the board.
        let present = self.colors_on_board();
        if !present.is_empty() {
            if let Some(c) = self.gun.current {
                if !present.contains(&c) && self.gun.state == super::gun::GunState::Ready {
                    self.gun.current = self.random_gun_color();
                }
            }
            if let Some(c) = self.gun.next {
                if !present.contains(&c) {
                    self.gun.next = self.random_gun_color();
                }
            }
        }

        // Chains.
        let back_left = self.zuma_back_left;
        for ci in 0..self.chains.len() {
            let v = self.chain_speed(ci, back_left);
            let can_spawn = !self.bar_full;
            let mut next_id = self.next_id;
            let res = {
                let (chain, rng) = (&mut self.chains[ci], &mut self.rng);
                chain.tick(v, can_spawn, &mut next_id, rng)
            };
            self.next_id = next_id;
            let mut merged = res.merged;
            merged.sort_unstable();
            merged.dedup();
            for &idx in merged.iter().rev() {
                self.handle_merge(ci, idx);
            }
            self.check_inserted(ci);
        }
        if self.zuma_back_left > 0.0 {
            self.zuma_back_left -= 3.0;
        }

        self.update_bullets();
        self.spawn_powerups();
        self.update_coin();

        // Danger / lose / win.
        let mut danger = false;
        let mut lost = false;
        for c in &self.chains {
            if let Some(h) = c.head_pos() {
                let d = c.len() - h;
                danger |= d < c.cfg.danger as f32;
                lost |= d <= 0.0;
            }
        }
        if danger && !self.danger {
            self.sound("SOUND_WARNING");
        }
        self.danger = danger;
        if lost {
            for c in &mut self.chains {
                c.lost = true;
            }
            self.bullets.clear();
            self.coin = None;
            self.sound("SOUND_LOST_LIFE");
            self.state = BoardState::Lost(0);
            return;
        }
        if self.bar_full && self.chains.iter().all(|c| c.balls.is_empty()) && self.bullets.is_empty() {
            self.state = BoardState::Won(0);
            self.trail = 0.0;
            self.coin = None;
        }
    }

    fn chain_speed(&mut self, ci: usize, zuma_back_left: f32) -> f32 {
        let c = &mut self.chains[ci];
        let cfg = &c.cfg;
        let mut v = cfg.speed;
        let head = c.head_pos().unwrap_or(-BALL_D);
        if c.roll_in {
            let target = c.len() * cfg.start as f32 / 100.0;
            let rem = target - head;
            if rem <= 0.0 || self.bar_full {
                c.roll_in = false;
            } else {
                return (rem / 40.0).clamp(cfg.speed, 5.0);
            }
        }
        let d = c.len() - head;
        if cfg.slow_factor > 1.0 && d < cfg.slow_distance as f32 {
            let t = 1.0 - (d / cfg.slow_distance as f32).clamp(0.0, 1.0);
            v /= 1.0 + (cfg.slow_factor - 1.0) * t;
        }
        if self.slow_ticks > 0 {
            v *= 0.3;
        }
        if self.zuma_slow_ticks > 0 {
            v *= 0.5;
        }
        if self.back_ticks > 0 {
            v = -(cfg.speed * 2.0).max(1.0);
        }
        if zuma_back_left > 0.0 {
            v = -3.0;
        }
        v
    }

    fn fire(&mut self) {
        let start = self.gun.ball_pos();
        let Some(color) = self.gun.fire() else { return };
        let mut speed = self.level.settings.fire_speed;
        if self.acc_ticks > 0 {
            speed *= 1.75;
        }
        let dir = self.gun.dir();
        self.bullets.push(Bullet { pos: start, vel: dir * speed, color, angle: self.gun.angle, min_gap: None, gaps: Vec::new() });
        self.sound("SOUND_BALLFIRE");
    }

    fn update_bullets(&mut self) {
        let mut i = 0;
        while i < self.bullets.len() {
            let mut remove = false;
            let steps = 4;
            for _ in 0..steps {
                let prev = self.bullets[i].pos;
                let step = self.bullets[i].vel / steps as f32;
                self.bullets[i].pos += step;
                let pos = self.bullets[i].pos;
                if self.try_collect_coin(pos) {
                    remove = true;
                    break;
                }
                self.track_gaps(i, prev, pos);
                if let Some((ci, bi)) = self.bullet_hit(pos) {
                    let b = self.bullets[i].clone();
                    self.insert_bullet(&b, ci, bi);
                    remove = true;
                    break;
                }
                if !(-60.0..700.0).contains(&pos.x) || !(-60.0..540.0).contains(&pos.y) {
                    remove = true;
                    if self.state == BoardState::Playing {
                        self.chain_count = 0;
                    }
                    break;
                }
            }
            if remove {
                self.bullets.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn bullet_hit(&self, pos: Vec2) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for (ci, c) in self.chains.iter().enumerate() {
            for (bi, b) in c.balls.iter().enumerate() {
                if !c.visible(b) {
                    continue;
                }
                let d = c.ball_draw_pos(b).distance_squared(pos);
                if d < BALL_D * BALL_D && best.is_none_or(|(_, _, bd)| d < bd) {
                    best = Some((ci, bi, d));
                }
            }
        }
        best.map(|(c, b, _)| (c, b))
    }

    /// Records gaps between chain segments that the bullet passes through.
    fn track_gaps(&mut self, bi: usize, prev: Vec2, pos: Vec2) {
        for (ci, c) in self.chains.iter().enumerate() {
            for i in 0..c.balls.len().saturating_sub(1) {
                if c.touching(i) {
                    continue;
                }
                let (a, b) = (&c.balls[i], &c.balls[i + 1]);
                if !c.visible(a) || !c.visible(b) {
                    continue;
                }
                let gap = b.pos - a.pos;
                if gap < BALL_D * 2.0 {
                    continue;
                }
                let (pa, pb) = (c.screen_pos(a.pos), c.screen_pos(b.pos));
                if segments_cross(prev, pos, pa, pb) {
                    let bullet = &mut self.bullets[bi];
                    if !bullet.gaps.iter().any(|g| *g == (ci, a.id, b.id)) {
                        bullet.gaps.push((ci, a.id, b.id));
                        let g = (gap - 64.0).max(0.0);
                        bullet.min_gap = Some(bullet.min_gap.map_or(g, |m: f32| m.min(g)));
                    }
                }
            }
        }
    }

    fn insert_bullet(&mut self, bullet: &Bullet, ci: usize, bi: usize) {
        let c = &mut self.chains[ci];
        let hit = &c.balls[bi];
        let hp = c.screen_pos(hit.pos);
        let tangent = c.screen_pos(hit.pos + 2.0) - c.screen_pos(hit.pos - 2.0);
        // The new ball starts with zero width half a ball from the hit one and
        // grows, pushing the balls ahead (see `Chain::spacing`).
        let (idx, pos) = if (bullet.pos - hp).dot(tangent) > 0.0 { (bi + 1, hit.pos + BALL_D * 0.5) } else { (bi, hit.pos - BALL_D * 0.5) };
        self.next_id += 1;
        let mut ball = Ball::new(self.next_id, bullet.color, pos);
        ball.from_shot = true;
        if let Some(g) = bullet.min_gap {
            let n = bullet.gaps.len() as i32;
            let base = (((500.0 * (300.0 - g)) / 3000.0) as i32 * 10).max(10);
            ball.gap_bonus = if n > 1 { base * n } else { base };
            ball.gap_count = n;
        }
        c.insert(ball, idx, bullet.pos);
        self.sound("SOUND_BALLCLICK1");
    }

    /// Match check for shot balls that finished sliding into the chain.
    fn check_inserted(&mut self, ci: usize) {
        loop {
            let c = &mut self.chains[ci];
            let Some(i) = c.balls.iter().position(|b| b.from_shot && !b.inserting()) else { return };
            c.balls[i].from_shot = false;
            let color = c.balls[i].color;
            match c.find_match(i) {
                Some((lo, hi)) => {
                    self.chain_count += 1;
                    self.stats.max_chain = self.stats.max_chain.max(self.chain_count);
                    self.explode(ci, lo, hi, 0, true);
                }
                None => {
                    let c = &self.chains[ci];
                    let same =
                        (i > 0 && c.touching(i - 1) && c.balls[i - 1].color == color) || (c.touching(i) && c.balls[i + 1].color == color);
                    if !same {
                        self.chain_count = 0;
                    }
                    self.chains[ci].balls[i].gap_bonus = 0;
                    self.chains[ci].balls[i].gap_count = 0;
                }
            }
        }
    }

    fn handle_merge(&mut self, ci: usize, idx: usize) {
        let c = &self.chains[ci];
        if idx >= c.balls.len() {
            return;
        }
        let combo = c.balls[idx].combo;
        let found = c.find_match(idx).or_else(|| if idx > 0 { c.find_match(idx - 1) } else { None });
        match found {
            // `combo` is set on the segment ahead of an explosion: a merge
            // right after one is the next step of a chain reaction.
            Some((lo, hi)) => self.explode(ci, lo, hi, combo, false),
            _ => {
                self.chains[ci].balls[idx].combo = 0;
                self.sound("SOUND_BALLCLICK2");
            }
        }
    }

    fn explode(&mut self, ci: usize, lo: usize, hi: usize, combo: i32, from_shot: bool) {
        let removed = self.chains[ci].remove(lo, hi);
        let n = removed.len() as i32;
        let color = removed[0].color;
        let mut center = Vec2::ZERO;
        let mut gap_bonus = 0;
        let mut gap_count = 0;
        let mut powers = Vec::new();
        for b in &removed {
            let p = self.chains[ci].ball_draw_pos(b);
            center += p;
            gap_bonus += b.gap_bonus;
            gap_count = gap_count.max(b.gap_count);
            self.effects.push(Effect { kind: EffectKind::Pop(b.color), pos: p, vel: Vec2::ZERO, age: 0, life: 26, variant: 0 });
            if let Some(pw) = b.power {
                powers.push((pw, p));
            }
        }
        center /= n as f32;
        self.stats.balls += n;
        self.stats.max_combo = self.stats.max_combo.max(combo);

        let mut points = 10 * n + 100 * combo + gap_bonus;
        let mut chain_bonus = 0;
        if from_shot && self.chain_count > 4 && combo == 0 {
            chain_bonus = (self.chain_count + 5) * 10;
            points += chain_bonus;
        }
        self.add_score(points);

        let rgb = BALL_RGB[color as usize % BALL_RGB.len()];
        let mut lines = vec![format!("+{points}")];
        if combo > 0 {
            lines.push(format!("COMBO x{}", combo + 1));
        }
        if gap_bonus > 0 {
            self.stats.gaps += 1;
            lines.push(match gap_count {
                0 | 1 => "GAP BONUS".to_string(),
                2 => "DOUBLE GAP BONUS".to_string(),
                _ => "TRIPLE GAP BONUS".to_string(),
            });
            self.sound("SOUND_GAP_BONUS");
        }
        if chain_bonus > 0 {
            lines.push(format!("CHAIN BONUS x{}", self.chain_count));
            self.sound("SOUND_CHAIN_BONUS");
        }
        for (k, l) in lines.into_iter().enumerate() {
            self.texts.push(FloatText { text: l, pos: center + Vec2::new(0.0, k as f32 * 14.0), color: rgb, age: 0, delay: 0 });
        }

        let snd = match combo {
            0 => "SOUND_BALLDESTROYED1",
            1 => "SOUND_BALLDESTROYED2",
            2 => "SOUND_BALLDESTROYED3",
            3 => "SOUND_BALLDESTROYED4",
            _ => "SOUND_BALLDESTROYED5",
        };
        self.sound(snd);
        if combo > 0 {
            self.sounds.push(SoundReq { id: "SOUND_COMBO", pitch: combo as f32 * 2.0, volume: (0.4 + 0.2 * combo as f32).min(1.0) });
        }

        // The segment ahead carries the next combo step if it rolls back.
        let c = &mut self.chains[ci];
        if lo < c.balls.len() && lo > 0 {
            c.balls[lo].combo = combo + 1;
        }
        for (pw, p) in powers {
            self.trigger_power(pw, p);
        }
    }

    fn trigger_power(&mut self, pw: Power, at: Vec2) {
        match pw {
            Power::Bomb => {
                self.sound("SOUND_EXPLODE");
                self.effects.push(Effect { kind: EffectKind::Blast, pos: at, vel: Vec2::ZERO, age: 0, life: 34, variant: 0 });
                let mut destroyed = 0;
                let mut chained = Vec::new();
                for c in &mut self.chains {
                    let mut i = c.balls.len();
                    while i > 0 {
                        i -= 1;
                        let b = &c.balls[i];
                        if c.visible(b) && c.ball_draw_pos(b).distance(at) < BOMB_RADIUS {
                            let p = c.ball_draw_pos(b);
                            let b = c.balls.remove(i);
                            destroyed += 1;
                            self.effects.push(Effect {
                                kind: EffectKind::Pop(b.color),
                                pos: p,
                                vel: Vec2::ZERO,
                                age: 0,
                                life: 26,
                                variant: 0,
                            });
                            if let Some(pw2) = b.power {
                                if pw2 != Power::Bomb {
                                    chained.push((pw2, p));
                                }
                            }
                        }
                    }
                }
                if destroyed > 0 {
                    let pts = destroyed * 10;
                    self.add_score(pts);
                    self.texts.push(FloatText { text: format!("+{pts}"), pos: at, color: (255, 200, 0), age: 0, delay: 0 });
                }
                for (p, a) in chained {
                    self.trigger_power(p, a);
                }
            }
            Power::Slow => {
                self.slow_ticks = SLOW_TICKS;
                self.back_ticks = 0;
                self.sound("SOUND_SLOWDOWN_BALL");
                self.texts.push(FloatText { text: "SLOWDOWN Ball".into(), pos: at, color: (255, 255, 0), age: 0, delay: 20 });
            }
            Power::Backwards => {
                self.back_ticks = BACK_TICKS;
                self.sound("SOUND_BACKWARDS_BALL");
                self.texts.push(FloatText { text: "BACKWARDS Ball".into(), pos: at, color: (255, 255, 0), age: 0, delay: 20 });
            }
            Power::Accuracy => {
                self.acc_ticks = ACCURACY_TICKS;
                self.sound("SOUND_ACCURACY_BALL");
                self.texts.push(FloatText { text: "ACCURACY Ball".into(), pos: at, color: (255, 255, 0), age: 0, delay: 20 });
            }
        }
    }

    fn add_score(&mut self, pts: i32) {
        self.score += pts;
        self.level_score += pts;
        if !self.bar_full && self.level_score >= self.target {
            self.bar_full = true;
            self.sound("SOUND_BAR_FULL");
            self.zuma_back_left = self.chains.first().map(|c| c.cfg.zuma_back as f32).unwrap_or(0.0);
            self.zuma_slow_ticks = self.chains.first().map(|c| c.cfg.zuma_slow).unwrap_or(0);
            self.coin = None;
            for c in &mut self.chains {
                c.roll_in = false;
            }
        }
        while self.score >= self.next_life {
            self.next_life += EXTRA_LIFE_EVERY;
            self.extra_lives += 1;
            self.sound("SOUND_EXTRA_LIFE");
            let p = self.gun.center - Vec2::new(0.0, 60.0);
            self.texts.push(FloatText { text: "EXTRA LIFE!".into(), pos: p, color: (255, 255, 0), age: 0, delay: 0 });
        }
    }

    fn spawn_powerups(&mut self) {
        if self.bar_full {
            return;
        }
        for ci in 0..self.chains.len() {
            let freqs = {
                let cfg = &self.chains[ci].cfg;
                [
                    (Power::Bomb, cfg.bomb_freq),
                    (Power::Slow, cfg.slow_freq),
                    (Power::Accuracy, cfg.acc_freq),
                    (Power::Backwards, cfg.back_freq),
                ]
            };
            for (pw, f) in freqs {
                if f <= 0 || self.rng.random_range(0..f) != 0 {
                    continue;
                }
                let c = &self.chains[ci];
                let candidates: Vec<usize> = c
                    .balls
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.power.is_none() && !b.inserting() && c.visible(b))
                    .map(|(i, _)| i)
                    .collect();
                if candidates.is_empty() {
                    continue;
                }
                let i = candidates[self.rng.random_range(0..candidates.len())];
                let b = &mut self.chains[ci].balls[i];
                b.power = Some(pw);
                b.power_ticks = POWER_TICKS;
            }
        }
    }

    fn update_coin(&mut self) {
        if let Some(coin) = &mut self.coin {
            coin.age += 1;
            if coin.age > COIN_TICKS {
                self.coin = None;
                self.last_coin = self.ticks;
                self.sound("SOUND_JEWEL_DISAPPEAR");
            }
            return;
        }
        let freq = self.level.settings.treasure_freq.max(1);
        if self.bar_full || self.level.graphics.treasure.is_empty() {
            return;
        }
        if self.rng.random_range(0..freq) != 0 || self.ticks - self.last_coin <= freq {
            return;
        }
        let eligible: Vec<Vec2> = self
            .level
            .graphics
            .treasure
            .iter()
            .filter(|t| t.dist.iter().enumerate().all(|(n, &d)| d < 0 || self.chains.get(n).is_none_or(|c| c.progress() >= d as f32)))
            .map(|t| Vec2::new(t.x as f32, t.y as f32))
            .collect();
        if eligible.is_empty() {
            return;
        }
        let p = eligible[self.rng.random_range(0..eligible.len())];
        self.coin = Some(Coin { pos: p, age: 0 });
        self.sound("SOUND_JEWEL_APPEAR");
        self.sparkles(p, 4);
    }

    fn try_collect_coin(&mut self, pos: Vec2) -> bool {
        let Some(coin) = &self.coin else { return false };
        if coin.pos.distance(pos) >= BALL_D * 0.5 + 20.0 {
            return false;
        }
        let at = coin.pos;
        self.coin = None;
        self.last_coin = self.ticks;
        let pts = (((self.target - self.level_score) / 600) * 100).max(500);
        self.add_score(pts);
        self.stats.coins += 1;
        self.sound("SOUND_JEWEL_HIT");
        self.texts.push(FloatText { text: format!("BONUS +{pts}"), pos: at, color: (255, 255, 0), age: 0, delay: 0 });
        self.sparkles(at, 10);
        true
    }

    fn sparkles(&mut self, at: Vec2, n: usize) {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU + self.rng.random_range(0.0..0.5);
            let v = Vec2::new(a.cos(), a.sin()) * 2.0;
            self.effects.push(Effect {
                kind: EffectKind::Sparkle,
                pos: at + v * 8.0,
                vel: v,
                age: 0,
                life: 40,
                variant: self.rng.random_range(0..5),
            });
        }
    }

    /// The skull over each hole opens as the chain head gets close.
    fn update_holes(&mut self) {
        self.hole_open.resize(self.chains.len(), 0.0);
        for (i, c) in self.chains.iter().enumerate() {
            let target = match self.state {
                BoardState::Lost(_) => 11.0,
                BoardState::Playing => match c.head_pos() {
                    Some(h) => {
                        let danger = c.cfg.danger.max(1) as f32;
                        ((1.0 - (c.len() - h) / danger) * 11.0).clamp(0.0, 11.0)
                    }
                    None => 0.0,
                },
                _ => 0.0,
            };
            let cur = &mut self.hole_open[i];
            *cur += (target - *cur).clamp(-0.25, 0.25);
        }
    }

    fn update_effects(&mut self) {
        for e in &mut self.effects {
            e.age += 1;
            e.pos += e.vel;
            e.vel *= 0.95;
        }
        self.effects.retain(|e| e.age < e.life);
        for t in &mut self.texts {
            if t.delay > 0 {
                t.delay -= 1;
            } else {
                t.age += 1;
                t.pos.y -= 0.4;
            }
        }
        self.texts.retain(|t| t.age < 150);
    }
}

/// Whether segment p1-p2 crosses segment q1-q2.
fn segments_cross(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let d = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let d1 = d(q1, q2, p1);
    let d2 = d(q1, q2, p2);
    let d3 = d(p1, p2, q1);
    let d4 = d(p1, p2, q2);
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::curve::PathPoint;
    use crate::data::levels::{Graphics, LevelSettings};

    fn straight_level(len: usize) -> (LevelDesc, Vec<Arc<CurvePath>>) {
        let points = (0..len).map(|i| PathPoint { x: i as f32, y: 100.0, in_tunnel: false, priority: 1 }).collect();
        let path = Arc::new(CurvePath { points });
        let level = LevelDesc { graphics: Graphics { gun: (320, 300), ..Default::default() }, settings: LevelSettings::default() };
        (level, vec![path])
    }

    #[test]
    fn chain_rolls_in_and_scores_matches() {
        let (level, paths) = straight_level(2000);
        let mut b = Board::new(level, paths, 4, 0, "TEST".into());
        let input = BoardInput { aim: Vec2::new(320.0, 0.0), ..Default::default() };
        for _ in 0..400 {
            b.update(&input);
        }
        assert_eq!(b.state, BoardState::Playing);
        let c = &b.chains[0];
        assert!(c.balls.len() > 10, "balls rolled in: {}", c.balls.len());
        // Balls never overlap.
        for w in c.balls.windows(2) {
            assert!(w[1].pos - w[0].pos >= BALL_D - 0.01);
        }

        // Make three touching balls of one colour and check they explode.
        let n = b.chains[0].balls.len();
        let head = n - 1;
        for i in head - 2..=head {
            b.chains[0].balls[i].color = 5;
        }
        b.chains[0].balls[head - 3].color = 0;
        b.chains[0].balls[head - 1].from_shot = true;
        let before = b.score;
        b.check_inserted(0);
        assert_eq!(b.score - before, 30);
        assert_eq!(b.chains[0].balls.len(), n - 3);
    }

    #[test]
    fn matching_gap_rolls_back_into_a_combo() {
        let (level, paths) = straight_level(3000);
        let mut b = Board::new(level, paths, 4, 0, "TEST".into());
        b.state = BoardState::Playing;
        b.bar_full = true; // no new balls
        let c = &mut b.chains[0];
        c.roll_in = false;
        for (i, (col, pos)) in [(0, 100.0), (1, 132.0), (1, 164.0), (1, 400.0), (2, 432.0)].into_iter().enumerate() {
            c.balls.push(Ball::new(i as u32, col, pos));
        }
        // As left by an explosion that opened the gap.
        c.balls[3].combo = 1;
        let input = BoardInput { aim: Vec2::new(320.0, 0.0), ..Default::default() };
        for _ in 0..1000 {
            b.update(&input);
            if b.chains[0].balls.len() == 2 {
                break;
            }
        }
        let colors: Vec<u8> = b.chains[0].balls.iter().map(|x| x.color).collect();
        assert_eq!(colors, vec![0, 2], "the three colour-1 balls exploded");
        assert_eq!(b.stats.max_combo, 1);
        // 10 per ball + 100 for the combo step.
        assert_eq!(b.score, 3 * 10 + 100);
    }

    #[test]
    fn inserted_ball_opens_the_chain_smoothly() {
        let (level, paths) = straight_level(3000);
        let mut b = Board::new(level, paths, 4, 0, "TEST".into());
        b.state = BoardState::Playing;
        b.bar_full = true;
        let c = &mut b.chains[0];
        c.roll_in = false;
        for (i, pos) in [500.0, 532.0, 564.0].into_iter().enumerate() {
            c.balls.push(Ball::new(i as u32, i as u8, pos));
        }
        let mut shot = Ball::new(99, 3, 532.0 - BALL_D * 0.5);
        shot.from_shot = true;
        c.insert(shot, 1, Vec2::new(532.0, 300.0));
        let gap_after = |b: &Board| b.chains[0].balls[2].pos - b.chains[0].balls[0].pos;
        let mut last = gap_after(&b);
        assert!((last - BALL_D).abs() < 0.01, "no jump at insertion time");
        let input = BoardInput { aim: Vec2::new(320.0, 0.0), ..Default::default() };
        for _ in 0..20 {
            b.update(&input);
            let g = gap_after(&b);
            assert!(g >= last - 0.01 && g - last <= BALL_D * 0.2, "grows gradually: {last} -> {g}");
            last = g;
        }
        assert!((last - 2.0 * BALL_D).abs() < 0.01, "ends one ball wider: {last}");
        let ids: Vec<u32> = b.chains[0].balls.iter().map(|x| x.id).collect();
        assert_eq!(ids, vec![0, 99, 1, 2]);
    }

    #[test]
    fn gap_detection_crosses() {
        assert!(segments_cross(Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)));
        assert!(!segments_cross(Vec2::new(2.0, -1.0), Vec2::new(2.0, 1.0), Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)));
    }
}
