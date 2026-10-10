//! Checks every original data file the game uses. Runs as a unit test when
//! the game folder is available (`./data` or `ZUMA_DATA`), and is skipped
//! otherwise so CI without the copyrighted data still passes.

#[cfg(test)]
mod tests {
    use crate::data::curve::CurvePath;
    use crate::data::font::FontDesc;
    use crate::data::image_load::load_image;
    use crate::data::levels::LevelDb;
    use crate::data::manifest::Manifest;
    use crate::data::vfs::{DataDir, SOUND_EXTS};
    use bevy::audio::{AudioSource, Decodable};
    use std::path::PathBuf;

    fn data_dir() -> Option<DataDir> {
        let p = std::env::var("ZUMA_DATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("data"));
        if !p.join("levels").exists() && !p.join("Levels").exists() {
            eprintln!("skipping: no game data at {}", p.display());
            return None;
        }
        DataDir::open(&p).ok()
    }

    #[test]
    fn all_levels_and_curves_load() {
        let Some(dir) = data_dir() else { return };
        let db = LevelDb::parse(&dir.read_string("levels/levels.xml").unwrap()).unwrap();
        assert_eq!(db.stages.len(), 13, "adventure stages");
        assert!(db.gauntlet.len() >= 20, "gauntlet levels");
        for (s, stage) in db.stages.iter().enumerate() {
            for l in 0..stage.len() {
                let level = db.adventure_level(s, l).unwrap_or_else(|| panic!("level {}-{}", s + 1, l + 1));
                assert!(!level.graphics.curves.is_empty());
                for c in &level.graphics.curves {
                    let rel = format!("levels/{}/{}.dat", level.graphics.id, c);
                    let data = dir.read(&rel).unwrap_or_else(|| panic!("missing {rel}"));
                    let path = CurvePath::parse(&data).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
                    assert!(path.len() > 1000, "{rel} has {} points", path.len());
                }
                if let Some(img) = &level.graphics.image {
                    let rel = format!("levels/{}/{img}", level.graphics.id);
                    assert!(load_image(&dir, &rel, None, None, 1, 1).is_some(), "missing {rel}");
                }
                for cut in &level.graphics.cutouts {
                    let rel = format!("levels/{}/{}", level.graphics.id, cut.image);
                    assert!(load_image(&dir, &rel, None, None, 1, 1).is_some(), "missing cutout {rel}");
                }
            }
        }
        for g in &db.gauntlet {
            assert!(!g.rounds.is_empty(), "{} has rounds", g.graphics);
            assert!(db.level(&g.graphics, &g.rounds[0].0, g.rounds[0].1).is_some());
        }
    }

    #[test]
    fn all_manifest_resources_load() {
        let Some(dir) = data_dir() else { return };
        let m = Manifest::parse(&dir.read_string("properties/resources.xml").unwrap());
        let groups = ["Init", "Register", "LoadingThread", "MainMenu", "AdventureScreen", "GauntletScreen", "HelpScreen"];
        let mut missing = Vec::new();
        for (name, ids) in &m.groups {
            if !groups.contains(&name.as_str()) {
                continue;
            }
            for id in ids {
                if let Some(r) = m.images.get(id) {
                    if load_image(&dir, &r.path, r.alpha_image.as_deref(), r.alpha_grid.as_deref(), r.rows, r.cols).is_none() {
                        missing.push(id.clone());
                    }
                } else if let Some(r) = m.sounds.get(id) {
                    match dir.find_with_exts(&r.path, SOUND_EXTS) {
                        Some(p) => {
                            let src = AudioSource { bytes: std::fs::read(p).unwrap().into() };
                            let samples = src.decoder().take(1000).count();
                            assert!(samples > 0, "{id} decodes to silence");
                        }
                        // A few samples referenced by the manifest are not shipped
                        // with this distribution (warning1, ufo1).
                        None => eprintln!("sound not shipped: {id}"),
                    }
                } else if let Some(r) = m.fonts.get(id) {
                    if let Some(path) = &r.path {
                        let text = dir.read_string(path).unwrap_or_else(|| panic!("missing font {path}"));
                        let f = FontDesc::parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
                        assert!(f.main_layer().is_some_and(|l| !l.glyphs.is_empty()), "{path} has glyphs");
                        assert!(f.string_width("0123") > 0, "{path} has digits");
                    }
                }
            }
        }
        assert!(missing.is_empty(), "images that failed to load: {missing:?}");
    }
}
