//! `PracticeScreen` (Gauntlet): pick an unlocked level and play it in
//! Practice (rounds of rising difficulty) or Survival (random levels).
//!
//! Positions from `PracticeScreen::Draw` and `PracticeScreen::Resize`.

use super::{Mode, Screen, Session};
use crate::assets::GameAssets;
use crate::audio::Sfx;
use crate::gfx::Gfx;
use crate::input::Mouse;
use crate::profile::Profile;
use crate::ui::image_button;
use bevy::prelude::*;

/// Rank doors (Rabbit, Eagle, Jaguar, Sun God) with the score needed.
const RANKS: [(&str, f32, f32, i32); 4] = [
    ("IMAGE_GAUNTDOOR1", 209.0, 179.0, 0),
    ("IMAGE_GAUNTDOOR2", 232.0, 129.0, 25_000),
    ("IMAGE_GAUNTDOOR3", 249.0, 82.0, 60_000),
    ("IMAGE_GAUNTDOOR4", 265.0, 28.0, 120_000),
];

#[derive(Resource)]
pub struct GauntletState {
    selected: usize,
    survival: bool,
    ticks: u32,
}

pub fn enter(mut commands: Commands, assets: Res<GameAssets>, profile: Res<Profile>) {
    let selected = assets.levels.gauntlet.iter().position(|g| profile.is_unlocked(&g.graphics)).unwrap_or(0);
    commands.insert_resource(GauntletState { selected, survival: false, ticks: 0 });
}

