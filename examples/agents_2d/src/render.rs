//! A software renderer: draws a [`Sim`] into a slice of `0x00RRGGBB` pixels, the format
//! `softbuffer` uses, so it can be tested without a display.
//!
//! [`View`] maps between the world (meters, +Y up) and the frame (pixels, +Y down), letterboxing
//! the world to the frame. [`Frame`] checks that a pixel slice has the size it claims, so drawing
//! never needs a bounds panic. [`render`] allocates nothing.
//!
//! # Examples
//!
//! ```
//! use agents_2d::{Frame, Sim, SimConfig, View, render};
//!
//! let sim = Sim::new(&SimConfig { agents: 100, ..SimConfig::default() });
//! let mut pixels = vec![0_u32; 640 * 360];
//! let mut frame = Frame::new(&mut pixels, 640, 360).expect("the buffer has width * height pixels");
//! render(&sim, &View::fit(640, 360), &mut frame);
//! assert!(frame.pixels().contains(&agents_2d::render::WANDERER));
//! ```

use core::fmt;

use engine::prelude::*;

use crate::{Sim, WORLD_HEIGHT, WORLD_WIDTH};

/// Background colour, `0x00RRGGBB`.
pub const BACKGROUND: u32 = 0x10_1418;
/// Colour of the one-pixel border around the world.
pub const BORDER: u32 = 0x3A_4048;
/// Colour of an agent without the seeker part.
pub const WANDERER: u32 = 0x6E_C6FF;
/// Colour of an agent with the seeker part.
pub const SEEKER: u32 = 0xFF_B347;

/// A mutable pixel buffer with a width and a height, in `0x00RRGGBB`.
///
/// # Examples
///
/// ```
/// use agents_2d::Frame;
///
/// let mut pixels = [0_u32; 6];
/// let frame = Frame::new(&mut pixels, 3, 2).expect("3 * 2 pixels");
/// assert_eq!((frame.width(), frame.height()), (3, 2));
/// assert!(Frame::new(&mut [0_u32; 5], 3, 2).is_none());
/// ```
pub struct Frame<'a> {
    pixels: &'a mut [u32],
    width: u32,
    height: u32,
}

impl<'a> Frame<'a> {
    /// Wraps `pixels` as a `width` by `height` frame, or returns `None` unless
    /// `width * height == pixels.len()` (computed without overflow).
    #[must_use]
    pub fn new(pixels: &'a mut [u32], width: u32, height: u32) -> Option<Self> {
        let len = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?;
        (len == pixels.len()).then_some(Self {
            pixels,
            width,
            height,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, row by row from the top-left.
    #[must_use]
    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }

    /// Fills the half-open rectangle `x0..x1` by `y0..y1`, clipped to the frame.
    fn fill_rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: u32) {
        let (w, h) = (i64::from(self.width), i64::from(self.height));
        let (x0, x1) = (x0.max(0), x1.min(w));
        let (y0, y1) = (y0.max(0), y1.min(h));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        // Clipped to the frame, so every value below is in `0..=width` or `0..=height`.
        let width = usize::try_from(w).unwrap_or(0);
        let (xs, xe) = (
            usize::try_from(x0).unwrap_or(0),
            usize::try_from(x1).unwrap_or(0),
        );
        for row in usize::try_from(y0).unwrap_or(0)..usize::try_from(y1).unwrap_or(0) {
            self.pixels[row * width + xs..row * width + xe].fill(color);
        }
    }
}

impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// Maps the world into a frame: scaled to fit and centered, so the unused margin forms letterbox
/// bars.
///
/// A view fitted to a zero width or height has scale 0: it draws nothing and maps every cursor
/// to `None`.
///
/// # Examples
///
/// ```
/// use agents_2d::View;
/// use engine::prelude::*;
///
/// let view = View::fit(640, 480);
/// assert_eq!(view.scale(), 2.0);
/// assert_eq!(view.world_to_screen(Vec2::new(0.0, 180.0)), Vec2::new(0.0, 60.0));
/// assert_eq!(view.cursor_to_world(Vec2::new(320.0, 240.0)), Some(Vec2::new(160.0, 90.0)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl View {
    /// Fits the world into a `width` by `height` frame.
    ///
    /// The scale is `min(width / W, height / H)` pixels per meter and the world is centered.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "window sizes far below 2^24 are exact; larger ones only lose sub-pixel precision"
    )]
    pub fn fit(width: u32, height: u32) -> Self {
        if width == 0 || height == 0 {
            return Self {
                scale: 0.0,
                offset_x: 0.0,
                offset_y: 0.0,
            };
        }
        let (w, h) = (width as f32, height as f32);
        let scale = (w / WORLD_WIDTH).min(h / WORLD_HEIGHT);
        Self {
            scale,
            offset_x: (w - WORLD_WIDTH * scale) / 2.0,
            offset_y: (h - WORLD_HEIGHT * scale) / 2.0,
        }
    }

    /// Pixels per meter; 0 for a view fitted to a zero-sized frame.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// World position (meters, +Y up) to screen position (pixels, +Y down).
    #[must_use]
    pub fn world_to_screen(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            self.offset_x + p.x * self.scale,
            self.offset_y + (WORLD_HEIGHT - p.y) * self.scale,
        )
    }

    /// Screen position to world position: the inverse of [`world_to_screen`](Self::world_to_screen).
    /// A zero-scale view maps everything to the origin.
    #[must_use]
    pub fn screen_to_world(&self, s: Vec2) -> Vec2 {
        if self.scale == 0.0 {
            return Vec2::ZERO;
        }
        Vec2::new(
            (s.x - self.offset_x) / self.scale,
            WORLD_HEIGHT - (s.y - self.offset_y) / self.scale,
        )
    }

    /// The world position under screen position `s`, or `None` if it lies outside the world
    /// (in a letterbox bar, say) or the view has scale 0.
    #[must_use]
    pub fn cursor_to_world(&self, s: Vec2) -> Option<Vec2> {
        if self.scale == 0.0 {
            return None;
        }
        let w = self.screen_to_world(s);
        ((0.0..=WORLD_WIDTH).contains(&w.x) && (0.0..=WORLD_HEIGHT).contains(&w.y)).then_some(w)
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "`as` from float to integer saturates; clipping happens in i64 afterwards"
)]
fn floor_i64(f: f32) -> i64 {
    f.floor() as i64
}

