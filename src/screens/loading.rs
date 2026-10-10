//! `LoadingScreen`: loads the remaining resource groups a few at a time
//! while showing the loader bar, then waits for a click.

use super::Screen;
use crate::assets::GameAssets;
use crate::gfx::Gfx;
use crate::input::Mouse;
use bevy::prelude::*;
use std::time::Duration;

pub fn update(
    mut assets: ResMut<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut g: ResMut<Gfx>,
    mut sfx: ResMut<crate::audio::Sfx>,
    mouse: Res<Mouse>,
    mut next: ResMut<NextState<Screen>>,
    debug: Res<crate::debug::DebugOptions>,
) {
    let done = assets.load_some(&mut images, &mut audio, Duration::from_millis(25));
    if let Some(bg) = assets.img("IMAGE_LOADING_SCREEN") {
        g.image(bg, 0.0, 0.0);
    }
    if let Some(bar) = assets.img("IMAGE_LOADERBAR") {
        let w = (bar.w * assets.progress()).floor();
        if w > 0.0 {
            g.sub(bar, 0, Rect::new(0.0, 0.0, w, bar.h), 128.0, 349.0, Color::WHITE);
        }
    }
    if !done {
        return;
    }
    let text = "CLICK HERE TO PLAY!";
    if assets.has_font() {
        let f = assets.font("FONT_FLOAT");
        let over = mouse.over(128.0, 349.0, 399.0, 44.0);
        let col = if over { Color::srgb(1.0, 1.0, 1.0) } else { Color::srgb_u8(0xff, 0xec, 0x91) };
        g.text_centered(f, text, 320.0, 378.0, col);
    }
    if mouse.left_released || debug.skip_menus() {
        sfx.play("SOUND_BUTTON1");
        if debug.level.is_some() && debug.start_screen.is_none() {
            next.set(Screen::Playing);
        } else {
            next.set(debug.start_screen.unwrap_or(Screen::MainMenu));
        }
    }
}
