//! `AdventureScreen`: the four temples with their stage doors.
//!
//! Layout from the original: temple positions from `FUN_0050a490`,
//! `FUN_0050a5b0`, `FUN_0050a6d0`, `FUN_00507140`; door positions from the
//! constructor (0x005101c0); buttons from `AdventureScreen::Resize`.

use super::{Screen, Session};
use crate::assets::GameAssets;
use crate::audio::Sfx;
use crate::gfx::Gfx;
use crate::input::Mouse;
use crate::profile::Profile;
use crate::ui::image_button;
use bevy::prelude::*;

const DOORS: [(f32, f32); 12] = [
    (226.0, 300.0),
    (296.0, 270.0),
    (366.0, 300.0),
    (0.0, 164.0),
    (69.0, 159.0),
    (120.0, 155.0),
    (452.0, 154.0),
    (509.0, 154.0),
    (568.0, 154.0),
    (220.0, 35.0),
    (291.0, 35.0),
    (369.0, 35.0),
];
const DOOR_IMAGES: [&str; 12] = [
    "IMAGE_ADVDOOR1A",
    "IMAGE_ADVDOOR1B",
    "IMAGE_ADVDOOR1C",
    "IMAGE_ADVDOOR2A",
    "IMAGE_ADVDOOR2B",
    "IMAGE_ADVDOOR2C",
    "IMAGE_ADVDOOR3A",
    "IMAGE_ADVDOOR3B",
    "IMAGE_ADVDOOR3C",
    "IMAGE_ADVDOOR4A",
    "IMAGE_ADVDOOR4B",
    "IMAGE_ADVDOOR4C",
];
pub const TEMPLE_NAMES: [&str; 4] = ["TEMPLE OF ZUKULKAN", "QUETZAL QUATL", "POPO POYOLLI", "SECRET SHRINE OF ZUMA"];
const STAGE_TEXT: [&str; 13] = [
    "DO YOU DARE ENTER THE ANCIENT RUINS?",
    "THE 2ND LEVEL OF THE TEMPLE AWAITS.",
    "WILL YOU DESCEND TO THE THIRD LEVEL?",
    "YOU HAVE UNCOVERED A HIDDEN DOOR!",
    "THE GOLDEN SECRETS OF ZUMA AWAIT YOU.",
    "DO YOU DARE ENTER THE LAST CHAMBER?",
    "ANOTHER LOST TEMPLE...",
    "CAN YOU FULFILL THE PROPHECY?",
    "THE LAST STAGE (?)",
    "A RUINED TEMPLE RISES FROM THE EARTH.",
    "THE FINAL SECRET OF ZUMA BECKONS...",
    "YOU ENTER THE RUINS ONE LAST TIME...",
    "THE FINAL SECRET OF ZUMA BECKONS...",
];

#[derive(Resource, Default)]
pub struct AdventureState {
    pub selected: usize,
    ticks: u32,
}

