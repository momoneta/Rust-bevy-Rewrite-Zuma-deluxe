//! Case-insensitive view of the original "Zuma Deluxe" folder.
//!
//! The original game ran on Windows, where file names are case-insensitive
//! (`levels.xml` refers to `Spiral.jpg` as `spiral`, fonts reference
//! `'CancunOutline10'` while the file is `_CancunOUTLINE10.gif`, ...).
//! We index every file once and resolve lookups through lowercase keys.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Extensions tried, in order, when a resource path has no extension
/// (same order as `SexyAppFramework::ImageLib::GetImage`).
pub const IMAGE_EXTS: &[&str] = &["png", "tga", "jpg", "jpeg", "gif"];
pub const SOUND_EXTS: &[&str] = &["ogg", "wav", "mp3"];

#[derive(Debug, Clone)]
pub struct DataDir {
    #[allow(dead_code)]
    pub root: PathBuf,
    index: HashMap<String, PathBuf>,
}

impl DataDir {
    pub fn open(root: &Path) -> std::io::Result<Self> {
        let mut index = HashMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(root) {
                    index.insert(normalize(&rel.to_string_lossy()), path.clone());
                }
            }
        }
        Ok(Self { root: root.to_path_buf(), index })
    }

    /// Exact (case-insensitive) lookup of a relative path.
    pub fn find(&self, rel: &str) -> Option<&Path> {
        self.index.get(&normalize(rel)).map(|p| p.as_path())
    }

    /// Lookup that tries `rel` as-is first and then with each extension appended.
    pub fn find_with_exts(&self, rel: &str, exts: &[&str]) -> Option<&Path> {
        if has_extension(rel, exts) {
            if let Some(p) = self.find(rel) {
                return Some(p);
            }
        }
        exts.iter().find_map(|ext| self.find(&format!("{rel}.{ext}")))
    }

    pub fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.find(rel)?).ok()
    }

    pub fn read_string(&self, rel: &str) -> Option<String> {
        let bytes = self.read(rel)?;
        // The game data is plain ASCII / Latin-1.
        Some(bytes.iter().map(|&b| b as char).collect())
    }

    /// All indexed files below `dir` (case-insensitive prefix match).
    pub fn list_dir(&self, dir: &str) -> Vec<(&str, &Path)> {
        let prefix = format!("{}/", normalize(dir));
        let mut out: Vec<_> = self.index.iter().filter(|(k, _)| k.starts_with(&prefix)).map(|(k, v)| (k.as_str(), v.as_path())).collect();
        out.sort_by(|a, b| a.0.cmp(b.0));
        out
    }
}

pub fn normalize(rel: &str) -> String {
    let s = rel.replace('\\', "/").to_ascii_lowercase();
    let mut parts: Vec<&str> = Vec::new();
    for part in s.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

fn has_extension(rel: &str, exts: &[&str]) -> bool {
    let lower = rel.to_ascii_lowercase();
    exts.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Splits `dir/name` into (`dir`, `name`).
pub fn split_dir(rel: &str) -> (&str, &str) {
    match rel.rfind(['/', '\\']) {
        Some(i) => (&rel[..i], &rel[i + 1..]),
        None => ("", rel),
    }
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// Finds the folder holding the original game data.
///
/// Search order: `--data <dir>` argument, `ZUMA_DATA` environment variable,
/// then a few conventional folder names next to the working directory and the
/// executable.
pub fn locate_data_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--data" {
            if let Some(p) = args.next() {
                candidates.push(PathBuf::from(p));
            }
        } else if let Some(p) = a.strip_prefix("--data=") {
            candidates.push(PathBuf::from(p));
        }
    }
    if let Ok(p) = std::env::var("ZUMA_DATA") {
        candidates.push(PathBuf::from(p));
    }
    let names = ["data", "Zuma Deluxe", "assets/Zuma Deluxe", "assets/data", "assets", "zuma"];
    let mut bases = vec![PathBuf::from(".")];
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        bases.push(dir.clone());
        if let Some(up) = dir.parent().and_then(Path::parent) {
            // target/debug/zuma -> project root
            bases.push(up.to_path_buf());
        }
    }
    for base in &bases {
        for name in names {
            candidates.push(base.join(name));
        }
    }
    candidates.into_iter().find(|c| looks_like_zuma(c))
}

fn looks_like_zuma(dir: &Path) -> bool {
    let Ok(read) = std::fs::read_dir(dir) else { return false };
    let mut has_levels = false;
    let mut has_props = false;
    for e in read.flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        has_levels |= name == "levels";
        has_props |= name == "properties";
    }
    has_levels && has_props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize("Images\\BaBallBlue.GIF"), "images/baballblue.gif");
        assert_eq!(normalize("./levels/../levels/spiral/"), "levels/spiral");
        assert_eq!(split_dir("images/_ballalpha"), ("images", "_ballalpha"));
    }
}
