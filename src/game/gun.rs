//! The frog (`Sexy::Gun`).
//!
//! Layout constants come from the original drawing routine (`FUN_004d1530`):
//! the frog image is rotated around its centre (54,54), the loaded ball sits
//! 25px in front of the centre, the next-ball gem 32px behind it.

use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunState {
    Ready,
    /// Tongue animation after a shot.
    Firing,
    /// The next ball slides into the mouth.
    Reloading,
}

#[derive(Clone, Debug)]
pub struct Gun {
    pub center: Vec2,
    /// Clockwise rotation (screen space); 0 = mouth facing down.
    pub angle: f32,
    pub current: Option<u8>,
    pub next: Option<u8>,
    pub state: GunState,
    /// Progress of the current animation, 0..1.
    pub anim: f32,
    pub blink: i32,
    pub reload_delay: i32,
    reload_wait: i32,
}

impl Gun {
    pub fn new(center: Vec2, reload_delay: i32) -> Self {
        Self { center, angle: 0.0, current: None, next: None, state: GunState::Ready, anim: 0.0, blink: 0, reload_delay, reload_wait: 0 }
    }

    /// Unit vector the frog is facing.
    pub fn dir(&self) -> Vec2 {
        Vec2::new(-self.angle.sin(), self.angle.cos())
    }

    pub fn aim_at(&mut self, target: Vec2) {
        let d = target - self.center;
        if d.length_squared() > 1.0 {
            self.angle = (-d.x).atan2(d.y);
        }
    }

    /// Rotates a point given in the frog's local frame (mouth = +y).
    pub fn local_to_screen(&self, local: Vec2) -> Vec2 {
        let (s, c) = self.angle.sin_cos();
        self.center + Vec2::new(local.x * c - local.y * s, local.x * s + local.y * c)
    }

    /// Where the loaded ball is drawn.
    pub fn ball_pos(&self) -> Vec2 {
        let y = match self.state {
            GunState::Ready => 25.0,
            GunState::Firing => 25.0,
            // Slides from behind the eyes (-20) to the mouth (+25).
            GunState::Reloading => -20.0 + self.anim * 45.0,
        };
        self.local_to_screen(Vec2::new(1.0, y))
    }

    pub fn can_fire(&self) -> bool {
        self.current.is_some() && self.state == GunState::Ready
    }

    /// Takes the loaded ball; the caller creates the bullet.
    pub fn fire(&mut self) -> Option<u8> {
        if !self.can_fire() {
            return None;
        }
        let c = self.current.take();
        self.state = GunState::Firing;
        self.anim = 0.0;
        c
    }

    pub fn swap(&mut self) {
        if self.state == GunState::Ready && self.current.is_some() && self.next.is_some() {
            std::mem::swap(&mut self.current, &mut self.next);
        }
    }

    /// Advances animations; `new_next` supplies the colour of the next ball.
    pub fn tick(&mut self, mut new_next: impl FnMut() -> Option<u8>) {
        match self.state {
            GunState::Ready => {}
            GunState::Firing => {
                self.anim += 0.1;
                if self.anim >= 1.0 {
                    self.reload_wait = self.reload_delay;
                    self.state = GunState::Reloading;
                    self.anim = 0.0;
                    self.current = self.next.take();
                    self.next = new_next();
                }
            }
            GunState::Reloading => {
                if self.reload_wait > 0 {
                    self.reload_wait -= 1;
                } else {
                    self.anim += 0.125;
                    if self.anim >= 1.0 {
                        self.anim = 0.0;
                        self.state = GunState::Ready;
                    }
                }
            }
        }
        if self.current.is_none() && self.state == GunState::Ready {
            self.current = self.next.take().or_else(&mut new_next);
            if self.current.is_some() {
                self.state = GunState::Reloading;
                self.anim = 0.0;
            }
        }
        if self.next.is_none() {
            self.next = new_next();
        }
        // Eye blink: the original counts 25 ticks down from a random start.
        if self.blink > 0 {
            self.blink -= 1;
        }
    }

    /// Tongue length used when drawing (0x33 idle, 0x25 when empty).
    pub fn tongue_offset(&self) -> f32 {
        match self.state {
            _ if self.current.is_none() && self.state != GunState::Firing => 37.0,
            GunState::Ready => 51.0,
            GunState::Firing => 51.0 - (self.anim * std::f32::consts::PI).sin() * 14.0,
            GunState::Reloading => 37.0 + self.anim * 14.0,
        }
    }
}
