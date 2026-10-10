//! The playing screen: runs the [`Board`] at 100 ticks per second, draws it,
//! and handles level progression, lives and the in-game dialogs.

use super::{Mode, Screen, Session, options::OptionsDialog};
use crate::assets::GameAssets;
use crate::audio::{MusicCtl, Sfx, track};
use crate::data::curve::CurvePath;
use crate::data::levels::LevelDesc;
use crate::debug::DebugOptions;
use crate::game::board::{BoardInput, Outcome};
use crate::game::draw::{LevelGfx, draw_board, draw_lives};
use crate::game::{BALL_D, Board, BoardState};
use crate::gfx::Gfx;
use crate::input::Mouse;
use crate::net::Coop;
use crate::profile::Profile;
use crate::ui::{dialog_box, dialog_button};
use bevy::prelude::*;
use rand::Rng;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum PlayDialog {
    Menu,
    LevelComplete {
        points: i32,
        seconds: i32,
        par: i32,
        bonus: i32,
        max_combo: i32,
        stage_done: bool,
    },
    LifeLost,
    GameOver,
    Victory,
    /// Co-op: the other player left.
    Disconnected,
}

#[derive(Resource)]
pub struct Play {
    pub board: Board,
    pub lgfx: LevelGfx,
    pub dialog: Option<PlayDialog>,
    acc: f32,
    bot_cooldown: i32,
    /// Frames the current dialog has been open (used by `--autoplay`).
    dialog_frames: u32,
    /// Space bar pause (also set when the window loses focus).
    paused: bool,
    /// 't': show the time left to beat the ace time.
    show_time: bool,
}

/// Number of ball colours for a level. The level settings only set `colors`
/// for the first levels; the count grows with the stage / gauntlet
/// difficulty (approximation of the original progression).
fn colors_for(session: &Session, level: &LevelDesc) -> u8 {
    match session.mode {
        Mode::Adventure => match session.stage {
            0..=1 => 4,
            2..=5 => 5,
            _ => 6,
        },
        _ => match level.settings.difficulty {
            ..=6 => 4,
            7..=16 => 5,
            _ => 6,
        },
    }
}

fn load_paths(a: &GameAssets, level: &LevelDesc) -> Vec<Arc<CurvePath>> {
    let mut out = Vec::new();
    for name in &level.graphics.curves {
        let rel = format!("levels/{}/{}.dat", level.graphics.id, name);
        match a.dir.read(&rel).map(|d| CurvePath::parse(&d)) {
            Some(Ok(p)) if p.len() > 1 => out.push(Arc::new(p)),
            Some(Err(e)) => error!("{rel}: {e:?}"),
            _ => error!("missing curve {rel}"),
        }
    }
    out
}

/// Resolves the level for the current session position.
fn current_level(a: &GameAssets, s: &mut Session) -> Option<(LevelDesc, String)> {
    let db = &a.levels;
    match &s.mode {
        Mode::Adventure => {
            s.stage = s.stage.min(db.stages.len().saturating_sub(1));
            let l = db.adventure_level(s.stage, s.level)?;
            Some((l, format!("LEVEL {}-{}", s.stage + 1, s.level + 1)))
        }
        Mode::Practice { graphics } => {
            let g = db.gauntlet.iter().find(|g| g.graphics.eq_ignore_ascii_case(graphics))?;
            let (set, diff) = g.rounds.get(s.round.min(g.rounds.len().saturating_sub(1)))?.clone();
            let l = db.level(&g.graphics, &set, diff)?;
            Some((l, format!("ROUND {}", s.round + 1)))
        }
        Mode::Survival => {
            let g = db.gauntlet.iter().find(|g| g.graphics.eq_ignore_ascii_case(&s.graphics)).or(db.gauntlet.first())?;
            let (set, diff) = g.rounds.get(s.round.min(g.rounds.len().saturating_sub(1)))?.clone();
            let l = db.level(&g.graphics, &set, diff)?;
            Some((l, format!("ROUND {}", s.round + 1)))
        }
    }
}

