//! Immediate-mode 2D drawing on top of Bevy sprites.
//!
//! The original game draws every frame through `Sexy::Graphics`
//! (`DrawImage`, `DrawImageCel`, `DrawImageRotated`, `DrawString`...).  To keep
//! the port of that drawing code readable, screens push draw commands into
//! the [`Gfx`] resource in the 640x480 top-left-origin coordinate system of
//! the original, and [`flush_gfx`] maps them onto a pool of sprite entities.

use crate::assets::{Font, Img};
use bevy::camera::ScalingMode;
use bevy::prelude::*;

pub const SCREEN_W: f32 = 640.0;
pub const SCREEN_H: f32 = 480.0;

#[derive(Clone)]
struct DrawCmd {
    tex: Handle<Image>,
    src: Option<Rect>,
    pos: Vec2,
    size: Vec2,
    rot: f32,
    pivot: Vec2,
    color: Color,
    flip_x: bool,
}

/// Optional parameters for [`Gfx::draw_ex`].
#[derive(Clone, Copy)]
pub struct DrawOpts {
    /// Clockwise rotation in radians (screen space, y down).
    pub rot: f32,
    /// Rotation centre relative to the top-left of the image, in pixels.
    /// `None` = image centre.
    pub pivot: Option<Vec2>,
    pub color: Color,
    pub scale: f32,
    pub flip_x: bool,
}

impl Default for DrawOpts {
    fn default() -> Self {
        Self { rot: 0.0, pivot: None, color: Color::WHITE, scale: 1.0, flip_x: false }
    }
}

#[derive(Resource, Default)]
pub struct Gfx {
    cmds: Vec<DrawCmd>,
    white: Handle<Image>,
    /// Translation applied to every draw (like `Graphics::Translate`).
    pub offset: Vec2,
}

impl Gfx {
    pub fn set_white(&mut self, white: Handle<Image>) {
        self.white = white;
    }

    fn push(&mut self, mut cmd: DrawCmd) {
        cmd.pos += self.offset;
        self.cmds.push(cmd);
    }

    /// Changes the destination size of the last command (keeps its top-left).
    pub fn resize_last(&mut self, size: Vec2) {
        if let Some(c) = self.cmds.last_mut() {
            c.size = size;
            c.pivot = Vec2::ZERO;
        }
    }

    pub fn image(&mut self, img: &Img, x: f32, y: f32) {
        self.cel(img, 0, x, y);
    }

    pub fn cel(&mut self, img: &Img, cel: usize, x: f32, y: f32) {
        self.draw_ex(img, cel, x, y, DrawOpts::default());
    }

    pub fn draw_ex(&mut self, img: &Img, cel: usize, x: f32, y: f32, o: DrawOpts) {
        let size = Vec2::new(img.w, img.h) * o.scale;
        let pivot = o.pivot.map(|p| p * o.scale).unwrap_or(size * 0.5);
        self.push(DrawCmd {
            tex: img.cell(cel).clone(),
            src: None,
            pos: Vec2::new(x, y),
            size,
            rot: o.rot,
            pivot,
            color: o.color,
            flip_x: o.flip_x,
        });
    }

    /// Draws a centred image (cell) rotated around its centre.
    pub fn centered(&mut self, img: &Img, cel: usize, cx: f32, cy: f32, rot: f32, scale: f32, color: Color) {
        let o = DrawOpts { rot, color, scale, ..default() };
        self.draw_ex(img, cel, cx - img.w * scale * 0.5, cy - img.h * scale * 0.5, o);
    }

    /// Draws part of a cell: `src` is a rectangle inside the cell.
    pub fn sub(&mut self, img: &Img, cel: usize, src: Rect, x: f32, y: f32, color: Color) {
        self.sub_ex(img, cel, src, x, y, DrawOpts { color, pivot: Some(Vec2::ZERO), ..default() });
    }

    pub fn sub_ex(&mut self, img: &Img, cel: usize, src: Rect, x: f32, y: f32, o: DrawOpts) {
        let cel = cel.min(img.count() - 1);
        let (cx, cy) = ((cel as u32 % img.cols) as f32 * img.w, (cel as u32 / img.cols) as f32 * img.h);
        let r = Rect::new(cx + src.min.x, cy + src.min.y, cx + src.max.x, cy + src.max.y);
        let size = src.size() * o.scale;
        self.push(DrawCmd {
            tex: img.full.clone(),
            src: Some(r),
            pos: Vec2::new(x, y),
            size,
            rot: o.rot,
            pivot: o.pivot.map(|p| p * o.scale).unwrap_or(size * 0.5),
            color: o.color,
            flip_x: o.flip_x,
        });
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let tex = self.white.clone();
        self.push(DrawCmd {
            tex,
            src: None,
            pos: Vec2::new(x, y),
            size: Vec2::new(w, h),
            rot: 0.0,
            pivot: Vec2::ZERO,
            color,
            flip_x: false,
        });
    }

