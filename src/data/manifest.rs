//! `properties/resources.xml` (the PopCap `ResourceManager` manifest).

use super::vfs::join;
use super::xml;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ImageRes {
    pub path: String,
    pub alpha_image: Option<String>,
    pub alpha_grid: Option<String>,
    pub rows: u32,
    pub cols: u32,
}

#[derive(Debug, Clone)]
pub struct SoundRes {
    pub path: String,
    pub volume: f32,
}

#[derive(Debug, Clone)]
pub struct FontRes {
    /// `None` for `!program` fonts that the game builds in code.
    pub path: Option<String>,
}

#[derive(Debug, Default)]
pub struct Manifest {
    pub images: HashMap<String, ImageRes>,
    pub sounds: HashMap<String, SoundRes>,
    pub fonts: HashMap<String, FontRes>,
    /// Resource group name -> ids, in declaration order.
    pub groups: Vec<(String, Vec<String>)>,
}

impl Manifest {
    pub fn parse(text: &str) -> Self {
        let doc = xml::parse(text);
        let mut m = Manifest::default();
        let Some(root) = doc.children.iter().find(|e| e.is("ResourceManifest")) else {
            return m;
        };
        for group in root.children.iter().filter(|e| e.is("Resources")) {
            let gname = group.attr("id").unwrap_or_default().to_string();
            let mut ids = Vec::new();
            let mut def_path = String::new();
            let mut def_prefix = String::new();
            for el in &group.children {
                if el.is("SetDefaults") {
                    def_path = el.attr("path").unwrap_or_default().to_string();
                    def_prefix = el.attr("idprefix").unwrap_or_default().to_string();
                    continue;
                }
                let Some(id) = el.attr("id") else { continue };
                let id = format!("{def_prefix}{id}");
                let path = el.attr("path").unwrap_or_default();
                let full = |p: &str| join(&def_path, p);
                if el.is("Image") {
                    m.images.insert(
                        id.clone(),
                        ImageRes {
                            path: full(path),
                            alpha_image: el.attr("alphaimage").map(full),
                            alpha_grid: el.attr("alphagrid").map(full),
                            rows: el.attr_i32("rows").unwrap_or(1).max(1) as u32,
                            cols: el.attr_i32("cols").unwrap_or(1).max(1) as u32,
                        },
                    );
                    if let Some(alias) = el.attr("alias") {
                        let alias = format!("{def_prefix}{alias}");
                        let res = m.images[&id].clone();
                        m.images.insert(alias, res);
                    }
                } else if el.is("Sound") {
                    m.sounds.insert(id.clone(), SoundRes { path: full(path), volume: el.attr_f32("volume").unwrap_or(1.0) });
                } else if el.is("Font") {
                    let path = if path.starts_with('!') { None } else { Some(full(path)) };
                    m.fonts.insert(id.clone(), FontRes { path });
                } else {
                    continue;
                }
                ids.push(id);
            }
            m.groups.push((gname, ids));
        }
        m
    }
}
