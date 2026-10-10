//! Immediate-mode widgets drawn with the original art
//! (`CircleButton`, `CircleDialog`, sliders, checkboxes).

use crate::assets::{GameAssets, Img};
use crate::audio::Sfx;
use crate::gfx::Gfx;
use crate::input::Mouse;
use bevy::prelude::*;

/// A three-state button image laid out horizontally (normal, over, down),
/// like `FUN_004f98c0(..., image, 3)` in the original.
pub fn image_button(g: &mut Gfx, a: &GameAssets, m: &mut Mouse, sfx: &mut Sfx, id: &str, x: f32, y: f32) -> bool {
    let Some(img) = a.img(id) else { return false };
    image_button_img(g, m, sfx, img, x, y, true)
}

pub fn image_button_img(g: &mut Gfx, m: &mut Mouse, sfx: &mut Sfx, img: &Img, x: f32, y: f32, enabled: bool) -> bool {
    let w = (img.w / 3.0).floor();
    let h = img.h;
    let over = enabled && m.over(x, y, w, h);
    let cell = if !over {
        0.0
    } else if m.left_down {
        2.0
    } else {
        1.0
    };
    let color = if enabled { Color::WHITE } else { Color::srgba(0.6, 0.6, 0.6, 1.0) };
    g.sub(img, 0, Rect::new(cell * w, 0.0, cell * w + w, h), x, y, color);
    if over && m.left_released && !m.consumed {
        m.consumed = true;
        sfx.play("SOUND_BUTTON1");
        return true;
    }
    false
}

/// Stretches `img` over a box keeping a border of `b` pixels (9-slice).
pub fn nine_slice(g: &mut Gfx, img: &Img, x: f32, y: f32, w: f32, h: f32, b: f32, color: Color) {
    let (iw, ih) = (img.w, img.h);
    let xs = [(0.0, b, x, b), (b, iw - b, x + b, w - 2.0 * b), (iw - b, iw, x + w - b, b)];
    let ys = [(0.0, b, y, b), (b, ih - b, y + b, h - 2.0 * b), (ih - b, ih, y + h - b, b)];
    for (sx0, sx1, dx, dw) in xs {
        for (sy0, sy1, dy, dh) in ys {
            if dw <= 0.0 || dh <= 0.0 {
                continue;
            }
            let src = Rect::new(sx0, sy0, sx1, sy1);
            g.sub_ex(img, 0, src, dx, dy, crate::gfx::DrawOpts { color, pivot: Some(Vec2::ZERO), scale: 1.0, ..default() });
            // Rescale the last command to the destination size.
            g.resize_last(Vec2::new(dw, dh));
        }
    }
}

/// `CircleDialog`: the stone dialog box with a title.
pub fn dialog_box(g: &mut Gfx, a: &GameAssets, x: f32, y: f32, w: f32, h: f32, title: &str) {
    g.fill_rect(0.0, 0.0, 640.0, 480.0, Color::srgba(0.0, 0.0, 0.0, 0.35));
    if let Some(back) = a.img("IMAGE_DIALOG_BACK") {
        nine_slice(g, back, x, y, w, h, 60.0, Color::WHITE);
    } else {
        g.fill_rect(x, y, w, h, Color::srgb(0.3, 0.25, 0.2));
    }
    let f = a.font("FONT_TITLE");
    g.text_centered(f, title, x + w * 0.5, y + 52.0, Color::srgb(1.0, 0.85, 0.4));
}

/// `DialogButton`: a horizontally stretched three-state button with a label.
pub fn dialog_button(g: &mut Gfx, a: &GameAssets, m: &mut Mouse, sfx: &mut Sfx, label: &str, x: f32, y: f32, w: f32) -> bool {
    let Some(img) = a.img("IMAGE_DIALOG_BUTTON") else { return false };
    let cw = (img.w / 3.0).floor();
    let h = img.h;
    let over = m.over(x, y, w, h);
    let cell = if !over {
        0.0
    } else if m.left_down {
        2.0
    } else {
        1.0
    };
    let b = 20.0;
    let parts = [(0.0, b, x, b), (b, cw - b, x + b, w - 2.0 * b), (cw - b, cw, x + w - b, b)];
    for (s0, s1, dx, dw) in parts {
        g.sub(img, 0, Rect::new(cell * cw + s0, 0.0, cell * cw + s1, h), dx, y, Color::WHITE);
        g.resize_last(Vec2::new(dw, h));
    }
    let f = a.font("FONT_MAIN12");
    let col = if over { Color::srgb(1.0, 1.0, 0.6) } else { Color::srgb(1.0, 0.9, 0.5) };
    g.text_centered(f, label, x + w * 0.5, y + h * 0.5 + f.ascent() * 0.5 - 1.0, col);
    if over && m.left_released && !m.consumed {
        m.consumed = true;
        sfx.play("SOUND_BUTTON1");
        return true;
    }
    false
}

/// A slider using `IMAGE_SLIDER_TRACK` / `IMAGE_SLIDER_THUMB`. Returns the new value.
pub fn slider(g: &mut Gfx, a: &GameAssets, m: &Mouse, x: f32, y: f32, value: f32) -> f32 {
    let (Some(track), Some(thumb)) = (a.img("IMAGE_SLIDER_TRACK"), a.img("IMAGE_SLIDER_THUMB")) else { return value };
    g.image(track, x, y);
    let range = track.w - thumb.w;
    let mut v = value;
    if m.left_down && m.over(x, y - 4.0, track.w, track.h + 8.0) {
        v = ((m.pos.x - x - thumb.w * 0.5) / range).clamp(0.0, 1.0);
    }
    g.image(thumb, x + v * range, y + (track.h - thumb.h) * 0.5);
    v
}
