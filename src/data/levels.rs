//! `levels/levels.xml`: graphics, difficulty settings and progressions.
//!
//! Default values come from the game's constructors (`FUN_004e29f0` for the
//! level, the constructor at `0x004d5e90` for each curve), and the attribute
//! names from the parser `FUN_004f06b0`.

use super::xml::{self, Element};
use std::collections::HashMap;

#[derive(Debug, Clone)]
#[allow(dead_code)] // `pri` is always 1 in the shipped levels
pub struct Cutout {
    pub image: String,
    pub pri: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct TreasurePoint {
    pub x: i32,
    pub y: i32,
    /// Minimum progress (percent of the path) of each curve's chain before
    /// a coin may appear here.
    pub dist: [i32; 3],
}

/// Background pieces that move during the earthquake intro of some levels
/// (parsed, not animated yet).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BackgroundAlpha {
    pub image: String,
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
}

#[derive(Debug, Clone, Default)]
pub struct Graphics {
    pub id: String,
    pub curves: Vec<String>,
    pub image: Option<String>,
    pub display_name: String,
    pub gun: (i32, i32),
    pub space: bool,
    pub cutouts: Vec<Cutout>,
    pub treasure: Vec<TreasurePoint>,
    pub bg_alpha: Vec<BackgroundAlpha>,
    pub attrs: Vec<(String, String)>,
}

/// Per-curve difficulty settings.
#[derive(Debug, Clone)]
pub struct CurveSettings {
    /// Distance (path points) from the end where the danger warning starts.
    pub danger: i32,
    /// Chain speed in path points per 1/100 s tick.
    pub speed: f32,
    /// Percent of the path covered by the initial fast roll-in.
    pub start: i32,
    /// Acceleration of a segment rolling back to merge with a matching one.
    pub merge_speed: f32,
    /// Total balls in the level (0 = endless until the gauge is full).
    pub balls: i32,
    /// Percent chance that the next ball repeats the previous colour.
    pub repeat: i32,
    /// Percent chance that a colour run may be a single ball.
    pub single: i32,
    pub colors: i32,
    pub colors_set: bool,
    /// Speed divisor when the head is close to the hole.
    pub slow_factor: f32,
    pub slow_speed: f32,
    pub slow_distance: i32,
    pub acc_rate: f32,
    /// Points needed to fill the Zuma gauge.
    pub score: i32,
    pub skull_rot: i32,
    pub zuma_back: i32,
    pub zuma_slow: i32,
    pub max_speed: f32,
    pub bomb_freq: i32,
    pub slow_freq: i32,
    pub acc_freq: i32,
    pub back_freq: i32,
}

