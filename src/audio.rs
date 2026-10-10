//! Sound effects and music.

use crate::assets::GameAssets;
use crate::game::SoundReq;
use crate::profile::Profile;
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

/// Queue of sounds to start this frame.
#[derive(Resource, Default)]
pub struct Sfx {
    queue: Vec<SoundReq>,
}

impl Sfx {
    pub fn play(&mut self, id: &'static str) {
        self.queue.push(SoundReq::new(id));
    }

    pub fn extend(&mut self, reqs: impl IntoIterator<Item = SoundReq>) {
        self.queue.extend(reqs);
    }
}

#[derive(Component)]
pub struct Music;

pub fn play_sounds(mut commands: Commands, mut sfx: ResMut<Sfx>, assets: Option<Res<GameAssets>>, profile: Res<Profile>) {
    let reqs = std::mem::take(&mut sfx.queue);
    let Some(assets) = assets else { return };
    let mut started: Vec<&str> = Vec::new();
    for r in reqs {
        // The original never plays the same sample twice in one frame.
        if started.contains(&r.id) {
            continue;
        }
        let Some(s) = assets.sounds.get(r.id) else { continue };
        started.push(r.id);
        let vol = s.volume * r.volume * profile.sound_volume;
        if vol <= 0.0 {
            continue;
        }
        let speed = 2f32.powf(r.pitch / 12.0);
        commands.spawn((
            AudioPlayer::new(s.handle.clone()),
            PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(vol), speed, ..PlaybackSettings::DESPAWN },
        ));
    }
}

/// Music tracks (subsongs of the original zuma.mo3, see
/// `tools/convert_music.sh`). The original picks them by module order
/// position: 0 main theme, 36 danger, 38/39 jingles... (`FUN_00511100`).
pub mod track {
    pub const MAIN: usize = 0;
    pub const STAGE_COMPLETE: usize = 3;
    pub const GAME_OVER: usize = 4;
    pub const DANGER: usize = 5;
    pub const LIFE_LOST: usize = 6;
    pub const LEVEL_COMPLETE: usize = 7;
}

/// Which track the current screen wants.
#[derive(Resource, Default)]
pub struct MusicCtl {
    pub want: Option<usize>,
    playing: Option<usize>,
}

impl MusicCtl {
    pub fn play(&mut self, track: usize) {
        self.want = Some(track);
    }
}

/// Switches tracks when the wanted one changes and applies the volume setting.
pub fn update_music(
    mut commands: Commands,
    assets: Option<Res<GameAssets>>,
    profile: Res<Profile>,
    mut ctl: ResMut<MusicCtl>,
    mut sinks: Query<&mut AudioSink, With<Music>>,
    playing: Query<Entity, With<Music>>,
) {
    let Some(assets) = assets else { return };
    if ctl.want != ctl.playing {
        for e in &playing {
            commands.entity(e).despawn();
        }
        ctl.playing = ctl.want;
        let handle = ctl.want.and_then(|t| assets.music.get(t).cloned().flatten());
        if let (Some(t), Some(h)) = (ctl.want, handle) {
            let looping = matches!(t, track::MAIN | track::DANGER | 1 | 2);
            let base = if looping { PlaybackSettings::LOOP } else { PlaybackSettings::DESPAWN };
            commands.spawn((AudioPlayer::new(h), PlaybackSettings { volume: Volume::Linear(profile.music_volume), ..base }, Music));
        }
        return;
    }
    if profile.is_changed() {
        for mut sink in &mut sinks {
            sink.set_volume(Volume::Linear(profile.music_volume));
        }
    }
}
