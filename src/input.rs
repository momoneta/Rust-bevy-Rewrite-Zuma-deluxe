//! Mouse state in the original 640x480 coordinate system.

use crate::gfx::{SCREEN_H, SCREEN_W};
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
    pub fn over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.inside && self.pos.x >= x && self.pos.y >= y && self.pos.x < x + w && self.pos.y < y + h
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
            mouse.pos = Vec2::new(w.x + SCREEN_W * 0.5, SCREEN_H * 0.5 - w.y);
            mouse.inside = (0.0..SCREEN_W).contains(&mouse.pos.x) && (0.0..SCREEN_H).contains(&mouse.pos.y);
        }
        None => mouse.inside = false,
    }
}
