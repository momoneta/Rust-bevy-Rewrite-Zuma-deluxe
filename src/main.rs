//! Zuma Deluxe rewritten in Rust with Bevy.
//!
//! The engine reads the data files of an original Zuma Deluxe installation
//! (images, levels, sounds, fonts); see README.md.

mod additive;
mod assets;
mod audio;
mod data;
mod debug;
mod game;
mod gfx;
mod input;
mod net;
mod profile;
mod screens;
mod ui;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode, WindowResolution};

/// The screen size is the size of the loading screen: 640x480 for the
/// original game, 1280x720 for widescreen mods.
fn detect_screen() {
    let Some(path) = data::vfs::locate_data_dir() else { return };
    let Ok(dir) = data::vfs::DataDir::open(&path) else { return };
    if let Some(img) = data::image_load::load_image(&dir, "images/loadingscreen", None, None, 1, 1) {
        gfx::set_screen(img.width(), img.height());
    }
}

fn main() {
    detect_screen();
    let scr = gfx::screen();
    // Window: the game resolution scaled to a comfortable size.
    let k = if scr.x <= 640.0 { 1.5 } else { 1.0 };
    let profile = profile::Profile::load();
    let mode = if profile.fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Zuma Deluxe (Rust + Bevy)".into(),
                        resolution: WindowResolution::new((scr.x * k) as u32, (scr.y * k) as u32),
                        mode,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_linear()),
        )
        .insert_resource(profile)
        .insert_resource(debug::DebugOptions::from_args())
        .init_resource::<gfx::Gfx>()
        .init_resource::<gfx::SpritePool>()
        .init_resource::<gfx::AdditivePool>()
        .add_plugins(bevy::sprite_render::Material2dPlugin::<additive::AdditiveMaterial>::default())
        .init_resource::<input::Mouse>()
        .init_resource::<audio::Sfx>()
        .init_resource::<audio::MusicCtl>()
        .add_plugins(screens::ScreensPlugin)
        .add_systems(Startup, gfx::setup_camera)
        .add_systems(Update, input::update_mouse.before(screens::ScreenSet))
        .add_systems(PostUpdate, (gfx::flush_gfx, audio::play_sounds, audio::update_music, debug::debug_frames))
        .run();
}