fn build(a: &mut GameAssets, images: &mut Assets<Image>, s: &mut Session, coop: Option<&mut Coop>) -> Option<Play> {
    let (level, title) = current_level(a, s)?;
    let paths = load_paths(a, &level);
    if paths.is_empty() {
        return None;
    }
    let colors = colors_for(s, &level);
    s.graphics = level.graphics.id.clone();
    let board = match coop {
        Some(c) => {
            let seed = c.next_seed();
            c.reset_board();
            Board::with_players(level, paths, colors, s.score, title, 2, seed)
        }
        None => Board::new(level, paths, colors, s.score, title),
    };
    let lgfx = LevelGfx::load(a, images, &board);
    Some(Play { board, lgfx, dialog: None, acc: 0.0, bot_cooldown: 100, dialog_frames: 0, paused: false, show_time: false })
}

pub fn enter(
    mut commands: Commands,
    mut assets: ResMut<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut session: ResMut<Session>,
    debug: Res<DebugOptions>,
    mut next: ResMut<NextState<Screen>>,
    mut coop: Option<ResMut<Coop>>,
) {
    if let (Some((stage, level)), None) = (debug.level, &coop) {
        if session.score == 0 && session.stage == 0 && session.level == 0 {
            *session = Session { stage, level, ..Session::adventure(stage) };
        }
    }
    match build(&mut assets, &mut images, &mut session, coop.as_deref_mut()) {
        Some(play) => commands.insert_resource(play),
        None => {
            error!("could not start level {:?}", *session);
            next.set(Screen::MainMenu);
        }
    }
}

