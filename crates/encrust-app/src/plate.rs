use core_geometry::{Aabb, Vec3};
use printer_profiles::PrinterProfile;

/// The build volume the viewport draws and models are placed on, in millimetres.
///
/// Named for the platform, not for what stands on it: what a run cuts and writes is
/// `core_engine::Plate`.
///
/// Plate coordinates put the origin at the front left corner of the plate, so a model
/// standing on it has `z >= 0` and `x`, `y` inside the plate extent. This is the same
/// convention the rasteriser uses, so a model that looks placed here rasterises placed.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildPlate {
    /// The printer this volume belongs to, or `None` until a profile is loaded. Not
    /// having a printer is a state, not a printer called "no printer".
    pub name: Option<String>,
    pub x_mm: f32,
    pub y_mm: f32,
    pub z_mm: f32,
}

impl Default for BuildPlate {
    /// A mid-sized MSLA envelope, used until a printer profile is loaded.
    fn default() -> Self {
        Self {
            name: None,
            x_mm: 150.0,
            y_mm: 80.0,
            z_mm: 165.0,
        }
    }
}

impl BuildPlate {
    pub fn from_profile(profile: &PrinterProfile) -> Self {
        Self {
            name: Some(format!("{} {}", profile.manufacturer, profile.name)),
            x_mm: profile.build_volume.x,
            y_mm: profile.build_volume.y,
            z_mm: profile.build_volume.z,
        }
    }

    /// What to call the plate on screen, which is an invitation while there is no
    /// profile behind it.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or("Select a printer")
    }

    /// Middle of the plate surface, the point the camera looks at when nothing is loaded.
    pub fn center(&self) -> Vec3 {
        Vec3::new(self.x_mm / 2.0, self.y_mm / 2.0, 0.0)
    }

    /// Longest straight line through the build volume, used to size the default view.
    pub fn diagonal_mm(&self) -> f32 {
        Vec3::new(self.x_mm, self.y_mm, self.z_mm).length()
    }

    /// Where `bounds` reaches furthest past the volume, or `None` while it stands inside.
    pub fn overreach(&self, bounds: &Aabb) -> Option<Overreach> {
        let room = Vec3::new(self.x_mm, self.y_mm, self.z_mm);
        let sides = [
            ("left", -bounds.mins.x),
            ("right", bounds.maxs.x - room.x),
            ("front", -bounds.mins.y),
            ("back", bounds.maxs.y - room.y),
            ("top", bounds.maxs.z - room.z),
        ];
        let (side, by_mm) = sides
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, by_mm)| *by_mm > OVERREACH_MM)?;
        let size = bounds.maxs - bounds.mins;
        let fits = size.cmple(room).all();
        let shift = fits.then(|| {
            let axis = |min: f32, max: f32, room: f32| (-min).max(0.0) - (max - room).max(0.0);
            Vec3::new(
                axis(bounds.mins.x, bounds.maxs.x, room.x),
                axis(bounds.mins.y, bounds.maxs.y, room.y),
                axis(bounds.mins.z, bounds.maxs.z, room.z),
            )
        });
        Some(Overreach { side, by_mm, shift })
    }
}

/// Past this a model is outside the volume rather than touching its wall, millimetres:
/// a model stood on the plate sits within rounding of zero.
const OVERREACH_MM: f32 = 1e-3;

/// How a model reaches past the build volume, and the move that brings it back in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overreach {
    /// The wall it is furthest past: left, right, front, back or top.
    pub side: &'static str,
    pub by_mm: f32,
    /// The smallest move that stands it inside, or `None` when it is bigger than the volume.
    pub shift: Option<Vec3>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SAMPLE: &str = r#"
name = "Test"
manufacturer = "Acme"

[display]
width_px = 100
height_px = 50
width_mm = 10.0
height_mm = 5.0

[build_volume]
x = 10.0
y = 5.0
z = 100.0
"#;

    #[test]
    fn a_profile_sets_the_build_volume() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, Path::new("inline.toml"))
            .expect("the sample profile is valid");
        let plate = BuildPlate::from_profile(&profile);
        assert_eq!(plate.name.as_deref(), Some("Acme Test"));
        assert_eq!((plate.x_mm, plate.y_mm, plate.z_mm), (10.0, 5.0, 100.0));
    }

    #[test]
    fn a_plate_without_a_profile_invites_one() {
        assert_eq!(BuildPlate::default().display_name(), "Select a printer");
    }

    #[test]
    fn the_center_sits_on_the_plate_surface() {
        let plate = BuildPlate {
            name: None,
            x_mm: 10.0,
            y_mm: 6.0,
            z_mm: 100.0,
        };
        assert_eq!(plate.center(), Vec3::new(5.0, 3.0, 0.0));
    }

    #[test]
    fn the_diagonal_is_the_box_diagonal() {
        let plate = BuildPlate {
            name: None,
            x_mm: 3.0,
            y_mm: 4.0,
            z_mm: 12.0,
        };
        // 3-4-12 is a Pythagorean quadruple: the space diagonal is exactly 13.
        assert!((plate.diagonal_mm() - 13.0).abs() < 1e-5);
    }

    fn ten_cube() -> BuildPlate {
        BuildPlate {
            name: None,
            x_mm: 10.0,
            y_mm: 10.0,
            z_mm: 10.0,
        }
    }

    #[test]
    fn a_box_inside_the_volume_does_not_overreach() {
        let inside = Aabb::new(Vec3::ZERO, Vec3::splat(10.0));
        assert_eq!(ten_cube().overreach(&inside), None);
    }

    #[test]
    fn a_box_past_the_right_wall_is_moved_back_by_as_much() {
        let past = Aabb::new(Vec3::new(8.0, 2.0, 0.0), Vec3::new(13.0, 4.0, 3.0));
        let found = ten_cube()
            .overreach(&past)
            .expect("it reaches 3 mm past x = 10");
        assert_eq!(found.side, "right");
        assert!((found.by_mm - 3.0).abs() < 1e-5);
        assert_eq!(found.shift, Some(Vec3::new(-3.0, 0.0, 0.0)));
    }

    #[test]
    fn a_box_wider_than_the_volume_has_no_way_in() {
        let wide = Aabb::new(Vec3::new(-1.0, 0.0, 0.0), Vec3::new(12.0, 1.0, 1.0));
        let found = ten_cube().overreach(&wide).expect("13 mm do not fit in 10");
        assert_eq!(
            found.side, "right",
            "2 mm past the right beats 1 mm past the left"
        );
        assert_eq!(found.shift, None);
    }
}