    /// `Graphics::DrawString`: `y` is the baseline.
    pub fn text(&mut self, font: &Font, s: &str, x: f32, y: f32, color: Color) {
        let desc = font.desc.clone();
        let tags: Vec<&str> = font.tags.iter().map(String::as_str).collect();
        let chars: Vec<char> = s.chars().collect();
        let c = color.to_linear();
        for (li, layer) in desc.layers.iter().enumerate() {
            if !layer.active(&tags) {
                continue;
            }
            let Some(tex) = font.textures.get(li).cloned().flatten() else { continue };
            let m = layer.color_mult;
            let a = layer.color_add;
            let tint = Color::LinearRgba(LinearRgba::new(
                (m[0] * c.red + a[0]).clamp(0.0, 1.0),
                (m[1] * c.green + a[1]).clamp(0.0, 1.0),
                (m[2] * c.blue + a[2]).clamp(0.0, 1.0),
                (m[3] * c.alpha).clamp(0.0, 1.0),
            ));
            let mut cx = x;
            for i in 0..chars.len() {
                let ch = desc.map_char(chars[i]);
                let next = chars.get(i + 1).map(|n| desc.map_char(*n));
                let Some(g) = layer.glyphs.get(&ch) else {
                    continue;
                };
                if g.rect[2] > 0 && g.rect[3] > 0 {
                    let gx = cx + (layer.offset.0 + g.offset.0) as f32;
                    let gy = y - (layer.ascent - layer.offset.1 - g.offset.1) as f32;
                    let r = Rect::new(g.rect[0] as f32, g.rect[1] as f32, (g.rect[0] + g.rect[2]) as f32, (g.rect[1] + g.rect[3]) as f32);
                    self.push(DrawCmd {
                        tex: tex.clone(),
                        src: Some(r),
                        pos: Vec2::new(gx, gy),
                        size: r.size(),
                        rot: 0.0,
                        pivot: Vec2::ZERO,
                        color: tint,
                        flip_x: false,
                    });
                }
                let kern = next.and_then(|n| layer.kerning.get(&(ch, n))).copied().unwrap_or(0);
                cx += (g.width + kern) as f32;
            }
        }
    }

    pub fn text_centered(&mut self, font: &Font, s: &str, cx: f32, y: f32, color: Color) {
        let w = font.width(s);
        self.text(font, s, (cx - w * 0.5).round(), y, color);
    }

    pub fn text_right(&mut self, font: &Font, s: &str, right: f32, y: f32, color: Color) {
        let w = font.width(s);
        self.text(font, s, right - w, y, color);
    }

    /// Word-wrapped text inside a box; returns the height used.
    pub fn text_wrapped(&mut self, font: &Font, s: &str, x: f32, y: f32, w: f32, color: Color, center: bool) -> f32 {
        let lines = wrap(font, s, w);
        let ls = font.line_spacing().max(font.height());
        let mut cy = y + font.ascent();
        for line in &lines {
            if center {
                self.text_centered(font, line, x + w * 0.5, cy, color);
            } else {
                self.text(font, line, x, cy, color);
            }
            cy += ls;
        }
        lines.len() as f32 * ls
    }
}

pub fn wrap(font: &Font, s: &str, w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if font.width(&cand) > w && !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            } else {
                cur = cand;
            }
        }
        lines.push(cur);
    }
    lines
}

/// Pool of sprite entities reused every frame.
#[derive(Resource, Default)]
pub struct SpritePool {
    entities: Vec<Entity>,
    visible: usize,
}

#[derive(Component)]
pub struct PooledSprite;

pub fn to_world(p: Vec2) -> Vec2 {
    Vec2::new(p.x - SCREEN_W * 0.5, SCREEN_H * 0.5 - p.y)
}

pub fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: SCREEN_W, min_height: SCREEN_H },
            ..OrthographicProjection::default_2d()
        }),
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
    ));
}

fn cmd_transform(c: &DrawCmd, z: f32) -> (Transform, Vec2) {
    // Centre of the quad before rotation, rotated around the pivot.
    let centre = c.pos + c.size * 0.5;
    let pivot = c.pos + c.pivot;
    let (s, co) = c.rot.sin_cos();
    let d = centre - pivot;
    let rotated = pivot + Vec2::new(d.x * co - d.y * s, d.x * s + d.y * co);
    let w = to_world(rotated);
    let t = Transform::from_xyz(w.x, w.y, z).with_rotation(Quat::from_rotation_z(-c.rot));
    (t, c.size)
}

/// Copies the frame's draw commands onto sprite entities.
pub fn flush_gfx(
    mut commands: Commands,
    mut gfx: ResMut<Gfx>,
    mut pool: ResMut<SpritePool>,
    mut q: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<PooledSprite>>,
) {
    let cmds = std::mem::take(&mut gfx.cmds);
    gfx.offset = Vec2::ZERO;
    for (i, c) in cmds.iter().enumerate() {
        let (t, size) = cmd_transform(c, i as f32 * 0.0005);
        let sprite = Sprite { image: c.tex.clone(), color: c.color, custom_size: Some(size), rect: c.src, flip_x: c.flip_x, ..default() };
        if let Some(&e) = pool.entities.get(i) {
            if let Ok((mut s, mut tr, mut vis)) = q.get_mut(e) {
                *s = sprite;
                *tr = t;
                if *vis != Visibility::Inherited {
                    *vis = Visibility::Inherited;
                }
            }
        } else {
            let e = commands.spawn((sprite, t, Visibility::Inherited, PooledSprite)).id();
            pool.entities.push(e);
        }
    }
    let n = cmds.len();
    for i in n..pool.visible.min(pool.entities.len()) {
        if let Ok((_, _, mut vis)) = q.get_mut(pool.entities[i]) {
            *vis = Visibility::Hidden;
        }
    }
    pool.visible = n;
    gfx.cmds = cmds;
    gfx.cmds.clear();
}