pub fn enter(mut commands: Commands, profile: Res<Profile>, assets: Res<GameAssets>) {
    let max = profile.max_stage.min(assets.levels.stages.len().saturating_sub(1));
    commands.insert_resource(AdventureState { selected: max, ticks: 0 });
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut st: ResMut<AdventureState>,
    assets: Res<GameAssets>,
    mut g: ResMut<Gfx>,
    mut mouse: ResMut<Mouse>,
    mut sfx: ResMut<Sfx>,
    mut next: ResMut<NextState<Screen>>,
    mut session: ResMut<Session>,
    profile: Res<Profile>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    st.ticks += 1;
    let a = &*assets;
    let max = profile.max_stage.min(a.levels.stages.len().saturating_sub(1));
    if let Some(sky) = a.img("IMAGE_ADVSKY") {
        let off = ((st.ticks / 4) as f32) % sky.w;
        g.image(sky, off - sky.w, 0.0);
        g.image(sky, off, 0.0);
        g.image(sky, off + sky.w, 0.0);
    }
    for (id, x, y) in [("IMAGE_ADVBACK", 0.0, 0.0), ("IMAGE_ADVTITLE", 0.0, 0.0), ("IMAGE_ADVHIGHSCORE", 456.0, 0.0)] {
        if let Some(img) = a.img(id) {
            g.image(img, x, y);
        }
    }
    let outline = a.font("FONT_MAIN10OUTLINE");
    g.text_centered(outline, &format!("BEST {}", profile.adventure_best), 548.0, 84.0, Color::srgb(1.0, 1.0, 0.6));

    let mut hover = None;
    for (i, &(x, y)) in DOORS.iter().enumerate() {
        if let Some(img) = a.img(DOOR_IMAGES[i]) {
            let w = (img.w / 4.0).floor();
            if i <= max && mouse.over(x, y, w, img.h) {
                hover = Some(i);
            }
        }
    }
    let temple_open = |t: usize| max >= t * 3;
    let draw_doors = |g: &mut Gfx, range: std::ops::Range<usize>| {
        for i in range {
            let Some(img) = a.img(DOOR_IMAGES[i]) else { continue };
            let w = (img.w / 4.0).floor();
            let cell = if st.selected == i {
                3.0
            } else if hover == Some(i) {
                2.0
            } else if i <= max {
                1.0
            } else {
                0.0
            };
            let (x, y) = DOORS[i];
            g.sub(img, 0, Rect::new(cell * w, 0.0, cell * w + w, img.h), x, y, Color::WHITE);
        }
    };
    if temple_open(3) {
        if let Some(t) = a.img("IMAGE_ADVTEMPLE4") {
            g.image(t, 174.0, 1.0);
        }
        draw_doors(&mut g, 9..12);
        if let Some(m) = a.img("IMAGE_ADVMIDDLE") {
            g.image(m, 0.0, 72.0);
        }
    }
    let t2 = if temple_open(1) { "IMAGE_ADVTEMPLE2" } else { "IMAGE_ADVTEMPLE2V" };
    if let Some(t) = a.img(t2) {
        g.image(t, 0.0, 112.0);
    }
    if temple_open(1) {
        draw_doors(&mut g, 3..6);
    }
    let t3 = if temple_open(2) { "IMAGE_ADVTEMPLE3" } else { "IMAGE_ADVTEMPLE3V" };
    if let Some(t) = a.img(t3) {
        g.image(t, 395.0, 125.0);
    }
    if temple_open(2) {
        draw_doors(&mut g, 6..9);
    }
    if let Some(t) = a.img("IMAGE_ADVTEMPLE1") {
        g.image(t, 0.0, 188.0);
    }
    draw_doors(&mut g, 0..3);

    if let Some(i) = hover {
        if mouse.left_released && !mouse.consumed {
            mouse.consumed = true;
            st.selected = i;
            sfx.play("SOUND_BUTTON2");
        }
    }

    // Selected stage description.
    let sel = st.selected;
    if let Some(stage) = a.img("IMAGE_ADVSTAGE") {
        g.image(stage, 289.0, 382.0);
    }
    g.text(a.font("FONT_DIALOG"), &(sel + 1).to_string(), 344.0, 396.0, Color::srgb(1.0, 1.0, 0.6));
    let temple = TEMPLE_NAMES[(sel / 3).min(3)];
    g.text_centered(a.font("FONT_BROWNTITLE"), temple, 326.0, 425.0, Color::srgb(1.0, 0.85, 0.4));
    g.text_centered(a.font("FONT_MAIN10OUTLINE2"), STAGE_TEXT[sel.min(12)], 326.0, 456.0, Color::WHITE);

    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_ADVMAINMENUBUTTON", 0.0, 438.0) || keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::MainMenu);
    }
    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_ADVPLAYBUTTON", 542.0, 441.0) || keys.just_pressed(KeyCode::Enter) {
        *session = Session::adventure(sel);
        sfx.play("SOUND_GAME_START");
        next.set(Screen::Playing);
    }
}