impl Default for CurveSettings {
    fn default() -> Self {
        Self {
            danger: 600,
            speed: 0.5,
            start: 40,
            merge_speed: 0.05,
            balls: 0,
            repeat: 40,
            single: 10,
            colors: 4,
            colors_set: false,
            slow_factor: 4.0,
            slow_speed: 0.0,
            slow_distance: 500,
            acc_rate: 0.0,
            score: 1000,
            skull_rot: -1,
            zuma_back: 300,
            zuma_slow: 1100,
            max_speed: 100.0,
            bomb_freq: 3000,
            slow_freq: 3000,
            acc_freq: 3000,
            back_freq: 3000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LevelSettings {
    pub fire_speed: f32,
    pub reload_delay: i32,
    pub treasure_freq: i32,
    /// Ace time in seconds (0 = derived from the gauge score).
    pub par_time: i32,
    pub difficulty: i32,
    pub curves: [CurveSettings; 3],
}

impl Default for LevelSettings {
    fn default() -> Self {
        Self { fire_speed: 6.0, reload_delay: 0, treasure_freq: 300, par_time: 0, difficulty: 0, curves: Default::default() }
    }
}

impl LevelSettings {
    fn apply(&mut self, attrs: &[(String, String)]) {
        let get = |k: &str| attrs.iter().rev().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
        let f = |k: &str| get(k).and_then(xml::parse_f32);
        let i = |k: &str| get(k).and_then(xml::parse_i32);
        if let Some(v) = f("firespeed") {
            self.fire_speed = v;
        }
        if let Some(v) = i("reloaddelay") {
            self.reload_delay = v;
        }
        if let Some(v) = i("treasurefreq") {
            self.treasure_freq = v.max(1);
        }
        if let Some(v) = i("partime") {
            self.par_time = v;
        }
        if let Some(v) = i("difficulty") {
            self.difficulty = v;
        }
        // Per-curve values: "a" applies to every curve, "a,b,c" per curve.
        let per_curve = |k: &str| -> Option<Vec<String>> { get(k).map(|v| v.split(',').map(|s| s.trim().to_string()).collect()) };
        macro_rules! curve_field {
            ($key:literal, $field:ident, $parse:path) => {
                if let Some(vals) = per_curve($key) {
                    for (n, c) in self.curves.iter_mut().enumerate() {
                        let s = vals.get(n).or(vals.last()).unwrap();
                        if let Some(v) = $parse(s) {
                            c.$field = v as _;
                        }
                    }
                }
            };
        }
        curve_field!("danger", danger, xml::parse_i32);
        curve_field!("speed", speed, xml::parse_f32);
        curve_field!("start", start, xml::parse_i32);
        curve_field!("mergespeed", merge_speed, xml::parse_f32);
        curve_field!("balls", balls, xml::parse_i32);
        curve_field!("repeat", repeat, xml::parse_i32);
        curve_field!("single", single, xml::parse_i32);
        curve_field!("colors", colors, xml::parse_i32);
        curve_field!("slowfactor", slow_factor, xml::parse_f32);
        curve_field!("slowspeed", slow_speed, xml::parse_f32);
        curve_field!("slowdistance", slow_distance, xml::parse_i32);
        curve_field!("accrate", acc_rate, xml::parse_f32);
        curve_field!("score", score, xml::parse_i32);
        curve_field!("skullrot", skull_rot, xml::parse_i32);
        curve_field!("zumaback", zuma_back, xml::parse_i32);
        curve_field!("zumaslow", zuma_slow, xml::parse_i32);
        curve_field!("maxspeed", max_speed, xml::parse_f32);
        curve_field!("bombfreq", bomb_freq, xml::parse_i32);
        curve_field!("slowfreq", slow_freq, xml::parse_i32);
        curve_field!("accfreq", acc_freq, xml::parse_i32);
        curve_field!("backfreq", back_freq, xml::parse_i32);
        if get("colors").is_some() {
            for c in &mut self.curves {
                c.colors_set = true;
            }
        }
        // powerfreq overrides every enabled power-up frequency.
        if let Some(p) = i("powerfreq") {
            for c in &mut self.curves {
                for f in [&mut c.bomb_freq, &mut c.slow_freq, &mut c.acc_freq, &mut c.back_freq] {
                    if *f > 0 {
                        *f = p;
                    }
                }
            }
        }
    }

    /// Finishes derived values once all attributes are applied.
    fn finish(&mut self) {
        for c in &mut self.curves {
            if c.slow_speed != 0.0 {
                c.slow_factor = c.speed / c.slow_speed;
            }
        }
        if self.par_time == 0 {
            // FUN_004f06b0: partime = score * 35 / 1000, rounded up to 5 s.
            let t = self.curves[0].score * 35 / 1000;
            self.par_time = (t + 4) / 5 * 5;
        }
    }
}

/// A playable level: graphics + settings.
#[derive(Debug, Clone)]
pub struct LevelDesc {
    pub graphics: Graphics,
    pub settings: LevelSettings,
}

#[derive(Debug, Default)]
pub struct LevelDb {
    pub graphics: HashMap<String, Graphics>,
    settings: HashMap<String, Vec<(String, String)>>,
    progressions: HashMap<String, (Vec<String>, Vec<i32>)>,
    /// `<Level>` entries expanded through their progression (Gauntlet).
    pub gauntlet: Vec<GauntletLevel>,
    /// Adventure mode: stage -> (graphics id, settings id).
    pub stages: Vec<Vec<(String, String)>>,
}

#[derive(Debug, Clone)]
pub struct GauntletLevel {
    pub graphics: String,
    /// (settings id, difficulty) per gauntlet round.
    pub rounds: Vec<(String, i32)>,
}

impl LevelDb {
    pub fn parse(text: &str) -> Result<Self, String> {
        let doc = xml::parse(text);
        let root = doc.children.iter().find(|e| e.is("Levels")).ok_or("Expecting Levels tag")?;
        let mut db = LevelDb::default();
        for el in &root.children {
            if el.is("Graphics") {
                let g = parse_graphics(el);
                db.graphics.insert(g.id.to_ascii_lowercase(), g);
            } else if el.is("Settings") {
                if let Some(id) = el.attr("id") {
                    db.settings.insert(id.to_ascii_lowercase(), el.attrs.clone());
                }
            } else if el.is("LevelProgression") {
                let id = el.attr("id").unwrap_or_default().to_ascii_lowercase();
                let settings = split_list(el.attr("settings").unwrap_or_default());
                let diff = split_list(el.attr("difficulty").unwrap_or_default()).iter().map(|s| xml::parse_i32(s).unwrap_or(0)).collect();
                db.progressions.insert(id, (settings, diff));
            } else if el.is("Level") {
                let graphics = el.attr("graphics").unwrap_or_default().to_ascii_lowercase();
                let mut rounds = Vec::new();
                if let Some(p) = el.attr("progression") {
                    let dd = el.attr_i32("difficultydiff").unwrap_or(0);
                    if let Some((settings, diff)) = db.progressions.get(&p.to_ascii_lowercase()) {
                        for (n, s) in settings.iter().enumerate() {
                            rounds.push((s.clone(), diff.get(n).copied().unwrap_or(0) + dd));
                        }
                    }
                } else if let Some(s) = el.attr("settings") {
                    rounds.push((s.to_string(), 0));
                }
                db.gauntlet.push(GauntletLevel { graphics, rounds });
            } else if el.is("StageProgression") {
                db.stages.clear();
                for n in 1.. {
                    let (Some(st), Some(df)) = (el.attr(&format!("stage{n}")), el.attr(&format!("diffi{n}"))) else {
                        break;
                    };
                    let st = split_list(st);
                    let df = split_list(df);
                    db.stages.push(st.into_iter().zip(df).collect());
                }
            }
        }
        Ok(db)
    }

    pub fn level(&self, graphics: &str, settings: &str, difficulty: i32) -> Option<LevelDesc> {
        let g = self.graphics.get(&graphics.to_ascii_lowercase())?.clone();
        let mut s = LevelSettings::default();
        if let Some(attrs) = self.settings.get(&settings.to_ascii_lowercase()) {
            s.apply(attrs);
        } else if let Some(d) = xml::parse_i32(settings) {
            // The commented-out easy progression uses plain numbers.
            s.difficulty = d;
        }
        s.apply(&g.attrs);
        if difficulty != 0 {
            s.difficulty = difficulty;
        }
        s.finish();
        Some(LevelDesc { graphics: g, settings: s })
    }

    pub fn adventure_level(&self, stage: usize, index: usize) -> Option<LevelDesc> {
        let (g, s) = self.stages.get(stage)?.get(index)?;
        self.level(g, s, 0)
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Attributes on `<Graphics>` that are graphics data rather than settings.
const GRAPHICS_KEYS: &[&str] = &["id", "curve", "curve1", "curve2", "curve3", "image", "dispname", "gx", "gy", "space"];

fn parse_graphics(el: &Element) -> Graphics {
    let id = el.attr("id").unwrap_or_default().to_string();
    let mut curves = Vec::new();
    for key in ["curve", "curve1", "curve2", "curve3"] {
        if let Some(c) = el.attr(key) {
            if key == "curve1" && !curves.is_empty() {
                curves[0] = c.to_string();
            } else {
                curves.push(c.to_string());
            }
        }
    }
    let mut g = Graphics {
        display_name: el.attr("dispname").unwrap_or(&id).to_string(),
        image: el.attr("image").map(str::to_string),
        gun: (el.attr_i32("gx").unwrap_or(320), el.attr_i32("gy").unwrap_or(240)),
        space: el.attr("space").is_some_and(|v| v.eq_ignore_ascii_case("true")),
        curves,
        id,
        ..Default::default()
    };
    g.attrs = el.attrs.iter().filter(|(k, _)| !GRAPHICS_KEYS.iter().any(|g| g.eq_ignore_ascii_case(k))).cloned().collect();
    for c in &el.children {
        if c.is("Cutout") {
            g.cutouts.push(Cutout {
                image: c.attr("image").unwrap_or_default().to_string(),
                pri: c.attr_i32("pri").unwrap_or(0),
                x: c.attr_i32("x").unwrap_or(0),
                y: c.attr_i32("y").unwrap_or(0),
            });
        } else if c.is("TreasurePoint") {
            // -1 = no requirement on that curve (a point listed only with
            // `dist2` must not be gated by curve 1).
            let d = |k: &str| c.attr_i32(k).unwrap_or(-1);
            let dist = [d("dist1"), d("dist2"), d("dist3")];
            g.treasure.push(TreasurePoint { x: c.attr_i32("x").unwrap_or(0), y: c.attr_i32("y").unwrap_or(0), dist });
        } else if c.is("BackgroundAlpha") {
            g.bg_alpha.push(BackgroundAlpha {
                image: c.attr("image").unwrap_or_default().to_string(),
                x: c.attr_i32("x").unwrap_or(0),
                y: c.attr_i32("y").unwrap_or(0),
                vx: c.attr_i32("vx").unwrap_or(0),
                vy: c.attr_i32("vy").unwrap_or(0),
            });
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_settings_and_graphics() {
        let db = LevelDb::parse(
            r#"<Levels>
            <Graphics id="serpents" image="serpents" curve="serpents-1" curve2="serpents-2" skullrot="0" dispname="Mirror Serpent" gx="339" gy="271">
              <Cutout pri="1" image="serpentsT" x="0" y="0" />
              <TreasurePoint x="123" y="125" dist2="72" />
            </Graphics>
            <Settings id="lvl35" speed="0.60" start="45" score="4500" single="8" zumaslow="500" zumaback="200" partime="130"/>
            <Settings id="dual13" speed="0.80" powerfreq=2900 />
            <LevelProgression id="dual" settings="dual13" difficulty="16" />
            <Level graphics="serpents" progression="dual" />
            <StageProgression stage1 = "serpents" diffi1 = "lvl35" />
            </Levels>"#,
        )
        .unwrap();
        let l = db.adventure_level(0, 0).unwrap();
        assert_eq!(l.graphics.curves, vec!["serpents-1", "serpents-2"]);
        assert_eq!(l.graphics.gun, (339, 271));
        assert_eq!(l.settings.par_time, 130);
        assert_eq!(l.settings.curves[1].speed, 0.6);
        assert_eq!(l.settings.curves[0].skull_rot, 0);
        assert_eq!(l.settings.curves[0].single, 8);
        assert_eq!(l.graphics.treasure[0].dist, [-1, 72, -1]);
        let g = &db.gauntlet[0];
        assert_eq!(g.rounds, vec![("dual13".to_string(), 16)]);
        let l = db.level(&g.graphics, &g.rounds[0].0, g.rounds[0].1).unwrap();
        assert_eq!(l.settings.curves[0].bomb_freq, 2900);
        assert_eq!(l.settings.difficulty, 16);
        // partime derived from the default score of 1000.
        assert_eq!(l.settings.par_time, 35);
    }
}
