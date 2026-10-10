//! Game screens (the original widgets: `LoadingScreen`, `MainMenu`,
//! `AdventureScreen`, `PracticeScreen`, `Board`...), one Bevy state each.

pub mod adventure;
pub mod gauntlet;
pub mod loading;
pub mod main_menu;
pub mod nodata;
pub mod options;
pub mod play;

use crate::assets::GameAssets;
use crate::data::vfs::{DataDir, locate_data_dir};
use crate::gfx::Gfx;
use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Boot,
    NoData,
    Loading,
    MainMenu,
    Adventure,
    Gauntlet,
    Playing,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScreenSet;

/// Which game is running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Adventure,
    /// One Gauntlet level played through its difficulty rounds.
    Practice {
        graphics: String,
    },
    /// Endless Gauntlet: random unlocked levels with rising difficulty.
    Survival,
}

#[derive(Resource, Clone, Debug)]
pub struct Session {
    pub mode: Mode,
    pub stage: usize,
    pub level: usize,
    pub round: usize,
    pub graphics: String,
    pub lives: i32,
    pub score: i32,
}

impl Default for Session {
    fn default() -> Self {
        Self { mode: Mode::Adventure, stage: 0, level: 0, round: 0, graphics: String::new(), lives: 3, score: 0 }
    }
}

impl Session {
    pub fn adventure(stage: usize) -> Self {
        Self { mode: Mode::Adventure, stage, ..default() }
    }
}

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Screen>()
            .init_resource::<Session>()
            .init_resource::<options::OptionsDialog>()
            .add_systems(Update, boot.run_if(in_state(Screen::Boot)).in_set(ScreenSet))
            .add_systems(OnEnter(Screen::NoData), nodata::enter)
            .add_systems(Update, loading::update.run_if(in_state(Screen::Loading)).in_set(ScreenSet))
            .add_systems(Update, main_menu::update.run_if(in_state(Screen::MainMenu)).in_set(ScreenSet))
            .add_systems(OnEnter(Screen::Adventure), adventure::enter)
            .add_systems(Update, adventure::update.run_if(in_state(Screen::Adventure)).in_set(ScreenSet))
            .add_systems(OnEnter(Screen::Gauntlet), gauntlet::enter)
            .add_systems(Update, gauntlet::update.run_if(in_state(Screen::Gauntlet)).in_set(ScreenSet))
            .add_systems(OnEnter(Screen::Playing), play::enter)
            .add_systems(OnExit(Screen::Playing), play::exit)
            .add_systems(Update, play::update.run_if(in_state(Screen::Playing)).in_set(ScreenSet));
    }
}

fn boot(
    mut commands: Commands,
    mut next: ResMut<NextState<Screen>>,
    mut images: ResMut<Assets<Image>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut gfx: ResMut<Gfx>,
) {
    let Some(path) = locate_data_dir() else {
        next.set(Screen::NoData);
        return;
    };
    info!("using game data from {}", path.display());
    let dir = match DataDir::open(&path) {
        Ok(d) => d,
        Err(e) => {
            error!("cannot read {}: {e}", path.display());
            next.set(Screen::NoData);
            return;
        }
    };
    match GameAssets::new(dir, &mut images) {
        Ok(mut assets) => {
            assets.load_group_now("Init", &mut images, &mut audio);
            gfx.set_white(assets.white.clone());
            commands.insert_resource(assets);
            next.set(Screen::Loading);
        }
        Err(e) => {
            error!("{e}");
            next.set(Screen::NoData);
        }
    }
}
