//! `MainMenu`: scrolling sky, the frog whose eyes follow the cursor, the
//! glowing sun and the five stone buttons.
//!
//! Button positions come from `MainMenu::AddedToManager` (0x00505e80), the eye
//! sockets from `FUN_005051a0`, the eye mask origin from the constructor.

use super::{Screen, Session, options::OptionsDialog};
use crate::assets::{GameAssets, Img, to_bevy_image};
use crate::audio::Sfx;
use crate::data::image_load;
use crate::gfx::{DrawOpts, Gfx};
use crate::input::Mouse;
use crate::profile::Profile;
use crate::ui::image_button;
use bevy::app::AppExit;
use bevy::prelude::*;

const EYE_MASK_POS: Vec2 = Vec2::new(191.0, 331.0);
/// Eye socket centres relative to the mask, and how far each eye can move.
const SOCKETS: [(Vec2, f32); 2] = [(Vec2::new(29.0, 25.0), 7.0), (Vec2::new(87.0, 16.0), 5.0)];

#[derive(Default)]
pub struct MenuState {
    ticks: u32,
    eye_mask: Option<Img>,
    tried_mask: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut st: Local<MenuState>,
    mut assets: ResMut<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut g: ResMut<Gfx>,
    mut mouse: ResMut<Mouse>,
    mut sfx: ResMut<Sfx>,
    mut next: ResMut<NextState<Screen>>,
    mut session: ResMut<Session>,
    mut options: ResMut<OptionsDialog>,
    mut profile: ResMut<Profile>,
    mut exit: MessageWriter<AppExit>,
    mut windows: Query<&mut Window>,
) {
    st.ticks += 1;
    if !st.tried_mask {
        st.tried_mask = true;
        st.eye_mask = make_eye_mask(&assets, &mut images);
    }
    let a = &mut *assets;
    if let Some(sky) = a.img("IMAGE_MM_SKY") {
        let off = ((st.ticks / 3) as f32) % sky.w;
        g.image(sky, off - sky.w, 0.0);
        g.image(sky, off, 0.0);
        g.image(sky, off + sky.w, 0.0);
    }
    if let Some(back) = a.img("IMAGE_MM_BACK") {
        g.image(back, 0.0, 0.0);
    }
    // Eyes follow the cursor.
    for (i, id) in ["IMAGE_MM_EYELEFT", "IMAGE_MM_EYERIGHT"].iter().enumerate() {
        if let Some(eye) = a.img(id) {
            let (socket, r) = SOCKETS[i];
            let c = EYE_MASK_POS + socket;
            let d = mouse.pos - c;
            let off = if d.length() > 0.1 { d.normalize() * (d.length() * 0.05).min(r) } else { Vec2::ZERO };
            let p = c + off;
            g.image(eye, (p.x - eye.w * 0.5).round(), (p.y - eye.h * 0.5).round());
        }
    }
    if let Some(mask) = &st.eye_mask {
        g.image(mask, EYE_MASK_POS.x, EYE_MASK_POS.y);
    }
    // Sun glow (pulsing, slowly rotating) and the sun.
    if let Some(glow) = a.img("IMAGE_SUNGLOW") {
        let t = (st.ticks % 500) as f32;
        let t = if t > 250.0 { 500.0 - t } else { t };
        let alpha = (t * 75.0 / 250.0 + 75.0) / 255.0;
        let rot = st.ticks as f32 * std::f32::consts::PI / 180.0 * 0.3;
        g.draw_ex(glow, 0, -69.0, -63.0, DrawOpts { rot, color: Color::srgba(1.0, 1.0, 0.0, alpha), ..default() });
    }
    if let Some(sun) = a.img("IMAGE_MM_SUN") {
        g.image(sun, 0.0, 2.0);
    }
    let title = a.font("FONT_BROWNTITLE");
    g.text_centered(title, "Welcome to Zuma!", 320.0, 22.0, Color::WHITE);

    if options.open {
        options.draw(&mut g, a, &mut mouse, &mut sfx, &mut profile, &mut windows);
        return;
    }

    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_MM_ARCADE", 452.0, 64.0) {
        *session = Session::adventure(profile.max_stage);
        next.set(Screen::Adventure);
    }
    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_MM_GAUNTLET", 436.0, 153.0) {
        next.set(Screen::Gauntlet);
    }
    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_MM_OPTIONS", 418.0, 236.0) {
        options.open = true;
    }
    // "More Games" opened PopCap's web site; it is kept for the look only.
    if let Some(img) = a.img("IMAGE_MM_MOREGAMES") {
        let w = (img.w / 3.0).floor();
        g.sub(img, 0, Rect::new(0.0, 0.0, w, img.h), 394.0, 305.0, Color::WHITE);
    }
    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_MM_QUIT", 496.0, 315.0) {
        profile.save();
        exit.write(AppExit::Success);
    }
}

/// `FUN_004f8740(MM_BACK, MM_EYEMASK, x, y)`: background pixels under the
/// eye mask, drawn over the eyeballs as eyelids.
fn make_eye_mask(a: &GameAssets, images: &mut Assets<Image>) -> Option<Img> {
    let back = a.manifest.images.get("IMAGE_MM_BACK")?;
    let mask = a.manifest.images.get("IMAGE_MM_EYEMASK")?;
    let back = image_load::load_image(&a.dir, &back.path, None, None, 1, 1)?;
    let mask = image_load::load_image(&a.dir, &mask.path, None, None, 1, 1)?;
    let (ox, oy) = (EYE_MASK_POS.x as u32, EYE_MASK_POS.y as u32);
    let mut out = image::RgbaImage::new(mask.width(), mask.height());
    for (x, y, p) in out.enumerate_pixels_mut() {
        if x + ox < back.width() && y + oy < back.height() {
            let s = back.get_pixel(x + ox, y + oy);
            *p = image::Rgba([s[0], s[1], s[2], mask.get_pixel(x, y)[3]]);
        }
    }
    let h = images.add(to_bevy_image(&out));
    Some(Img { cells: vec![h.clone()], full: h, w: out.width() as f32, h: out.height() as f32, rows: 1, cols: 1 })
}
