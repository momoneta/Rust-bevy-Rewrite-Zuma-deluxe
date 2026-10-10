//! Image loading with the PopCap alpha conventions.
//!
//! `SexyAppFramework` stores colour and alpha in separate files:
//! * `foo.jpg` + `_foo.gif`  (or `foo_.gif`): the second file's grey level is the alpha.
//! * only `_foo.gif` present: a white image with that alpha (used by fonts, glows).
//! * `alphaimage="X"` in resources.xml names the alpha file explicitly.
//! * `alphagrid="X"` names one alpha cell that is repeated over every cell of
//!   a `rows`x`cols` strip (the rolling ball strips use one shared ball mask).
//! * GIF transparency is honoured as well.

use super::vfs::{DataDir, IMAGE_EXTS, join, split_dir};
use image::RgbaImage;

fn decode(dir: &DataDir, rel: &str) -> Option<RgbaImage> {
    let path = dir.find_with_exts(rel, IMAGE_EXTS)?;
    let bytes = std::fs::read(path).ok()?;
    match image::load_from_memory(&bytes) {
        Ok(img) => Some(img.to_rgba8()),
        Err(err) => {
            bevy::log::warn!("could not decode {}: {err}", path.display());
            None
        }
    }
}

/// Loads `rel` (relative to the data folder, extension optional).
pub fn load_image(
    dir: &DataDir,
    rel: &str,
    alpha_image: Option<&str>,
    alpha_grid: Option<&str>,
    rows: u32,
    cols: u32,
) -> Option<RgbaImage> {
    let color = decode(dir, rel);
    let (folder, name) = split_dir(rel);

    let alpha = if let Some(a) = alpha_image {
        decode(dir, a)
    } else if alpha_grid.is_none() {
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
        decode(dir, &join(folder, &format!("_{stem}"))).or_else(|| decode(dir, &join(folder, &format!("{stem}_"))))
    } else {
        None
    };

    let mut img = match (color, &alpha) {
        (Some(c), _) => c,
        (None, Some(a)) => {
            // Alpha-only resource: white with that alpha.
            let mut white = RgbaImage::from_pixel(a.width(), a.height(), image::Rgba([255, 255, 255, 255]));
            apply_alpha(&mut white, a, 0, 0);
            return Some(white);
        }
        (None, None) => return None,
    };

    if let Some(a) = &alpha {
        apply_alpha(&mut img, a, 0, 0);
    }
    if let Some(grid) = alpha_grid.and_then(|g| decode(dir, g)) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let cw = img.width() / cols;
        let ch = img.height() / rows;
        for r in 0..rows {
            for c in 0..cols {
                apply_alpha(&mut img, &grid, c * cw, r * ch);
            }
        }
    }
    Some(img)
}

/// Multiplies the alpha of `img` (at offset) with the grey level of `alpha`.
fn apply_alpha(img: &mut RgbaImage, alpha: &RgbaImage, ox: u32, oy: u32) {
    let w = alpha.width().min(img.width().saturating_sub(ox));
    let h = alpha.height().min(img.height().saturating_sub(oy));
    for y in 0..h {
        for x in 0..w {
            let a = alpha.get_pixel(x, y);
            // PopCap reads the low byte (blue) of the 0xAARRGGBB pixel; grey
            // alpha files have identical channels anyway.  A transparent GIF
            // index in the alpha file means fully transparent.
            let v = if a[3] == 0 { 0 } else { a[2] };
            let p = img.get_pixel_mut(x + ox, y + oy);
            p[3] = ((p[3] as u16 * v as u16) / 255) as u8;
        }
    }
}

/// Splits a strip into `rows`x`cols` cells (row-major).
pub fn split_cells(img: &RgbaImage, rows: u32, cols: u32) -> Vec<RgbaImage> {
    let rows = rows.max(1);
    let cols = cols.max(1);
    let cw = img.width() / cols;
    let ch = img.height() / rows;
    let mut out = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        for c in 0..cols {
            out.push(image::imageops::crop_imm(img, c * cw, r * ch, cw, ch).to_image());
        }
    }
    out
}