/// Draws `sim` into `frame` through `view`.
///
/// Fills the background, draws a one-pixel border around the world, then draws every agent as a
/// filled square of `max(1, floor(scale))` pixels centered on its position, clipped to the frame.
/// If the frame or the view has a zero dimension, only the background is filled.
///
/// # Examples
///
/// ```
/// use agents_2d::{Frame, Sim, SimConfig, View, render};
///
/// let sim = Sim::new(&SimConfig { agents: 10, ..SimConfig::default() });
/// let mut pixels = vec![0_u32; 320 * 180];
/// let mut frame = Frame::new(&mut pixels, 320, 180).unwrap();
/// render(&sim, &View::fit(320, 180), &mut frame);
/// assert!(frame.pixels().contains(&agents_2d::render::BORDER));
/// ```
pub fn render(sim: &Sim, view: &View, frame: &mut Frame<'_>) {
    frame.pixels.fill(BACKGROUND);
    if frame.width == 0 || frame.height == 0 || view.scale <= 0.0 {
        return;
    }

    // Border: the outline of the world's screen rectangle.
    let top_left = view.world_to_screen(Vec2::new(0.0, WORLD_HEIGHT));
    let bottom_right = view.world_to_screen(Vec2::new(WORLD_WIDTH, 0.0));
    let (x0, y0) = (floor_i64(top_left.x), floor_i64(top_left.y));
    let (x1, y1) = (floor_i64(bottom_right.x), floor_i64(bottom_right.y));
    frame.fill_rect(x0, y0, x1.saturating_add(1), y0.saturating_add(1), BORDER);
    frame.fill_rect(x0, y1, x1.saturating_add(1), y1.saturating_add(1), BORDER);
    frame.fill_rect(x0, y0, x0.saturating_add(1), y1.saturating_add(1), BORDER);
    frame.fill_rect(x1, y0, x1.saturating_add(1), y1.saturating_add(1), BORDER);

    let side = floor_i64(view.scale).max(1);
    let half = side / 2;
    for agent in sim.agents() {
        let s = view.world_to_screen(agent.transform.translation);
        let x = floor_i64(s.x).saturating_sub(half);
        let y = floor_i64(s.y).saturating_sub(half);
        let color = if agent.seeker.is_some() {
            SEEKER
        } else {
            WANDERER
        };
        frame.fill_rect(x, y, x.saturating_add(side), y.saturating_add(side), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Input, Sim, SimConfig, WORLD_HEIGHT, WORLD_WIDTH};
    use proptest::prelude::*;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    fn scene(agents: usize, ticks: u64) -> Sim {
        let mut sim = Sim::new(&SimConfig {
            agents,
            ..SimConfig::default()
        });
        for _ in 0..ticks {
            sim.step(&Input::scripted(sim.tick()));
        }
        sim
    }

    #[test]
    fn frame_new_checks_the_length() {
        let mut buf = vec![0_u32; 12];
        assert!(Frame::new(&mut buf, 4, 3).is_some());
        assert!(Frame::new(&mut buf, 4, 4).is_none(), "buffer one short");
        assert!(Frame::new(&mut buf, 3, 3).is_none(), "buffer too long");
        assert!(
            Frame::new(&mut buf, u32::MAX, u32::MAX).is_none(),
            "overflowing size"
        );
        assert!(Frame::new(&mut [], 0, 0).is_some());
    }

    #[test]
    fn fit_letterboxes() {
        let view = View::fit(640, 480);
        assert_eq!(view.scale(), 2.0);
        assert_eq!(
            view.world_to_screen(Vec2::new(0.0, 180.0)),
            Vec2::new(0.0, 60.0)
        );
        assert_eq!(
            view.world_to_screen(Vec2::new(320.0, 0.0)),
            Vec2::new(640.0, 420.0)
        );
    }

    #[test]
    fn zero_sized_view_draws_nothing_and_maps_no_cursor() {
        for view in [View::fit(0, 100), View::fit(100, 0), View::fit(0, 0)] {
            for s in [Vec2::ZERO, Vec2::new(50.0, 50.0), Vec2::new(-1.0, 1e9)] {
                assert_eq!(view.cursor_to_world(s), None);
            }
            let mut empty: [u32; 0] = [];
            let mut frame = Frame::new(&mut empty, 0, 0).unwrap();
            render(&scene(10, 1), &view, &mut frame);
        }
        // A zero-sized view into a non-empty frame fills the background and stops.
        let mut buf = vec![SENTINEL; 16];
        let mut frame = Frame::new(&mut buf, 4, 4).unwrap();
        render(&scene(10, 1), &View::fit(0, 0), &mut frame);
        assert!(buf.iter().all(|&p| p == BACKGROUND));
    }

    #[test]
    fn cursor_outside_the_world_is_none() {
        let view = View::fit(640, 480);
        assert_eq!(
            view.cursor_to_world(Vec2::new(300.0, 10.0)),
            None,
            "letterbox bar"
        );
        assert_eq!(
            view.cursor_to_world(Vec2::new(300.0, 470.0)),
            None,
            "letterbox bar"
        );
        assert_eq!(view.cursor_to_world(Vec2::new(-5.0, 240.0)), None);
        assert_eq!(
            view.cursor_to_world(Vec2::new(320.0, 240.0)),
            Some(Vec2::new(160.0, 90.0))
        );
    }

    #[test]
    fn empty_sim_renders_background_and_border() {
        let sim = scene(0, 0);
        let mut buf = vec![0_u32; 320 * 180];
        let mut frame = Frame::new(&mut buf, 320, 180).unwrap();
        render(&sim, &View::fit(320, 180), &mut frame);
        assert!(buf.iter().all(|&p| p == BACKGROUND || p == BORDER));
        assert!(buf.contains(&BORDER));
    }

    fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
        bytes.into_iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    #[test]
    fn golden_pixels() {
        let sim = scene(200, 60);
        let mut buf = vec![0_u32; 320 * 180];
        let mut frame = Frame::new(&mut buf, 320, 180).unwrap();
        render(&sim, &View::fit(320, 180), &mut frame);
        let hash = fnv(frame.pixels().iter().flat_map(|p| p.to_le_bytes()));
        assert_eq!(hash, 0x57e2_497e_8128_338b, "{hash:#018x}");
    }

    proptest! {
        #[test]
        fn round_trip_within_half_a_pixel(
            w in 1_u32..2000, h in 1_u32..2000, fx in 0.0_f32..=1.0, fy in 0.0_f32..=1.0,
        ) {
            let view = View::fit(w, h);
            let p = Vec2::new(WORLD_WIDTH * fx, WORLD_HEIGHT * fy);
            let back = view.screen_to_world(view.world_to_screen(p));
            let tolerance = 0.5 / view.scale();
            prop_assert!((back.x - p.x).abs() <= tolerance, "x: {p:?} -> {back:?}");
            prop_assert!((back.y - p.y).abs() <= tolerance, "y: {p:?} -> {back:?}");
        }

        #[test]
        fn render_never_panics_or_spills(
            fw in 0_u32..64, fh in 0_u32..64, vw in 0_u32..64, vh in 0_u32..64,
        ) {
            let sim = scene(50, 5);
            let len = (fw * fh) as usize;
            let pad = 7;
            let mut buf = vec![SENTINEL; len + 2 * pad];
            {
                let mut frame = Frame::new(&mut buf[pad..pad + len], fw, fh).unwrap();
                render(&sim, &View::fit(vw, vh), &mut frame);
            }
            prop_assert!(buf[..pad].iter().all(|&p| p == SENTINEL));
            prop_assert!(buf[pad + len..].iter().all(|&p| p == SENTINEL));
            let palette = [BACKGROUND, BORDER, WANDERER, SEEKER];
            prop_assert!(buf[pad..pad + len].iter().all(|p| palette.contains(p)));
        }
    }
}
