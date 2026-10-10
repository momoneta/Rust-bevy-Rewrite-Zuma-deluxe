//! Drawing of the board (`Board::Draw`, `Board::DrawOverlay`) and the HUD.

use super::board::{Board, BoardState, EffectKind};
use super::chain::Chain;
use super::gun::GunState;
use super::{BALL_D, COLOR_NAMES};
use crate::assets::{GameAssets, Img, to_bevy_image};
use crate::data::image_load;
use crate::gfx::hud;
use crate::gfx::{DrawOpts, Gfx};
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Level art prepared when a level starts.
pub struct LevelGfx {
    pub background: Option<Img>,
    /// Cutouts: background pixels masked by the cutout alpha, drawn over
    /// low-priority balls (bridges, tunnels).
    pub cutouts: Vec<(Img, f32, f32)>,
    pub space: Option<Img>,
    /// Frog top layer (frog pixels under the hatchback mask).
    pub frog_top: Option<Img>,
}

impl LevelGfx {
    pub fn load(a: &mut GameAssets, images: &mut Assets<Image>, board: &Board) -> Self {
        let g = &board.level.graphics;
        let dir = format!("levels/{}", g.id);
        let bg_raw = g.image.as_ref().and_then(|img| image_load::load_image(&a.dir, &format!("{dir}/{img}"), None, None, 1, 1));
        let mut cutouts = Vec::new();
        if let Some(bg) = &bg_raw {
            for c in &g.cutouts {
                let Some(mask) = image_load::load_image(&a.dir, &format!("{dir}/{}", c.image), None, None, 1, 1) else {
                    warn!("missing cutout {}", c.image);
                    continue;
                };
                let mut out = image::RgbaImage::new(mask.width(), mask.height());
                for (x, y, p) in out.enumerate_pixels_mut() {
                    let (bx, by) = (x as i32 + c.x, y as i32 + c.y);
                    if bx >= 0 && by >= 0 && (bx as u32) < bg.width() && (by as u32) < bg.height() {
                        let src = bg.get_pixel(bx as u32, by as u32);
                        *p = image::Rgba([src[0], src[1], src[2], mask.get_pixel(x, y)[3]]);
                    }
                }
                cutouts.push((single(images, &out), c.x as f32, c.y as f32));
            }
        }
        let background = bg_raw.map(|raw| single(images, &raw));
        let space = if g.space { a.file_image("images/nebula1", images) } else { None };
        let frog_top = frog_top(a, images);
        Self { background, cutouts, space, frog_top }
    }
}

fn single(images: &mut Assets<Image>, raw: &image::RgbaImage) -> Img {
    let h = images.add(to_bevy_image(raw));
    Img { cells: vec![h.clone()], full: h, w: raw.width() as f32, h: raw.height() as f32, rows: 1, cols: 1 }
}

/// `FUN_004f8740(SHOOTER_BOTTOM, SHOOTER_TOP, 31, 12)`: the part of the frog
/// covered by the hatchback mask, drawn over the loaded ball.
fn frog_top(a: &GameAssets, images: &mut Assets<Image>) -> Option<Img> {
    let bottom = a.manifest.images.get("IMAGE_SHOOTER_BOTTOM")?;
    let top = a.manifest.images.get("IMAGE_SHOOTER_TOP")?;
    let base = image_load::load_image(&a.dir, &bottom.path, bottom.alpha_image.as_deref(), None, 1, 1)?;
    let mask = image_load::load_image(&a.dir, &top.path, top.alpha_image.as_deref(), None, 1, 1)?;
    let mut out = image::RgbaImage::new(mask.width(), mask.height());
    for (x, y, p) in out.enumerate_pixels_mut() {
        let (bx, by) = (x + 31, y + 12);
        if bx < base.width() && by < base.height() {
            let s = base.get_pixel(bx, by);
            let m = mask.get_pixel(x, y)[3] as u16;
            *p = image::Rgba([s[0], s[1], s[2], ((s[3] as u16 * m) / 255) as u8]);
        }
    }
    Some(single(images, &out))
}

