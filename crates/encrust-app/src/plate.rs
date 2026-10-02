use core_geometry::Vec3;
use printer_profiles::PrinterProfile;

/// The build volume the viewport draws and models are placed on, in millimetres.
///
/// Plate coordinates put the origin at the front left corner of the plate, so a model
/// standing on it has `z >= 0` and `x`, `y` inside the plate extent. This is the same
/// convention the rasteriser uses, so a model that looks placed here rasterises placed.
#[derive(Debug, Clone, PartialEq)]
pub struct Plate {
    /// The printer this volume belongs to, or `None` until a profile is loaded. Not
    /// having a printer is a state, not a printer called "no printer".
    pub name: Option<String>,
    pub x_mm: f32,
    pub y_mm: f32,
    pub z_mm: f32,
}

impl Default for Plate {
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

impl Plate {
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
        let plate = Plate::from_profile(&profile);
        assert_eq!(plate.name.as_deref(), Some("Acme Test"));
        assert_eq!((plate.x_mm, plate.y_mm, plate.z_mm), (10.0, 5.0, 100.0));
    }

    #[test]
    fn a_plate_without_a_profile_invites_one() {
        assert_eq!(Plate::default().display_name(), "Select a printer");
    }

    #[test]
    fn the_center_sits_on_the_plate_surface() {
        let plate = Plate {
            name: None,
            x_mm: 10.0,
            y_mm: 6.0,
            z_mm: 100.0,
        };
        assert_eq!(plate.center(), Vec3::new(5.0, 3.0, 0.0));
    }

    #[test]
    fn the_diagonal_is_the_box_diagonal() {
        let plate = Plate {
            name: None,
            x_mm: 3.0,
            y_mm: 4.0,
            z_mm: 12.0,
        };
        // 3-4-12 is a Pythagorean quadruple: the space diagonal is exactly 13.
        assert!((plate.diagonal_mm() - 13.0).abs() < 1e-5);
    }
}
