//! `OptionsDialog`: sound and music volume, full screen.

use crate::assets::GameAssets;
use crate::audio::Sfx;
use crate::gfx::Gfx;
use crate::input::Mouse;
use crate::profile::Profile;
use crate::ui::{dialog_box, dialog_button, slider};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode};

#[derive(Resource, Default)]
pub struct OptionsDialog {
    pub open: bool,
}

impl OptionsDialog {
    pub fn draw(
        &mut self,
        g: &mut Gfx,
        a: &GameAssets,
        m: &mut Mouse,
        sfx: &mut Sfx,
        profile: &mut Profile,
        windows: &mut Query<&mut Window>,
    ) {
        let (x, y, w, h) = (170.0, 110.0, 300.0, 270.0);
        dialog_box(g, a, x, y, w, h, "OPTIONS");
        let f = a.font("FONT_MAIN12");
        let col = Color::srgb(1.0, 0.9, 0.6);
        g.text(f, "Sound", x + 40.0, y + 100.0, col);
        let v = slider(g, a, m, x + 80.0, y + 82.0, profile.sound_volume);
        if (v - profile.sound_volume).abs() > 0.001 {
            profile.sound_volume = v;
            if m.left_released {
                sfx.play("SOUND_BUTTON2");
            }
        }
        g.text(f, "Music", x + 40.0, y + 145.0, col);
        let v = slider(g, a, m, x + 80.0, y + 127.0, profile.music_volume);
        if (v - profile.music_volume).abs() > 0.001 {
            profile.music_volume = v;
        }
        if !a.has_music() {
            let small = a.font("FONT_MAIN8");
            g.text(small, "(run tools/convert_music.sh to enable)", x + 40.0, y + 170.0, Color::srgb(0.8, 0.7, 0.5));
        }
        // Full screen checkbox.
        let (cx, cy) = (x + 40.0, y + 182.0);
        if let Some(cb) = a.img("IMAGE_DIALOG_CHECKBOX") {
            let cw = (cb.w / 2.0).floor();
            let cell = if profile.fullscreen { 1.0 } else { 0.0 };
            g.sub(cb, 0, Rect::new(cell * cw, 0.0, cell * cw + cw, cb.h), cx, cy, Color::WHITE);
            if m.clicked_in(cx, cy, cw + 120.0, cb.h) && !m.consumed {
                m.consumed = true;
                profile.fullscreen = !profile.fullscreen;
                sfx.play("SOUND_BUTTON1");
                for mut win in windows.iter_mut() {
                    win.mode =
                        if profile.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
                }
            }
            g.text(f, "Full Screen", cx + cw + 8.0, cy + cb.h * 0.5 + 5.0, col);
        }
        if dialog_button(g, a, m, sfx, "DONE", x + 90.0, y + h - 70.0, 120.0) {
            self.open = false;
            profile.save();
        }
    }
}
