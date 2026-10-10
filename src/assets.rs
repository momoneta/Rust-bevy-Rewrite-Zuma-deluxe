//! Game resources (images, fonts, sounds) loaded from the original data
//! folder and turned into Bevy assets.

use crate::data::font::FontDesc;
use crate::data::image_load::{self, split_cells};
use crate::data::levels::LevelDb;
use crate::data::manifest::Manifest;
use crate::data::vfs::{DataDir, SOUND_EXTS};
use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;
use std::sync::Arc;

/// An image split into its animation cells.
#[derive(Clone, Debug)]
pub struct Img {
    pub cells: Vec<Handle<Image>>,
    /// The whole strip, for sub-rectangle draws.
    pub full: Handle<Image>,
    pub w: f32,
    pub h: f32,
    /// Grid size of the strip (cells are stored row-major).
    #[allow(dead_code)]
    pub rows: u32,
    pub cols: u32,
}

impl Img {
    pub fn cell(&self, i: usize) -> &Handle<Image> {
        &self.cells[i.min(self.cells.len() - 1)]
    }
    pub fn count(&self) -> usize {
        self.cells.len()
    }
}

#[derive(Clone, Debug)]
pub struct Font {
    pub desc: Arc<FontDesc>,
    /// One texture per layer (same order as `desc.layers`).
    pub textures: Vec<Option<Handle<Image>>>,
    pub tags: Vec<String>,
}

impl Font {
    pub fn width(&self, s: &str) -> f32 {
        self.desc.string_width(s) as f32
    }
    pub fn ascent(&self) -> f32 {
        self.desc.ascent() as f32
    }
    pub fn height(&self) -> f32 {
        self.desc.height() as f32
    }
    pub fn line_spacing(&self) -> f32 {
        self.desc.line_spacing() as f32
    }
    pub fn with_tags(&self, tags: &[&str]) -> Font {
        Font { tags: tags.iter().map(|s| s.to_string()).collect(), ..self.clone() }
    }
}

#[derive(Clone, Debug)]
pub struct Sound {
    pub handle: Handle<AudioSource>,
    pub volume: f32,
}

/// Resource groups loaded at start-up, in order.
const GROUPS: &[&str] = &["Init", "Register", "LoadingThread", "MainMenu", "AdventureScreen", "GauntletScreen", "HelpScreen"];

#[derive(Resource)]
pub struct GameAssets {
    pub dir: DataDir,
    pub manifest: Manifest,
    pub levels: LevelDb,
    pub images: HashMap<String, Img>,
    pub fonts: HashMap<String, Font>,
    pub sounds: HashMap<String, Sound>,
    /// The eight subsongs of zuma.mo3, converted to ogg (see tools/convert_music.sh).
    pub music: Vec<Option<Handle<AudioSource>>>,
    music_loaded: bool,
    pub white: Handle<Image>,
    level_images: HashMap<String, Option<Img>>,
    pending: Vec<String>,
    total: usize,
}