pub fn exit(mut commands: Commands) {
    commands.remove_resource::<Play>();
    // Leaving the game ends the co-op session (the link says goodbye).
    commands.remove_resource::<Coop>();
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    play: Option<ResMut<Play>>,
    mut assets: ResMut<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut g: ResMut<Gfx>,
    mut mouse: ResMut<Mouse>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut sfx: ResMut<Sfx>,
    mut session: ResMut<Session>,
    mut profile: ResMut<Profile>,
    mut next: ResMut<NextState<Screen>>,
    mut options: ResMut<OptionsDialog>,
    mut windows: Query<&mut Window>,
    debug: Res<DebugOptions>,
    mut music: ResMut<MusicCtl>,
    mut coop: Option<ResMut<Coop>>,
) {
    let Some(mut play) = play else { return };
    let mut coop = coop.as_deref_mut();
    let play = &mut *play;
    let a = &mut *assets;

    // Menu button (top right) and keyboard.
    let menu_hover = play.dialog.is_none() && {
        let (p, q) = (crate::gfx::hud(540.0, 3.0), crate::gfx::hud(619.0, 28.0));
        mouse.inside && mouse.pos.x >= p.x && mouse.pos.y >= p.y && mouse.pos.x < q.x && mouse.pos.y < q.y
    };
    if play.dialog.is_none() && play.board.state != BoardState::Done(Outcome::Won) {
        if (menu_hover && mouse.left_released) || keys.just_pressed(KeyCode::Escape) {
            play.dialog = Some(PlayDialog::Menu);
            mouse.consumed = true;
            sfx.play("SOUND_BUTTON1");
        }
    }

    // Space pauses ("Press space bar to pause the game."), 't' shows the
    // time left for the ace bonus; losing focus pauses as in the original.
    if play.dialog.is_none() && coop.is_none() {
        if keys.just_pressed(KeyCode::Space) {
            play.paused = !play.paused;
        }
        if keys.just_pressed(KeyCode::KeyT) {
            play.show_time = !play.show_time;
        }
        if !debug.autoplay && debug.screenshot.is_none() && windows.iter().any(|w| !w.focused) {
            play.paused = true;
        }
        if play.paused && mouse.left_pressed && !menu_hover {
            play.paused = false;
            mouse.consumed = true;
        }
    }

    // Simulation at 100 Hz. In co-op both machines step the same board in
    // lockstep; the in-game menu does not stop it.
    if let Some(c) = coop.as_deref_mut() {
        c.pump();
        if c.link.closed && !matches!(play.dialog, Some(PlayDialog::Disconnected)) {
            play.dialog = Some(PlayDialog::Disconnected);
        }
        if c.ctl.iter().any(|x| x == "menu") {
            record_scores(&session, &mut profile);
            next.set(Screen::MainMenu);
        }
        if matches!(play.dialog, None | Some(PlayDialog::Menu)) {
            let free = play.dialog.is_none() && !options.open;
            let mut input = BoardInput {
                aim: mouse.pos,
                fire: free && mouse.left_pressed && !menu_hover && !mouse.consumed,
                swap: free && mouse.right_pressed,
            };
            if debug.autoplay {
                bot_input(play, &mut input, c.me());
            }
            c.local_events(input.fire, input.swap);
            c.send_inputs(input.aim);
            play.acc = (play.acc + time.delta_secs() * crate::game::TICKS_PER_SEC).min(10.0);
            while play.acc >= 1.0 {
                let Some(both) = c.ready() else {
                    c.stalled += 1;
                    break;
                };
                play.acc -= 1.0;
                play.board.update(&both);
                let b = &play.board;
                c.advance(|| b.checksum());
                c.send_inputs(input.aim);
            }
        }
    } else if play.dialog.is_none() && !options.open && !play.paused {
        let mut input =
            BoardInput { aim: mouse.pos, fire: mouse.left_pressed && !menu_hover && !mouse.consumed, swap: mouse.right_pressed };
        if debug.autoplay {
            bot_input(play, &mut input, 0);
        }
        play.acc = (play.acc + time.delta_secs() * crate::game::TICKS_PER_SEC).min(10.0);
        while play.acc >= 1.0 {
            play.acc -= 1.0;
            play.board.update(&[input]);
            input.fire = false;
            input.swap = false;
        }
    }
    sfx.extend(play.board.sounds.drain(..));
    if play.board.extra_lives > 0 {
        session.lives += play.board.extra_lives;
        play.board.extra_lives = 0;
    }
    session.score = play.board.score;

    // Level finished?
    if play.dialog.is_none() {
        match play.board.state {
            BoardState::Done(Outcome::Won) => level_won(play, a, &mut session, &mut profile, &mut sfx),
            BoardState::Done(Outcome::Lost) => {
                session.lives -= 1;
                if session.lives > 0 {
                    play.dialog = Some(PlayDialog::LifeLost);
                } else {
                    sfx.play("SOUND_GAME_OVER");
                    record_scores(&session, &mut profile);
                    play.dialog = Some(PlayDialog::GameOver);
                }
            }
            _ => {}
        }
    }

    music.play(match &play.dialog {
        None | Some(PlayDialog::Menu) => {
            if play.board.danger && matches!(play.board.state, BoardState::Playing | BoardState::Lost(_)) {
                track::DANGER
            } else {
                track::MAIN
            }
        }
        Some(PlayDialog::LevelComplete { stage_done: true, .. }) | Some(PlayDialog::Victory) => track::STAGE_COMPLETE,
        Some(PlayDialog::LevelComplete { .. }) => track::LEVEL_COMPLETE,
        Some(PlayDialog::LifeLost) => track::LIFE_LOST,
        Some(PlayDialog::GameOver) | Some(PlayDialog::Disconnected) => track::GAME_OVER,
    });

    draw_board(&mut g, a, &play.lgfx, &play.board, mouse.pos, menu_hover);
    draw_lives(&mut g, a, session.lives);
    if let Some(c) = coop.as_deref() {
        draw_coop(&mut g, a, play, c);
    }
    if play.show_time && play.board.state == BoardState::Playing {
        // Board::DrawOverlay draws this at (580, 460) with MAIN10OUTLINE.
        let left = play.board.level.settings.par_time - play.board.seconds();
        let txt = if left < 0 { format!("-{}", fmt_time(-left)) } else { fmt_time(left) };
        let p = crate::gfx::hud(580.0, 460.0);
        g.with_raw(|g| g.text(a.font("FONT_MAIN10OUTLINE"), &txt, p.x, p.y, Color::srgb(1.0, 1.0, 0.0)));
    }
    if play.paused && play.dialog.is_none() {
        g.fill_rect(0.0, 0.0, 640.0, 480.0, Color::srgba(0.0, 0.0, 0.0, 0.5));
        g.text_centered(a.font("FONT_HUGE"), "PAUSED", 320.0, 240.0, Color::WHITE);
        g.text_centered(a.font("FONT_MAIN10"), "CLICK OR PRESS SPACE TO CONTINUE", 320.0, 280.0, Color::srgb(1.0, 0.9, 0.6));
    }

    if options.open {
        options.draw(&mut g, a, &mut mouse, &mut sfx, &mut profile, &mut windows);
        return;
    }
    let Some(dialog) = play.dialog.clone() else {
        play.dialog_frames = 0;
        return;
    };
    play.dialog_frames += 1;
    let bot_go = debug.autoplay && play.dialog_frames > 30 && coop.as_ref().is_none_or(|c| c.host);
    // Co-op: the host decides (`go` = continue / try again); the guest follows.
    let guest = coop.as_ref().is_some_and(|c| !c.host);
    let host_go = guest && take_ctl(coop.as_deref_mut(), "go");
    let f = a.font("FONT_MAIN12");
    let col = Color::srgb(1.0, 0.9, 0.6);
    match dialog {
        PlayDialog::Menu => {
            let (x, y, w, h) = (195.0, 120.0, 250.0, 250.0);
            dialog_box(&mut g, a, x, y, w, h, "MENU");
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "CONTINUE", x + 45.0, y + 80.0, 160.0) || keys.just_pressed(KeyCode::Escape) {
                play.dialog = None;
            }
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "OPTIONS", x + 45.0, y + 125.0, 160.0) {
                options.open = true;
            }
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "MAIN MENU", x + 45.0, y + 170.0, 160.0) {
                if let Some(c) = coop.as_deref_mut() {
                    c.send_ctl("menu");
                }
                record_scores(&session, &mut profile);
                next.set(Screen::MainMenu);
            }
        }
        PlayDialog::LevelComplete { points, seconds, par, bonus, max_combo, stage_done } => {
            let (x, y, w, h) = (170.0, 90.0, 300.0, 300.0);
            let title = if stage_done { "STAGE COMPLETE!" } else { "LEVEL COMPLETE!" };
            dialog_box(&mut g, a, x, y, w, h, title);
            let lines = [
                ("POINTS", points.to_string()),
                ("YOUR TIME", fmt_time(seconds)),
                ("ACE TIME", fmt_time(par)),
                ("MAX COMBO", max_combo.to_string()),
            ];
            for (i, (k, v)) in lines.iter().enumerate() {
                let ly = y + 95.0 + i as f32 * 26.0;
                g.text(f, k, x + 45.0, ly, col);
                g.text_right(f, v, x + w - 45.0, ly, Color::WHITE);
            }
            let btxt = if bonus > 0 { format!("BONUS +{bonus}") } else { "NO TIME BONUS".to_string() };
            g.text_centered(a.font("FONT_MAIN10OUTLINE2"), &btxt, x + w * 0.5, y + 210.0, Color::srgb(1.0, 1.0, 0.0));
            let go = if guest {
                waiting(&mut g, a, x + w * 0.5, y + h - 45.0);
                host_go
            } else {
                dialog_button(&mut g, a, &mut mouse, &mut sfx, "CONTINUE", x + 70.0, y + h - 70.0, 160.0)
                    || keys.just_pressed(KeyCode::Enter)
                    || bot_go
            };
            if go {
                if let Some(c) = coop.as_deref_mut().filter(|c| c.host) {
                    c.send_ctl("go");
                }
                let seed = coop.as_ref().map(|c| c.base_seed);
                if advance(a, &mut session, &mut profile, seed) {
                    restart(play, a, &mut images, &mut session, &mut next, coop.as_deref_mut());
                } else {
                    play.dialog = Some(PlayDialog::Victory);
                }
            }
        }
        PlayDialog::LifeLost => {
            let (x, y, w, h) = (195.0, 150.0, 250.0, 190.0);
            dialog_box(&mut g, a, x, y, w, h, "OUCH!");
            let msg = if session.lives == 1 { "LAST LIFE!".to_string() } else { format!("{} LIVES LEFT", session.lives) };
            g.text_centered(f, &msg, x + w * 0.5, y + 95.0, col);
            let go = if guest {
                waiting(&mut g, a, x + w * 0.5, y + h - 45.0);
                host_go
            } else {
                dialog_button(&mut g, a, &mut mouse, &mut sfx, "TRY AGAIN", x + 45.0, y + h - 70.0, 160.0)
                    || keys.just_pressed(KeyCode::Enter)
                    || bot_go
            };
            if go {
                if let Some(c) = coop.as_deref_mut().filter(|c| c.host) {
                    c.send_ctl("go");
                }
                restart(play, a, &mut images, &mut session, &mut next, coop.as_deref_mut());
            }
        }
        PlayDialog::GameOver => {
            let (x, y, w, h) = (195.0, 150.0, 250.0, 190.0);
            dialog_box(&mut g, a, x, y, w, h, "GAME OVER");
            g.text_centered(f, &format!("SCORE: {}", session.score), x + w * 0.5, y + 95.0, col);
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "MAIN MENU", x + 45.0, y + h - 70.0, 160.0)
                || keys.just_pressed(KeyCode::Enter)
            {
                if let Some(c) = coop.as_deref_mut() {
                    c.send_ctl("menu");
                }
                next.set(Screen::MainMenu);
            }
        }
        PlayDialog::Disconnected => {
            let (x, y, w, h) = (170.0, 150.0, 300.0, 190.0);
            dialog_box(&mut g, a, x, y, w, h, "CO-OP");
            g.text_centered(f, "EL OTRO JUGADOR SE DESCONECTÓ", x + w * 0.5, y + 95.0, col);
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "MAIN MENU", x + 70.0, y + h - 70.0, 160.0)
                || keys.just_pressed(KeyCode::Enter)
            {
                record_scores(&session, &mut profile);
                next.set(Screen::MainMenu);
            }
        }
        PlayDialog::Victory => {
            if let Some(img) = a.file_image("images/happyending", &mut images) {
                g.image(&img, 0.0, 0.0);
            }
            let brown = a.font("FONT_BROWNTITLE");
            g.text_centered(brown, "CONGRATULATIONS!", 320.0, 40.0, Color::srgb(1.0, 0.9, 0.4));
            g.text_wrapped(
                a.font("FONT_MAIN10"),
                "Excellent work, mighty frog one! You obtained your lawful place of the stars, at the side of the ancient ruler of the Zuma.",
                120.0,
                400.0,
                400.0,
                Color::WHITE,
                true,
            );
            if mouse.left_released {
                if let Some(c) = coop.as_deref_mut() {
                    c.send_ctl("menu");
                }
                record_scores(&session, &mut profile);
                next.set(Screen::MainMenu);
            }
        }
    }
}

