use glam::Vec2;

use crate::RasterSettings;

/// Maps model millimetres onto the display's pixel grid; row `0` is the near edge. The panel's
/// mirroring is applied here, not in the geometry.
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

    /// Whether the map turns a contour the other way round. Each panel mirror is one reflection;
    /// the parity of the two decides, since material is positive winding.
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

    /// The inverse of `map` in millimetres: a point `panel_mm` off the first pixel, back on
    /// the plate.
    pub(crate) fn plate_mm(&self, panel_mm: Vec2) -> Vec2 {
        let px = panel_mm / self.pitch;
        let x = if self.mirror_x {
            self.width_px - px.x
        } else {
            px.x
        };
        let y = if self.mirror_y {
            self.height_px - px.y
        } else {
            px.y
        };
        Vec2::new(x, y) * self.pitch
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
    fn a_point_read_off_a_panel_mirrored_in_x_comes_back_across_the_plate() {
        // 1 mm off the first pixel of a 10 mm wide panel drawn mirrored is 9 mm from the left.
        let found = space(true, false).plate_mm(Vec2::new(1.0, 2.0));
        assert!(
            (found - Vec2::new(9.0, 2.0)).length() < 1e-5,
            "expected (9, 2) mm, found {found}"
        );
    }

    #[test]
    fn a_plate_point_drawn_then_read_back_is_where_it_was_on_every_panel() {
        let plate = Vec2::new(2.5, 3.1);
        for (mirror_x, mirror_y) in [(false, false), (true, false), (false, true), (true, true)] {
            let space = space(mirror_x, mirror_y);
            let panel_mm = space.map(plate) * Vec2::new(0.1, 0.1);
            let found = space.plate_mm(panel_mm);
            assert!(
                (found - plate).length() < 1e-5,
                "mirrored {mirror_x}, {mirror_y}: expected (2.5, 3.1) mm, found {found}"
            );
        }
    }

    #[test]
    fn mirroring_y_flips_the_rows() {
        assert_eq!(space(false, true).map(Vec2::ZERO), Vec2::new(0.0, 50.0));
    }
}
