use glam::Vec2;

use crate::RasterSettings;

/// Maps model millimetres onto the display's pixel grid.
///
/// Row `0` is the first row a sliced file carries, which is the near edge of the plate:
/// Y runs the same way as model space. The panel's own
/// mirroring is applied here rather than in the geometry, so that the mesh on screen and
/// the mesh being sliced stay the same object.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PixelSpace {
    width_px: f32,
    height_px: f32,
    pitch: Vec2,
    mirror_x: bool,
    mirror_y: bool,
}

impl PixelSpace {
    pub(crate) fn new(settings: &RasterSettings) -> Self {
        Self {
            width_px: settings.width_px as f32,
            height_px: settings.height_px as f32,
            pitch: Vec2::new(settings.pitch.x, settings.pitch.y),
            mirror_x: settings.mirror_x,
            mirror_y: settings.mirror_y,
        }
    }

    /// Whether the map turns a contour the other way round.
    ///
    /// Material is what a positive winding encloses, so a ring that lands mirrored has to
    /// be reversed before it is swept: each panel mirror is one reflection, and it is the
    /// parity of the two that decides.
    pub(crate) fn reverses_winding(&self) -> bool {
        self.mirror_x != self.mirror_y
    }

    pub(crate) fn map(&self, point: Vec2) -> Vec2 {
        let mut x = point.x / self.pitch.x;
        let mut y = point.y / self.pitch.y;

        if self.mirror_x {
            x = self.width_px - x;
        }
        if self.mirror_y {
            y = self.height_px - y;
        }
        Vec2::new(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Grey, PixelPitch, Shading};

    fn space(mirror_x: bool, mirror_y: bool) -> PixelSpace {
        PixelSpace::new(&RasterSettings {
            width_px: 100,
            height_px: 50,
            pitch: PixelPitch { x: 0.1, y: 0.1 },
            mirror_x,
            mirror_y,
            shading: Shading::default(),
            grey: Grey::default(),
            blur_px: 0,
        })
    }

    #[test]
    fn the_plate_origin_lands_on_the_first_pixel() {
        assert_eq!(space(false, false).map(Vec2::ZERO), Vec2::ZERO);
    }

    #[test]
    fn the_far_corner_lands_on_the_last_pixel() {
        // The panel covers 10 x 5 mm at a 0.1 mm pitch.
        assert_eq!(
            space(false, false).map(Vec2::new(10.0, 5.0)),
            Vec2::new(100.0, 50.0)
        );
    }

    #[test]
    fn mirroring_x_reflects_across_the_middle_column() {
        assert_eq!(
            space(true, false).map(Vec2::new(1.0, 5.0)),
            Vec2::new(90.0, 50.0)
        );
    }

    #[test]
    fn one_mirror_alone_reverses_a_contour() {
        assert!(!space(false, false).reverses_winding());
        assert!(!space(true, true).reverses_winding(), "two reflections");
        assert!(space(true, false).reverses_winding());
        assert!(space(false, true).reverses_winding());
    }

    #[test]
    fn mirroring_y_flips_the_rows() {
        assert_eq!(space(false, true).map(Vec2::ZERO), Vec2::new(0.0, 50.0));
    }
}
