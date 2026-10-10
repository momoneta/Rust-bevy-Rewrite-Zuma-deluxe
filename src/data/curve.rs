//! `levels/<name>/<curve>.dat` loader.
//!
//! Format recovered from the game's loader (`FUN_004dfbf0` in the
//! decompilation), little endian:
//!
//! ```text
//! "CURV"  i32 version (1..=3)
//! version > 2: u8 points_only, u8 has_flags          (absent: has_flags = 1)
//! !points_only: i32 editor_flag, i32 n, n bytes editor data (control points)
//! i32 count
//! version < 2: count * (f32 x, f32 y, u8 in_tunnel, u8 priority)
//! else:        f32 x, f32 y, [u8 in_tunnel, u8 priority]
//!              (count-1) * (i8 dx, i8 dy, [u8 in_tunnel, u8 priority])  dx,dy in 1/100 px
//! ```
//!
//! Consecutive points are ~1px apart, so a ball position along the path is
//! simply an index (fractional) into the point list.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathPoint {
    pub x: f32,
    pub y: f32,
    /// Fully inside a tunnel: invisible and cannot be hit.
    pub in_tunnel: bool,
    /// Draw priority relative to the level's `Cutout` overlays (0 = below).
    pub priority: u8,
}

#[derive(Debug, Clone, Default)]
pub struct CurvePath {
    pub points: Vec<PathPoint>,
}

#[derive(Debug)]
#[allow(dead_code)] // fields are shown through Debug
pub enum CurveError {
    BadHeader,
    BadVersion(i32),
    Truncated,
}

struct Reader<'a> {
    d: &'a [u8],
    o: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CurveError> {
        let s = self.d.get(self.o..self.o + n).ok_or(CurveError::Truncated)?;
        self.o += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, CurveError> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, CurveError> {
        Ok(self.u8()? as i8)
    }
    fn i32(&mut self) -> Result<i32, CurveError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, CurveError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

impl CurvePath {
    pub fn parse(data: &[u8]) -> Result<Self, CurveError> {
        let mut r = Reader { d: data, o: 0 };
        if r.take(4)? != b"CURV" {
            return Err(CurveError::BadHeader);
        }
        let version = r.i32()?;
        if !(1..=3).contains(&version) {
            return Err(CurveError::BadVersion(version));
        }
        let mut has_flags = true;
        let mut points_only = false;
        if version > 2 {
            points_only = r.u8()? != 0;
            has_flags = r.u8()? != 0;
        }
        if !points_only {
            let _editor_flag = r.i32()?;
            let size = r.i32()?;
            r.take(size.max(0) as usize)?;
        }
        let count = r.i32()?.max(0) as usize;
        let mut points = Vec::with_capacity(count);
        if count == 0 {
            return Ok(Self { points });
        }
        let read_flags =
            |r: &mut Reader| -> Result<(bool, u8), CurveError> { if has_flags { Ok((r.u8()? != 0, r.u8()?)) } else { Ok((false, 0)) } };
        if version < 2 {
            for _ in 0..count {
                let (x, y) = (r.f32()?, r.f32()?);
                let (in_tunnel, priority) = read_flags(&mut r)?;
                points.push(PathPoint { x, y, in_tunnel, priority });
            }
        } else {
            let (mut x, mut y) = (r.f32()?, r.f32()?);
            let (in_tunnel, priority) = read_flags(&mut r)?;
            points.push(PathPoint { x, y, in_tunnel, priority });
            for _ in 1..count {
                let dx = r.i8()? as f32 * 0.01;
                let dy = r.i8()? as f32 * 0.01;
                x += dx;
                y += dy;
                let (in_tunnel, priority) = read_flags(&mut r)?;
                points.push(PathPoint { x, y, in_tunnel, priority });
            }
        }
        Ok(Self { points })
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Last valid index as a float position.
    pub fn end(&self) -> f32 {
        (self.points.len().max(1) - 1) as f32
    }

    pub fn point(&self, pos: f32) -> PathPoint {
        let i = (pos.max(0.0) as usize).min(self.points.len() - 1);
        self.points[i]
    }

    /// Interpolated position along the path.
    pub fn pos(&self, pos: f32) -> (f32, f32) {
        if pos <= 0.0 {
            let p = self.points[0];
            return (p.x, p.y);
        }
        let i = pos as usize;
        if i + 1 >= self.points.len() {
            let p = self.points[self.points.len() - 1];
            return (p.x, p.y);
        }
        let t = pos - i as f32;
        let a = self.points[i];
        let b = self.points[i + 1];
        (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }

    /// Direction of travel at `pos` in radians (screen space, y down).
    pub fn angle(&self, pos: f32) -> f32 {
        let a = self.pos(pos - 4.0);
        let b = self.pos(pos + 4.0);
        (b.1 - a.1).atan2(b.0 - a.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_delta_encoded_v2() {
        let mut d = Vec::new();
        d.extend_from_slice(b"CURV");
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&1i32.to_le_bytes());
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&[9, 9]); // editor data
        d.extend_from_slice(&3i32.to_le_bytes());
        d.extend_from_slice(&10.0f32.to_le_bytes());
        d.extend_from_slice(&20.0f32.to_le_bytes());
        d.extend_from_slice(&[0, 1]);
        d.extend_from_slice(&[100, 0, 0, 1]);
        d.extend_from_slice(&[0, (-50i8) as u8, 1, 0]);
        let c = CurvePath::parse(&d).unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c.points[1].x, 11.0);
        assert!((c.points[2].y - 19.5).abs() < 1e-4);
        assert!(c.points[2].in_tunnel);
        assert_eq!(c.points[0].priority, 1);
    }
}