pub fn to_bevy_image(img: &image::RgbaImage) -> Image {
    Image::new(
        Extent3d { width: img.width().max(1), height: img.height().max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        if img.width() == 0 || img.height() == 0 { vec![0; 4] } else { img.as_raw().clone() },
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_img(images: &mut Assets<Image>, raw: image::RgbaImage, rows: u32, cols: u32) -> Img {
    let full = images.add(to_bevy_image(&raw));
    let cells = if rows * cols > 1 {
        split_cells(&raw, rows, cols).iter().map(|c| images.add(to_bevy_image(c))).collect()
    } else {
        vec![full.clone()]
    };
    Img { cells, full, w: (raw.width() / cols.max(1)) as f32, h: (raw.height() / rows.max(1)) as f32, rows, cols }
}

impl GameAssets {
    pub fn new(dir: DataDir, images: &mut Assets<Image>) -> Result<Self, String> {
        let manifest_text = dir.read_string("properties/resources.xml").ok_or("properties/resources.xml not found")?;
        let manifest = Manifest::parse(&manifest_text);
        let levels_text = dir.read_string("levels/levels.xml").ok_or("levels/levels.xml not found")?;
        let levels = LevelDb::parse(&levels_text)?;
        let white = images.add(to_bevy_image(&image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]))));
        let mut pending = Vec::new();
        for g in GROUPS {
            if let Some((_, ids)) = manifest.groups.iter().find(|(n, _)| n.eq_ignore_ascii_case(g)) {
                pending.extend(ids.iter().cloned());
            }
        }
        pending.reverse();
        let total = pending.len();
        Ok(Self {
            dir,
            manifest,
            levels,
            images: HashMap::new(),
            fonts: HashMap::new(),
            sounds: HashMap::new(),
            music: Vec::new(),
            music_loaded: false,
            white,
            level_images: HashMap::new(),
            pending,
            total,
        })
    }

    /// Loading progress in 0..=1.
    pub fn progress(&self) -> f32 {
        if self.total == 0 { 1.0 } else { 1.0 - self.pending.len() as f32 / self.total as f32 }
    }

    /// Loads resources for up to `budget` and returns true when everything is loaded.
    pub fn load_some(&mut self, images: &mut Assets<Image>, audio: &mut Assets<AudioSource>, budget: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        while let Some(id) = self.pending.pop() {
            self.load_resource(&id, images, audio);
            if start.elapsed() > budget {
                break;
            }
        }
        if self.pending.is_empty() && !self.music_loaded {
            self.music_loaded = true;
            self.load_music(audio);
        }
        self.pending.is_empty()
    }

    /// Loads every resource of the given group immediately.
    pub fn load_group_now(&mut self, group: &str, images: &mut Assets<Image>, audio: &mut Assets<AudioSource>) {
        let ids = self.manifest.groups.iter().find(|(n, _)| n.eq_ignore_ascii_case(group)).map(|(_, ids)| ids.clone()).unwrap_or_default();
        for id in ids {
            self.pending.retain(|p| p != &id);
            self.load_resource(&id, images, audio);
        }
    }

    fn load_resource(&mut self, id: &str, images: &mut Assets<Image>, audio: &mut Assets<AudioSource>) {
        if let Some(res) = self.manifest.images.get(id).cloned() {
            if self.images.contains_key(id) {
                return;
            }
            let raw =
                image_load::load_image(&self.dir, &res.path, res.alpha_image.as_deref(), res.alpha_grid.as_deref(), res.rows, res.cols);
            match raw {
                Some(raw) => {
                    let img = make_img(images, raw, res.rows, res.cols);
                    self.images.insert(id.to_string(), img);
                }
                None => warn!("missing image {id} ({})", res.path),
            }
            // Aliases share the same resource description.
            let aliases: Vec<String> = self
                .manifest
                .images
                .iter()
                .filter(|(k, v)| k.as_str() != id && v.path == res.path && !self.images.contains_key(*k))
                .map(|(k, _)| k.clone())
                .collect();
            for a in aliases {
                if let Some(img) = self.images.get(id).cloned() {
                    self.images.insert(a, img);
                }
            }
        } else if let Some(res) = self.manifest.sounds.get(id).cloned() {
            match self.dir.find_with_exts(&res.path, SOUND_EXTS).and_then(|p| std::fs::read(p).ok()) {
                Some(bytes) => {
                    let handle = audio.add(AudioSource { bytes: bytes.into() });
                    self.sounds.insert(id.to_string(), Sound { handle, volume: res.volume });
                }
                None => debug!("missing sound {id} ({})", res.path),
            }
        } else if let Some(res) = self.manifest.fonts.get(id).cloned() {
            match res.path {
                Some(path) => {
                    if let Some(font) = self.load_font(&path, images) {
                        self.fonts.insert(id.to_string(), font);
                    } else {
                        warn!("could not load font {id} ({path})");
                    }
                }
                None => self.make_program_font(id),
            }
        }
    }

    fn load_font(&mut self, path: &str, images: &mut Assets<Image>) -> Option<Font> {
        let text = self.dir.read_string(path)?;
        let desc = FontDesc::parse(&text).map_err(|e| warn!("{path}: {e}")).ok()?;
        let (dir, _) = crate::data::vfs::split_dir(path);
        let mut cache: HashMap<String, Handle<Image>> = HashMap::new();
        let mut textures = Vec::new();
        for layer in &desc.layers {
            let key = layer.image.to_ascii_lowercase();
            if let Some(h) = cache.get(&key) {
                textures.push(Some(h.clone()));
                continue;
            }
            let rel = crate::data::vfs::join(dir, &layer.image);
            let tex = image_load::load_image(&self.dir, &rel, None, None, 1, 1).map(|raw| images.add(to_bevy_image(&raw)));
            if let Some(t) = &tex {
                cache.insert(key, t.clone());
            }
            textures.push(tex);
        }
        Some(Font { desc: Arc::new(desc), textures, tags: Vec::new() })
    }

    /// `!program` fonts are variants of loaded fonts with layer tags enabled.
    fn make_program_font(&mut self, id: &str) {
        let (base, tags): (&str, &[&str]) = match id {
            "FONT_MAIN10OUTLINE" => ("FONT_MAIN10", &["Outline"]),
            "FONT_MAIN10OUTLINE2" => ("FONT_MAIN10", &["Outline2"]),
            "FONT_MAIN10OUTLINE3" => ("FONT_MAIN10", &["Outline3"]),
            "FONT_MAIN8OUTLINE" => ("FONT_MAIN8", &["Outline"]),
            // CircleShootApp::Init builds FONT_DIALOG as MAIN10 with the Shadow1 layer.
            "FONT_DIALOG" => ("FONT_MAIN10", &["Shadow1"]),
            _ => return,
        };
        if let Some(f) = self.fonts.get(base) {
            let f = f.with_tags(tags);
            self.fonts.insert(id.to_string(), f);
        }
    }

    fn load_music(&mut self, audio: &mut Assets<AudioSource>) {
        // zuma.mo3 (a compressed tracker module) cannot be decoded by Bevy;
        // its subsongs converted to music/zuma_N.ogg are used instead. Any
        // other audio file in music/ stands in for the main theme.
        let mut load = |rel: &str| {
            let bytes = std::fs::read(self.dir.find_with_exts(rel, SOUND_EXTS)?).ok()?;
            Some(audio.add(AudioSource { bytes: bytes.into() }))
        };
        let mut tracks: Vec<_> = (0..8).map(|i| load(&format!("music/zuma_{i}"))).collect();
        if tracks[0].is_none() {
            let other = self
                .dir
                .list_dir("music")
                .into_iter()
                .find(|(k, _)| SOUND_EXTS.iter().any(|e| k.ends_with(&format!(".{e}"))))
                .map(|(k, _)| k.to_string());
            tracks[0] = other.and_then(|k| load(&k));
        }
        self.music = tracks;
    }

    pub fn has_music(&self) -> bool {
        self.music.iter().any(Option::is_some)
    }

    pub fn img(&self, id: &str) -> Option<&Img> {
        self.images.get(id)
    }

    pub fn font(&self, id: &str) -> &Font {
        self.fonts.get(id).or_else(|| self.fonts.get("FONT_MAIN10")).or_else(|| self.fonts.values().next()).expect("no fonts loaded")
    }

    pub fn has_font(&self) -> bool {
        !self.fonts.is_empty()
    }

    /// Loads (and caches) an image that is not in the manifest, e.g. level art.
    pub fn file_image(&mut self, rel: &str, images: &mut Assets<Image>) -> Option<Img> {
        let key = rel.to_ascii_lowercase();
        if let Some(i) = self.level_images.get(&key) {
            return i.clone();
        }
        let img = image_load::load_image(&self.dir, rel, None, None, 1, 1).map(|raw| make_img(images, raw, 1, 1));
        self.level_images.insert(key, img.clone());
        img
    }
}
