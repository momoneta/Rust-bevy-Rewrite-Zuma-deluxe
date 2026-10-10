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

/// Starts the looping music (if a converted track exists) and applies the volume setting.
pub fn update_music(
    mut commands: Commands,
    assets: Option<Res<GameAssets>>,
    profile: Res<Profile>,
    mut sinks: Query<&mut AudioSink, With<Music>>,
    existing: Query<(), With<Music>>,
) {
    let Some(assets) = assets else { return };
    let Some(music) = &assets.music else { return };
    if existing.is_empty() {
        commands.spawn((
            AudioPlayer::new(music.clone()),
            PlaybackSettings { volume: Volume::Linear(profile.music_volume), ..PlaybackSettings::LOOP },
            Music,
        ));
        return;
    }
    if profile.is_changed() {
        for mut sink in &mut sinks {
            sink.set_volume(Volume::Linear(profile.music_volume));
        }
    }
}