fn fmt_time(s: i32) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

fn level_won(play: &mut Play, a: &GameAssets, session: &mut Session, profile: &mut Profile, sfx: &mut Sfx) {
    let b = &mut play.board;
    let seconds = b.seconds();
    let par = b.level.settings.par_time;
    let bonus = if seconds <= par { (par - seconds) * 100 } else { 0 };
    b.score += bonus;
    session.score = b.score;
    let id = b.level.graphics.id.to_ascii_lowercase();
    profile.unlock(&id);
    let best = profile.best_time.entry(id).or_insert(i32::MAX);
    *best = (*best).min(seconds);
    let stage_done =
        matches!(session.mode, Mode::Adventure) && a.levels.stages.get(session.stage).is_some_and(|st| session.level + 1 >= st.len());
    if stage_done {
        if session.stage % 3 == 2 {
            sfx.play("SOUND_TEMPLE_COMPLETE");
        } else {
            sfx.play("SOUND_STAGE_COMPLETE");
        }
    } else {
        sfx.play("SOUND_LEVEL_UP");
    }
    profile.save();
    play.dialog = Some(PlayDialog::LevelComplete { points: b.level_score, seconds, par, bonus, max_combo: b.stats.max_combo, stage_done });
}

/// Moves the session to the next level. Returns false when the game is won.
/// `coop_seed`: in co-op the next Survival level must be the same on both
/// machines, so it is picked from the session seed among all levels.
fn advance(a: &GameAssets, s: &mut Session, profile: &mut Profile, coop_seed: Option<u64>) -> bool {
    match s.mode {
        Mode::Adventure => {
            s.level += 1;
            let len = a.levels.stages.get(s.stage).map(|st| st.len()).unwrap_or(0);
            if s.level >= len {
                s.level = 0;
                s.stage += 1;
                if s.stage >= a.levels.stages.len() {
                    profile.max_stage = a.levels.stages.len() - 1;
                    record_scores(s, profile);
                    return false;
                }
            }
            if s.stage > profile.max_stage {
                profile.max_stage = s.stage;
                profile.save();
            }
        }
        Mode::Practice { .. } => s.round += 1,
        Mode::Survival => {
            s.round += 1;
            let unlocked: Vec<String> = a
                .levels
                .gauntlet
                .iter()
                .filter(|g| coop_seed.is_some() || profile.is_unlocked(&g.graphics))
                .map(|g| g.graphics.clone())
                .collect();
            if !unlocked.is_empty() {
                let i = match coop_seed {
                    Some(seed) => (seed.wrapping_add(s.round as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 33) as usize % unlocked.len(),
                    None => rand::rng().random_range(0..unlocked.len()),
                };
                s.graphics = unlocked[i].clone();
            }
        }
    }
    true
}

fn restart(
    play: &mut Play,
    a: &mut GameAssets,
    images: &mut Assets<Image>,
    s: &mut Session,
    next: &mut NextState<Screen>,
    coop: Option<&mut Coop>,
) {
    match build(a, images, s, coop) {
        Some(p) => *play = p,
        None => next.set(Screen::MainMenu),
    }
}

fn record_scores(s: &Session, profile: &mut Profile) {
    match &s.mode {
        Mode::Adventure => profile.adventure_best = profile.adventure_best.max(s.score),
        Mode::Practice { graphics } => {
            let e = profile.gauntlet_best.entry(graphics.to_ascii_lowercase()).or_insert(0);
            *e = (*e).max(s.score);
        }
        Mode::Survival => {
            let e = profile.gauntlet_best.entry("survival".into()).or_insert(0);
            *e = (*e).max(s.score);
        }
    }
    profile.save();
}

/// `--autoplay`: aims at a ball of the frog's colour and shoots now and then.
fn bot_input(play: &mut Play, input: &mut BoardInput, me: usize) {
    let b = &play.board;
    play.bot_cooldown -= 1;
    let gun = &b.guns[me.min(b.guns.len() - 1)];
    let Some(cur) = gun.current else { return };
    let target = b
        .chains
        .iter()
        .flat_map(|c| c.balls.iter().filter(|x| c.visible(x) && x.color == cur).map(move |x| c.ball_draw_pos(x)))
        .min_by(|p, q| p.distance(gun.center).total_cmp(&q.distance(gun.center)));
    let any = b.chains.iter().flat_map(|c| c.balls.iter().filter(|x| c.visible(x)).map(move |x| c.ball_draw_pos(x))).next();
    if let Some(t) = target.or(any) {
        input.aim = t;
        if play.bot_cooldown <= 0 && b.state == BoardState::Playing {
            // Fire only once the frog faces the target.
            let d = (t - gun.center).normalize_or_zero();
            if d.dot(gun.dir()) > 0.995 || (t - gun.center).length() < BALL_D * 3.0 {
                input.fire = true;
                play.bot_cooldown = 35;
            }
        }
    }
}

/// Removes a pending control message from the host.
fn take_ctl(coop: Option<&mut Coop>, what: &str) -> bool {
    let Some(c) = coop else { return false };
    match c.ctl.iter().position(|x| x == what) {
        Some(i) => {
            c.ctl.remove(i);
            true
        }
        None => false,
    }
}

fn waiting(g: &mut Gfx, a: &GameAssets, cx: f32, y: f32) {
    g.text_centered(a.font("FONT_MAIN10"), "ESPERANDO AL ANFITRIÓN...", cx, y, Color::srgb(1.0, 0.9, 0.6));
}

/// Co-op overlay: P1 / P2 over the frogs and the connection state.
fn draw_coop(g: &mut Gfx, a: &GameAssets, play: &Play, c: &Coop) {
    let f = a.font("FONT_MAIN10OUTLINE");
    g.with_raw(|g| {
        for (i, gun) in play.board.guns.iter().enumerate() {
            let col = if i == c.me() { Color::srgb(0.5, 1.0, 0.5) } else { Color::srgb(1.0, 0.8, 0.4) };
            g.text_centered(f, &format!("P{}", i + 1), gun.center.x, gun.center.y - 58.0, col);
        }
    });
    if c.stalled > 30 {
        g.text_centered(f, "ESPERANDO AL OTRO JUGADOR...", 320.0, 470.0, Color::srgb(1.0, 0.6, 0.3));
    } else if c.desync {
        g.text_centered(f, "AVISO: PARTIDAS DESINCRONIZADAS", 320.0, 470.0, Color::srgb(1.0, 0.3, 0.3));
    }
}
