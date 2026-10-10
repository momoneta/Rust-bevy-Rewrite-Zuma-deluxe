//! Mouse state. `pos` is in screen pixels (what the playing field uses);
//! hit tests take 640x480 design coordinates like the drawing code.

use crate::gfx::{screen, ui_xform};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Mouse {
    pub pos: Vec2,
    pub inside: bool,
    pub left_down: bool,
    pub left_pressed: bool,
    pub left_released: bool,
    pub right_pressed: bool,
    /// Set when a click was consumed by a button this frame.
    pub consumed: bool,
}

impl Mouse {
    /// Hit test of a box given in design coordinates.
    pub fn over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let k = ui_xform().0;
        self.over_px(x, y, w * k, h * k)
    }

    /// Hit test of a box at a design position with a size in pixels (an image).
    pub fn over_px(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let (k, off) = ui_xform();
        let (x, y) = (x * k + off.x, y * k + off.y);
        self.inside && self.pos.x >= x && self.pos.y >= y && self.pos.x < x + w && self.pos.y < y + h
    }

    /// Cursor in design coordinates.
    pub fn ui_pos(&self) -> Vec2 {
        let (k, off) = ui_xform();
        (self.pos - off) / k
    }

    pub fn clicked_in(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.left_released && self.over(x, y, w, h)
    }
}

pub fn update_mouse(
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform)>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut mouse: ResMut<Mouse>,
) {
    mouse.consumed = false;
    mouse.left_down = buttons.pressed(MouseButton::Left);
    mouse.left_pressed = buttons.just_pressed(MouseButton::Left);
    mouse.left_released = buttons.just_released(MouseButton::Left);
    mouse.right_pressed = buttons.just_pressed(MouseButton::Right);
    let (Ok(window), Ok((cam, cam_t))) = (windows.single(), cams.single()) else { return };
    match window.cursor_position().and_then(|c| cam.viewport_to_world_2d(cam_t, c).ok()) {
        Some(w) => {
            let s = screen();
            mouse.pos = Vec2::new(w.x + s.x * 0.5, s.y * 0.5 - w.y);
            mouse.inside = (0.0..s.x).contains(&mouse.pos.x) && (0.0..s.y).contains(&mouse.pos.y);
        }
        None => mouse.inside = false,
    }
}