fn unlocked(a: &GameAssets, p: &Profile, i: usize) -> bool {
    // The first level is always available so Gauntlet is never empty.
    i == 0 || a.levels.gauntlet.get(i).is_some_and(|g| p.is_unlocked(&g.graphics))
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut st: ResMut<GauntletState>,
    mut assets: ResMut<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut g: ResMut<Gfx>,
    mut mouse: ResMut<Mouse>,
    mut sfx: ResMut<Sfx>,
    mut next: ResMut<NextState<Screen>>,
    mut session: ResMut<Session>,
    profile: Res<Profile>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    st.ticks += 1;
    let n = assets.levels.gauntlet.len();
    if n == 0 {
        next.set(Screen::MainMenu);
        return;
    }
    let graphics = assets.levels.gauntlet[st.selected.min(n - 1)].graphics.clone();
    let thumb = assets.file_image(&format!("levels/perm_thumbnails/{graphics}"), &mut images);
    let a = &*assets;

    if let Some(sky) = a.img("IMAGE_GAUNTSKY") {
        let off = ((st.ticks / 4) as f32) % sky.w;
        g.image(sky, off - sky.w, 0.0);
        g.image(sky, off, 0.0);
        g.image(sky, off + sky.w, 0.0);
    }
    if let Some(t) = &thumb {
        g.image(t, 170.0, 220.0);
    }
    for (id, x, y) in [("IMAGE_GAUNTSCREEN", 0.0, 0.0), ("IMAGE_GAUNTTITLE", 0.0, 0.0), ("IMAGE_GAUNTHIGHSCORE", 434.0, 0.0)] {
        if let Some(img) = a.img(id) {
            g.image(img, x, y);
        }
    }
    let key = if st.survival { "survival".to_string() } else { graphics.to_ascii_lowercase() };
    let best = profile.gauntlet_best.get(&key).copied().unwrap_or(0);
    g.text_centered(a.font("FONT_MAIN10OUTLINE"), &format!("BEST {best}"), 536.0, 84.0, Color::srgb(1.0, 1.0, 0.6));

    for (id, x, y, need) in RANKS {
        if let Some(img) = a.img(id) {
            let w = (img.w / 4.0).floor();
            let cell = if best >= need && best > 0 { 2.0 } else { 0.0 };
            g.sub(img, 0, Rect::new(cell * w, 0.0, cell * w + w, img.h), x, y, Color::WHITE);
        }
    }

    let name = a.levels.graphics.get(&graphics).map(|g| g.display_name.clone()).unwrap_or_default();
    let is_unlocked = unlocked(a, &profile, st.selected);
    g.text_centered(a.font("FONT_MAIN10OUTLINE"), &name, 326.0, 236.0, Color::WHITE);
    if !is_unlocked {
        g.fill_rect(176.0, 240.0, 304.0, 196.0, Color::srgba(0.0, 0.0, 0.0, 0.6));
        g.text_wrapped(
            a.font("FONT_MAIN10"),
            "YOU MUST FIRST UNLOCK THIS LEVEL IN ADVENTURE MODE.",
            200.0,
            320.0,
            256.0,
            Color::srgb(1.0, 0.9, 0.6),
            true,
        );
    }
    let desc = if st.survival {
        "HOW LONG CAN YOU SURVIVE AN ENDLESS STREAM OF ZUMAIC INSANITY?"
    } else {
        "PLAY THIS LEVEL AGAIN AND AGAIN WITH RISING DIFFICULTY."
    };
    if is_unlocked {
        g.fill_rect(176.0, 404.0, 304.0, 32.0, Color::srgba(0.0, 0.0, 0.0, 0.55));
        g.text_wrapped(a.font("FONT_MAIN8"), desc, 186.0, 406.0, 284.0, Color::srgb(1.0, 0.95, 0.7), true);
    }

    // Buttons with four cells (normal, over, down, disabled).
    let four = |g: &mut Gfx, m: &mut Mouse, sfx: &mut Sfx, id: &str, x: f32, y: f32, enabled: bool| -> bool {
        let Some(img) = a.img(id) else { return false };
        let w = (img.w / 4.0).floor();
        let over = enabled && m.over(x, y, w, img.h);
        let cell = if !enabled {
            3.0
        } else if over && m.left_down {
            2.0
        } else if over {
            1.0
        } else {
            0.0
        };
        g.sub(img, 0, Rect::new(cell * w, 0.0, cell * w + w, img.h), x, y, Color::WHITE);
        if over && m.left_released && !m.consumed {
            m.consumed = true;
            sfx.play("SOUND_BUTTON1");
            return true;
        }
        false
    };
    if four(&mut g, &mut mouse, &mut sfx, "IMAGE_GAUNTBACKBUTTON", 177.0, 444.0, true) || keys.just_pressed(KeyCode::ArrowLeft) {
        st.selected = (st.selected + n - 1) % n;
    }
    if four(&mut g, &mut mouse, &mut sfx, "IMAGE_GAUNTNEXTBUTTON", 390.0, 444.0, true) || keys.just_pressed(KeyCode::ArrowRight) {
        st.selected = (st.selected + 1) % n;
    }
    if four(&mut g, &mut mouse, &mut sfx, "IMAGE_GAUNTPLAYBUTTON", 277.0, 444.0, is_unlocked)
        || (is_unlocked && keys.just_pressed(KeyCode::Enter))
    {
        *session = Session {
            mode: if st.survival { Mode::Survival } else { Mode::Practice { graphics: graphics.clone() } },
            graphics: graphics.clone(),
            ..Session::default()
        };
        sfx.play("SOUND_GAME_START");
        next.set(Screen::Playing);
    }
    // Mode toggles: the active one is drawn pressed.
    for (id, x, y, surv) in [("IMAGE_GAUNTPRACTICEBUTTON", 506.0, 413.0, false), ("IMAGE_GAUNTSURVIVALBUTTON", 505.0, 446.0, true)] {
        if let Some(img) = a.img(id) {
            let w = (img.w / 3.0).floor();
            let over = mouse.over(x, y, w, img.h);
            let cell = if st.survival == surv {
                2.0
            } else if over {
                1.0
            } else {
                0.0
            };
            g.sub(img, 0, Rect::new(cell * w, 0.0, cell * w + w, img.h), x, y, Color::WHITE);
            if over && mouse.left_released && !mouse.consumed {
                mouse.consumed = true;
                st.survival = surv;
                sfx.play("SOUND_BUTTON1");
            }
        }
    }
    if image_button(&mut g, a, &mut mouse, &mut sfx, "IMAGE_GAUNTMAINMENUBUTTON", 10.0, 422.0) || keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::MainMenu);
    }
}