fn rgb(c: (u8, u8, u8), a: f32) -> Color {
    Color::srgba_u8(c.0, c.1, c.2, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn draw_board(g: &mut Gfx, a: &GameAssets, lg: &LevelGfx, b: &Board, mouse: Vec2, menu_hover: bool) {
    // The playing field is in screen pixels (level files use the real resolution).
    g.raw = true;
    // Background.
    if let Some(space) = &lg.space {
        let off = (b.ticks as f32 * 0.1) % space.h;
        g.image(space, 0.0, -off);
        g.image(space, 0.0, space.h - off);
    }
    if let Some(bg) = &lg.background {
        g.image(bg, 0.0, 0.0);
    }

    // Holes at the end of every path, with the skull that opens as the
    // chain approaches. `skullrot` (degrees) fixes its rotation; -1 turns
    // the skull's mouth towards the incoming balls.
    for (ci, c) in b.chains.iter().enumerate() {
        let p = c.screen_pos(c.len());
        let rot = if c.cfg.skull_rot >= 0 {
            (c.cfg.skull_rot as f32).to_radians()
        } else {
            let d = p - c.screen_pos(c.len() - 24.0);
            d.x.atan2(-d.y)
        };
        let pulse = if b.danger { 0.75 + 0.25 * (b.ticks as f32 * 0.2).sin() } else { 1.0 };
        let color = Color::srgb(1.0, pulse, pulse);
        if let Some(hole) = a.img("IMAGE_HOLE") {
            g.centered(hole, 0, p.x, p.y, 0.0, 1.0, color);
        }
        if let Some(cover) = a.img("IMAGE_HOLE_COVER") {
            let frame = b.hole_open.get(ci).copied().unwrap_or(0.0).round() as usize;
            g.centered(cover, frame, p.x, p.y, rot, 1.0, color);
        }
    }

    // Balls under the cutouts, the cutouts, then the rest.
    for c in &b.chains {
        draw_chain(g, a, b, c, 0);
    }
    for (img, x, y) in &lg.cutouts {
        g.image(img, *x, *y);
    }
    for c in &b.chains {
        draw_chain(g, a, b, c, 1);
    }

    // End-of-level light.
    if let BoardState::Won(_) = b.state {
        if let Some(light) = a.img("IMAGE_WHITE_LIGHT") {
            for c in &b.chains {
                if b.trail < c.len() {
                    let p = c.screen_pos(b.trail);
                    g.centered_add(light, 0, p.x, p.y, 0.0, 2.5, Color::WHITE);
                }
            }
        }
    }

    // Coin.
    if let (Some(coin), Some(img)) = (&b.coin, a.img("IMAGE_COIN")) {
        let frame = (coin.age / 3) as usize % img.count();
        let fade = if coin.age > 900 { ((1000 - coin.age) as f32 / 100.0).max(0.0) } else { (coin.age as f32 / 30.0).min(1.0) };
        g.centered(img, frame, coin.pos.x, coin.pos.y, 0.0, 1.0, Color::srgba(1.0, 1.0, 1.0, fade));
    }

    // Accuracy guide.
    for gun in b.guns.iter().filter(|_| b.acc_ticks > 0 && b.state == BoardState::Playing) {
        if let Some(cur) = gun.current {
            let col = super::BALL_RGB[cur as usize];
            let dir = gun.dir();
            let start = gun.ball_pos();
            let mut p = start;
            let scr = crate::gfx::screen();
            for k in 0..120 {
                p += dir * 10.0;
                if !(0.0..scr.x).contains(&p.x) || !(0.0..scr.y).contains(&p.y) || hits_ball(b, p) {
                    break;
                }
                let alpha = 0.8 - k as f32 * 0.01;
                g.fill_rect(p.x - 1.5, p.y - 1.5, 3.0, 3.0, rgb(col, alpha));
            }
        }
    }

    // Bullets.
    for bl in &b.bullets {
        draw_ball_sprite(g, a, bl.color, None, bl.pos, bl.angle, 0, true);
    }

    for gun in &b.guns {
        draw_frog(g, a, lg, gun, b.ticks);
    }

    // Effects.
    for e in &b.effects {
        match e.kind {
            EffectKind::Pop(color) => {
                if let Some(img) = a.img("IMAGE_GRAY_EXPLOSION") {
                    let frame = (e.age as usize * img.count() / e.life.max(1) as usize).min(img.count() - 1);
                    let c = super::BALL_RGB[color as usize];
                    g.centered_add(img, frame, e.pos.x, e.pos.y, 0.0, 1.0, rgb(c, 1.0));
                }
            }
            EffectKind::Blast => {
                if let Some(img) = a.img("IMAGE_EXPLOSION") {
                    let frame = (e.age as usize * img.count() / e.life.max(1) as usize).min(img.count() - 1);
                    g.centered_add(img, frame, e.pos.x, e.pos.y, 0.0, 1.5, Color::WHITE);
                }
            }
            EffectKind::Sparkle => {
                if let Some(img) = a.img("IMAGE_SPARKLE") {
                    let frame = (e.age as usize / 3 + e.variant) % img.count();
                    let alpha = 1.0 - e.age as f32 / e.life as f32;
                    g.centered_add(img, frame, e.pos.x, e.pos.y, 0.0, 1.0, Color::srgba(1.0, 1.0, 0.8, alpha));
                }
            }
        }
    }

    // Floating score texts.
    let float = a.font("FONT_FLOAT");
    for t in &b.texts {
        if t.delay > 0 {
            continue;
        }
        let alpha = if t.age > 100 { 1.0 - (t.age - 100) as f32 / 50.0 } else { 1.0 };
        // Keep the text inside the playfield frame.
        let half = float.width(&t.text) * 0.5;
        let scr = crate::gfx::screen();
        let x = t.pos.x.clamp(half + 12.0, scr.x - 12.0 - half);
        let y = t.pos.y.clamp(50.0, scr.y - 12.0);
        g.text_centered(float, &t.text, x, y, rgb(t.color, alpha));
    }

    g.raw = false;
    draw_level_title(g, a, b);
    draw_hud(g, a, b, mouse, menu_hover);
}

fn hits_ball(b: &Board, p: Vec2) -> bool {
    b.chains.iter().any(|c| c.balls.iter().any(|ball| c.visible(ball) && c.ball_draw_pos(ball).distance_squared(p) < BALL_D * BALL_D * 0.5))
}

fn draw_chain(g: &mut Gfx, a: &GameAssets, b: &Board, c: &Chain, priority: u8) {
    for ball in &c.balls {
        if ball.pos < 0.0 {
            continue;
        }
        let pt = c.path.point(ball.pos);
        let pri = if ball.inserting() { 1 } else { pt.priority.min(1) };
        if pri != priority || (pt.in_tunnel && !ball.inserting()) {
            continue;
        }
        let p = c.ball_draw_pos(ball);
        let angle = c.path.angle(ball.pos) - FRAC_PI_2;
        let mut power = ball.power;
        // Power-ups blink before they expire.
        if power.is_some() && ball.power_ticks < 300 && (ball.power_ticks / 15) % 2 == 0 {
            power = None;
        }
        if let Some(pw) = power {
            let light = match pw {
                super::Power::Bomb => None,
                super::Power::Slow => a.img("IMAGE_SLOW_LIGHT"),
                super::Power::Accuracy => a.img("IMAGE_ACCURACY_LIGHT"),
                super::Power::Backwards => a.img("IMAGE_BACKWARDS_LIGHT"),
            };
            if let Some(l) = light {
                let pulse = 0.6 + 0.4 * (b.ticks as f32 * 0.15).sin();
                g.centered_add(l, 0, p.x, p.y, 0.0, 1.6, Color::srgba(1.0, 1.0, 1.0, pulse));
            }
        }
        let frame = ball.pos.max(0.0) as usize;
        draw_ball_sprite(g, a, ball.color, power, p, angle, frame, true);
    }
}

/// Draws one ball (rolling strip cell or power-up image) centred at `p`.
pub fn draw_ball_sprite(
    g: &mut Gfx,
    a: &GameAssets,
    color: u8,
    power: Option<super::Power>,
    p: Vec2,
    angle: f32,
    frame: usize,
    shadow: bool,
) {
    let name = COLOR_NAMES[color as usize % COLOR_NAMES.len()];
    if shadow {
        if let Some(s) = a.img("IMAGE_BALL_SHADOW") {
            g.centered(s, 0, p.x + 2.0, p.y + 3.0, 0.0, 1.0, Color::srgba(0.0, 0.0, 0.0, 0.45));
        }
    }
    if let Some(pw) = power {
        if let Some(img) = a.img(&format!("IMAGE_{name}_{}", pw.image_suffix())) {
            g.centered(img, 0, p.x, p.y, angle, 1.0, Color::WHITE);
            return;
        }
    }
    if let Some(img) = a.img(&format!("IMAGE_{name}_BALL")) {
        let n = img.count();
        g.centered(img, frame % n, p.x, p.y, angle, 1.0, Color::WHITE);
    }
}

fn draw_frog(g: &mut Gfx, a: &GameAssets, lg: &LevelGfx, gun: &super::gun::Gun, ticks: i32) {
    let (cx, cy) = (gun.center.x, gun.center.y);
    let rot = gun.angle;
    if let Some(bottom) = a.img("IMAGE_SHOOTER_BOTTOM") {
        g.draw_ex(bottom, 0, cx - 54.0, cy - 54.0, DrawOpts { rot, pivot: Some(Vec2::new(54.0, 54.0)), ..default() });
    }
    if let Some(tongue) = a.img("IMAGE_SHOOTER_TONGUE") {
        let t = gun.tongue_offset();
        g.draw_ex(tongue, 0, cx - 16.0, cy + t - 54.0, DrawOpts { rot, pivot: Some(Vec2::new(16.0, 54.0 - t)), ..default() });
    }
    if let Some(cur) = gun.current {
        let p = gun.ball_pos();
        draw_ball_sprite(g, a, cur, None, p, rot, 0, false);
    }
    if let (Some(next), Some(dots)) = (gun.next, a.img("IMAGE_NEXT_BALL")) {
        if gun.state != GunState::Reloading {
            g.draw_ex(dots, next as usize, cx - 7.0, cy - 32.0, DrawOpts { rot, pivot: Some(Vec2::new(7.0, 32.0)), ..default() });
        }
    }
    if let Some(top) = &lg.frog_top {
        g.draw_ex(top, 0, cx - 23.0, cy - 42.0, DrawOpts { rot, pivot: Some(Vec2::new(23.0, 42.0)), ..default() });
    }
    // Eye blink: two lid frames stacked vertically (20px each); frame 0 for
    // ticks 1-4 and 20-24 of the blink, frame 1 (closed) in between.
    let blink = ticks % 400;
    if blink < 25 {
        if let Some(eye) = a.img("IMAGE_EYE_BLINK") {
            let half = (eye.h / 2.0).floor();
            let frame = if (5..20).contains(&blink) { 1.0 } else { 0.0 };
            let src = Rect::new(0.0, frame * half, eye.w, frame * half + half);
            g.sub_ex(eye, 0, src, cx - 29.0, cy - 12.0, DrawOpts { rot, pivot: Some(Vec2::new(29.0, 12.0)), ..default() });
        }
    }
}

fn draw_level_title(g: &mut Gfx, a: &GameAssets, b: &Board) {
    let t = match b.state {
        BoardState::Intro(t) => t,
        BoardState::Playing => 60 + b.stats.play_ticks,
        _ => return,
    };
    if t > 300 {
        return;
    }
    let alpha = if t > 220 { 1.0 - (t - 220) as f32 / 80.0 } else { 1.0 };
    let huge = a.font("FONT_HUGE");
    let brown = a.font("FONT_BROWNTITLE");
    g.text_centered(huge, &b.title, 320.0, 200.0, Color::srgba(1.0, 1.0, 1.0, alpha));
    g.text_centered(brown, &b.level.graphics.display_name, 320.0, 240.0, Color::srgba(1.0, 0.9, 0.5, alpha));
}

pub fn draw_hud(g: &mut Gfx, a: &GameAssets, b: &Board, _mouse: Vec2, menu_hover: bool) {
    g.raw = true;
    if let Some(bar) = a.img("IMAGE_MENU_BAR") {
        g.image(bar, 0.0, 0.0);
    }
    let yellow = Color::srgb(1.0, 1.0, 0.0);
    let main = a.font("FONT_MAIN10");
    let p = hud(200.0, 17.0);
    g.text_centered(main, &b.title, p.x, p.y, yellow);

    let mono = a.font("FONT_MONODIGIT");
    let p = hud(360.0, 18.0);
    g.text_right(mono, &b.score.to_string(), p.x, p.y, yellow);

    if let (Some(gauge), Some(done)) = (a.img("IMAGE_ZUMA_BAR"), a.img("IMAGE_ZUMA_BAR_DONE")) {
        let p = b.bar_progress();
        let img = if b.bar_full { done } else { gauge };
        let w = (img.w * p).floor();
        if w > 0.0 {
            let p = hud(409.0, 2.0);
            g.sub(img, 0, Rect::new(0.0, 0.0, w, img.h), p.x, p.y, Color::WHITE);
        }
    }
    if let Some(btn) = a.img("IMAGE_MENU_BUTTON") {
        if menu_hover {
            let p = hud(540.0, 3.0);
            g.cel(btn, 1, p.x, p.y);
        }
    }
    g.raw = false;
}

/// Lives are drawn by the play screen (they belong to the session).
pub fn draw_lives(g: &mut Gfx, a: &GameAssets, lives: i32) {
    g.with_raw(|g| draw_lives_px(g, a, lives));
}

fn draw_lives_px(g: &mut Gfx, a: &GameAssets, lives: i32) {
    let shown = lives - 1;
    if shown <= 0 {
        if lives == 1 {
            let p = hud(64.0, 20.0);
            g.text_centered(a.font("FONT_MAIN10"), "Last Life", p.x, p.y, Color::srgb(1.0, 1.0, 0.0));
        }
        return;
    }
    let Some(icon) = a.img("IMAGE_FROG_LIVES") else { return };
    let icons = if shown > 3 { 1 } else { shown };
    let p = hud(28.0, 3.0);
    let mut x = p.x;
    for _ in 0..icons {
        g.image(icon, x, p.y);
        x += icon.w + 2.0;
    }
    if shown > 3 {
        g.text(a.font("FONT_MAIN10"), &format!("x{shown}"), x + 4.0, hud(0.0, 22.0).y, Color::srgb(1.0, 1.0, 0.0));
    }
}
