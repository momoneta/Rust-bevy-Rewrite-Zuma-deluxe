//! Player progress and options, saved as a small text file
//! (`key=value` lines) in the user's config directory.

use bevy::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Resource, Debug, Clone)]
pub struct Profile {
    /// Highest adventure stage reached (0-based, 0..=12).
    pub max_stage: usize,
    /// Highest level index reached inside `max_stage`.
    pub max_level: usize,
    /// Best adventure score.
    pub adventure_best: i32,
    /// Graphics ids unlocked for Gauntlet mode (played in Adventure).
    pub unlocked: Vec<String>,
    /// Best Gauntlet score per graphics id.
    pub gauntlet_best: BTreeMap<String, i32>,
    /// Best level time (seconds) per graphics id.
    pub best_time: BTreeMap<String, i32>,
    pub sound_volume: f32,
    pub music_volume: f32,
    pub fullscreen: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            max_stage: 0,
            max_level: 0,
            adventure_best: 0,
            unlocked: Vec::new(),
            gauntlet_best: BTreeMap::new(),
            best_time: BTreeMap::new(),
            sound_volume: 0.8,
            music_volume: 0.6,
            fullscreen: false,
        }
    }
}

fn profile_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("userdata"));
    base.join("zuma-bevy").join("profile.txt")
}

impl Profile {
    pub fn load() -> Self {
        let mut p = Profile::default();
        let Ok(text) = std::fs::read_to_string(profile_path()) else { return p };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "max_stage" => p.max_stage = v.parse().unwrap_or(0),
                "max_level" => p.max_level = v.parse().unwrap_or(0),
                "adventure_best" => p.adventure_best = v.parse().unwrap_or(0),
                "unlocked" => p.unlocked = v.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect(),
                "sound_volume" => p.sound_volume = v.parse().unwrap_or(0.8),
                "music_volume" => p.music_volume = v.parse().unwrap_or(0.6),
                "fullscreen" => p.fullscreen = v == "1",
                k if k.starts_with("gauntlet.") => {
                    p.gauntlet_best.insert(k[9..].to_string(), v.parse().unwrap_or(0));
                }
                k if k.starts_with("time.") => {
                    p.best_time.insert(k[5..].to_string(), v.parse().unwrap_or(0));
                }
                _ => {}
            }
        }
        p
    }

    pub fn save(&self) {
        let mut s = String::new();
        s += &format!("max_stage={}\nmax_level={}\nadventure_best={}\n", self.max_stage, self.max_level, self.adventure_best);
        s += &format!("unlocked={}\n", self.unlocked.join(","));
        s += &format!("sound_volume={}\nmusic_volume={}\nfullscreen={}\n", self.sound_volume, self.music_volume, self.fullscreen as u8);
        for (k, v) in &self.gauntlet_best {
            s += &format!("gauntlet.{k}={v}\n");
        }
        for (k, v) in &self.best_time {
            s += &format!("time.{k}={v}\n");
        }
        let path = profile_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, s) {
            warn!("could not save profile to {}: {e}", path.display());
        }
    }

    pub fn unlock(&mut self, graphics: &str) {
        if !self.unlocked.iter().any(|u| u.eq_ignore_ascii_case(graphics)) {
            self.unlocked.push(graphics.to_ascii_lowercase());
        }
    }

    pub fn is_unlocked(&self, graphics: &str) -> bool {
        self.unlocked.iter().any(|u| u.eq_ignore_ascii_case(graphics))
    }
}
