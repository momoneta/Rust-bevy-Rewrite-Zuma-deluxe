//! Command line options for testing:
//!
//! * `--level S-L`      start directly in adventure level S-L (1-based)
//! * `--screen NAME`    start in `menu`, `adventure` or `gauntlet`
//! * `--autoplay`       a simple bot aims and shoots
//! * `--screenshot F`   save a screenshot to F after `--frames N` frames (default 300), then quit
//! * `--frames N`       frame count for `--screenshot` / `--quit-after`
//! * `--quit-after N`   exit after N frames
//! * `--host`           open the co-op lobby and host (with `--level`, start there)
//! * `--join IP`        open the co-op lobby and join IP

use crate::screens::Screen;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use std::path::PathBuf;

#[derive(Resource, Default, Debug, Clone)]
pub struct DebugOptions {
    pub level: Option<(usize, usize)>,
    pub start_screen: Option<Screen>,
    pub autoplay: bool,
    pub screenshot: Option<PathBuf>,
    pub frames: u32,
    pub quit_after: Option<u32>,
    pub host: bool,
    pub join: Option<String>,
}

impl DebugOptions {
    pub fn from_args() -> Self {
        let mut d = DebugOptions { frames: 300, ..default() };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            let next = args.get(i + 1).cloned().unwrap_or_default();
            match args[i].as_str() {
                "--level" => {
                    if let Some((s, l)) = next.split_once('-') {
                        if let (Ok(s), Ok(l)) = (s.parse::<usize>(), l.parse::<usize>()) {
                            d.level = Some((s.max(1) - 1, l.max(1) - 1));
                        }
                    }
                    i += 1;
                }
                "--screen" => {
                    d.start_screen = match next.as_str() {
                        "menu" => Some(Screen::MainMenu),
                        "adventure" => Some(Screen::Adventure),
                        "gauntlet" => Some(Screen::Gauntlet),
                        _ => None,
                    };
                    i += 1;
                }
                "--autoplay" => d.autoplay = true,
                "--host" => {
                    d.host = true;
                    d.start_screen = Some(Screen::Lobby);
                }
                "--join" => {
                    d.join = Some(next);
                    d.start_screen = Some(Screen::Lobby);
                    i += 1;
                }
                "--screenshot" => {
                    d.screenshot = Some(PathBuf::from(next));
                    i += 1;
                }
                "--frames" => {
                    d.frames = next.parse().unwrap_or(300);
                    i += 1;
                }
                "--quit-after" => {
                    d.quit_after = next.parse().ok();
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        d
    }

    pub fn skip_menus(&self) -> bool {
        self.level.is_some() || self.start_screen.is_some()
    }
}

pub fn debug_frames(mut commands: Commands, mut frame: Local<u32>, opts: Res<DebugOptions>, mut exit: MessageWriter<AppExit>) {
    *frame += 1;
    if let Some(path) = &opts.screenshot {
        if *frame == opts.frames {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
        }
        if *frame == opts.frames + 30 {
            exit.write(AppExit::Success);
        }
    }
    if opts.quit_after == Some(*frame) {
        exit.write(AppExit::Success);
    }
}
