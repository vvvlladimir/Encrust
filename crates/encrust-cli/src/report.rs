use std::fmt;
use std::path::PathBuf;

use core_format::ExposureRange;
use core_geometry::{MeshDiagnostics, Orientation, Scalar, Vec3, Welded};
use printer_profiles::PrinterProfile;

use crate::stats::MeshStats;

/// Whether the model fits the machine it is meant for.
pub struct FitCheck {
    pub printer: String,
    pub overflow: Vec3,
}

impl FitCheck {
    pub fn of(stats: &MeshStats, profile: &PrinterProfile) -> Self {
        let volume = &profile.build_volume;
        let size = stats.size();
        Self {
            printer: profile.name.clone(),
            overflow: Vec3::new(size.x - volume.x, size.y - volume.y, size.z - volume.z)
                .max(Vec3::ZERO),
        }
    }

    pub fn fits(&self) -> bool {
        self.overflow == Vec3::ZERO
    }
}

/// Everything import learned about a model, ready to print.
pub struct ImportReport {
    pub path: PathBuf,
    pub stats: MeshStats,
    pub welded: Welded,
    pub diagnostics: Option<MeshDiagnostics>,
    pub orientation: Option<Orientation>,
    pub fit: Option<FitCheck>,
}

impl ImportReport {
    /// True when nothing found would stop the model printing correctly.
    pub fn is_clean(&self) -> bool {
        let sound = self
            .diagnostics
            .as_ref()
            .is_none_or(MeshDiagnostics::is_sound);
        let orientable = self.orientation.as_ref().is_none_or(|o| o.orientable);
        let fits = self.fit.as_ref().is_none_or(FitCheck::fits);
        sound && orientable && fits
    }
}

impl fmt::Display for ImportReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stats = &self.stats;
        let size = stats.size();

        writeln!(f, "{}", self.path.display())?;
        write!(f, "  vertices      {}", stats.vertices)?;
        if self.welded.vertices_merged > 0 {
            let before = stats.vertices + self.welded.vertices_merged;
            write!(f, " (welded from {before})")?;
        }
        writeln!(f)?;

        write!(f, "  triangles     {}", stats.faces)?;
        if self.welded.faces_removed() > 0 {
            write!(f, " ({} degenerate dropped)", self.welded.faces_removed())?;
        }
        writeln!(f)?;

        writeln!(
            f,
            "  size          {:.3} x {:.3} x {:.3} mm",
            size.x, size.y, size.z
        )?;
        writeln!(
            f,
            "  bounds        [{:.3}, {:.3}, {:.3}] .. [{:.3}, {:.3}, {:.3}] mm",
            stats.min.x, stats.min.y, stats.min.z, stats.max.x, stats.max.y, stats.max.z
        )?;
        writeln!(f, "  surface area  {:.3} mm^2", stats.surface_area)?;

        if let Some(diagnostics) = &self.diagnostics {
            if diagnostics.is_closed() {
                writeln!(f, "  volume        {:.3} mm^3", stats.volume)?;
            }
            writeln!(f, "  closed        {}", describe_closure(diagnostics))?;
            for defect in defects(diagnostics) {
                writeln!(f, "  defect        {defect}")?;
            }
        }

        if let Some(orientation) = &self.orientation {
            writeln!(f, "  orientation   {}", describe_orientation(orientation))?;
        }

        if let Some(fit) = &self.fit {
            writeln!(f, "  fits          {}", describe_fit(fit))?;
        }
        Ok(())
    }
}

fn describe_closure(diagnostics: &MeshDiagnostics) -> String {
    let shells = diagnostics.shells;
    let plural = if shells == 1 { "shell" } else { "shells" };
    if diagnostics.is_closed() {
        format!(
            "yes ({shells} {plural}, euler {})",
            diagnostics.euler_characteristic
        )
    } else {
        format!(
            "no ({shells} {plural}, {} open edges)",
            diagnostics.boundary_edges
        )
    }
}

fn defects(diagnostics: &MeshDiagnostics) -> Vec<String> {
    let counts = [
        (diagnostics.boundary_edges, "open edges"),
        (diagnostics.non_manifold_edges, "non-manifold edges"),
        (diagnostics.degenerate_faces, "degenerate faces"),
        (diagnostics.duplicate_faces, "duplicate faces"),
        (diagnostics.unreferenced_vertices, "unused vertices"),
    ];
    counts
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect()
}

fn describe_orientation(orientation: &Orientation) -> String {
    if !orientation.orientable {
        return "not orientable, winding left as found".to_owned();
    }
    if orientation.inverted_shells > 0 {
        let shells = orientation.inverted_shells;
        let plural = if shells == 1 { "shell" } else { "shells" };
        return format!("turned {shells} inside-out {plural} outwards");
    }
    if orientation.flipped_faces > 0 {
        return format!("fixed {} inverted faces", orientation.flipped_faces);
    }
    "consistent".to_owned()
}

fn describe_fit(fit: &FitCheck) -> String {
    if fit.fits() {
        return format!("{}: yes", fit.printer);
    }
    let over = fit.overflow;
    let axes: Vec<String> = [("X", over.x), ("Y", over.y), ("Z", over.z)]
        .into_iter()
        .filter(|(_, amount)| *amount > 0.0)
        .map(|(axis, amount)| format!("{axis} by {amount:.3} mm"))
        .collect();
    format!("{}: no, over {}", fit.printer, axes.join(", "))
}

/// Parses `1.5` as uniform and `1,2,3` as per-axis.
pub fn parse_vec3(text: &str, uniform: bool) -> Result<Vec3, String> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let parse = |value: &str| {
        value
            .parse::<Scalar>()
            .map_err(|_| format!("{value:?} is not a number"))
    };

    match parts.as_slice() {
        [single] if uniform => Ok(Vec3::splat(parse(single)?)),
        [x, y, z] => Ok(Vec3::new(parse(x)?, parse(y)?, parse(z)?)),
        _ if uniform => Err("expected one number or three separated by commas".to_owned()),
        _ => Err("expected three numbers separated by commas".to_owned()),
    }
}

/// Shorthand for `--scale`, which accepts a single factor.
pub fn parse_scale(text: &str) -> Result<Vec3, String> {
    parse_vec3(text, true)
}

/// Shorthand for `--rotate`, which needs all three angles.
pub fn parse_rotation(text: &str) -> Result<Vec3, String> {
    parse_vec3(text, false)
}

/// Parses `FROM:TO:SECONDS` for `--exposure-at`, all three in millimetres and seconds.
pub fn parse_exposure_band(text: &str) -> Result<ExposureRange, String> {
    let parts: Vec<&str> = text.split(':').map(str::trim).collect();
    let [from, to, exposure] = parts.as_slice() else {
        return Err("expected FROM:TO:SECONDS".to_owned());
    };
    let parse = |value: &str| {
        value
            .parse::<f32>()
            .map_err(|_| format!("{value:?} is not a number"))
    };
    let range = ExposureRange::new(parse(from)?, parse(to)?, parse(exposure)?);
    if range.to_mm <= range.from_mm {
        return Err(format!(
            "a band from {} mm to {} mm holds no layers",
            range.from_mm, range.to_mm
        ));
    }
    if range.exposure_s <= 0.0 {
        return Err(format!("{} s is not an exposure", range.exposure_s));
    }
    Ok(range)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn an_exposure_band_parses_its_three_numbers() {
        let band = parse_exposure_band("2:10.5:4.25").expect("three numbers");
        assert!((band.from_mm - 2.0).abs() < 1e-6);
        assert!((band.to_mm - 10.5).abs() < 1e-6);
        assert!((band.exposure_s - 4.25).abs() < 1e-6);
    }

    #[test]
    fn an_exposure_band_needs_all_three_numbers() {
        assert!(parse_exposure_band("2:10").is_err());
        assert!(parse_exposure_band("2:ten:4").is_err());
    }

    #[test]
    fn an_exposure_band_that_holds_no_layers_is_refused() {
        assert!(parse_exposure_band("10:10:4").is_err());
        assert!(parse_exposure_band("10:2:4").is_err());
        assert!(parse_exposure_band("0:10:0").is_err());
    }

    fn profile(x: Scalar, y: Scalar, z: Scalar) -> PrinterProfile {
        let source = format!(
            r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 100
height_px = 50
width_mm = 10.0
height_mm = 5.0

[build_volume]
x = {x}
y = {y}
z = {z}
"#
        );
        PrinterProfile::from_toml_str(&source, Path::new("inline.toml")).expect("valid")
    }

    fn stats(size: Vec3) -> MeshStats {
        MeshStats {
            vertices: 0,
            faces: 0,
            min: Vec3::ZERO,
            max: size,
            surface_area: 0.0,
            volume: 0.0,
        }
    }

    #[test]
    fn a_model_inside_the_envelope_fits() {
        let fit = FitCheck::of(
            &stats(Vec3::new(10.0, 10.0, 10.0)),
            &profile(20.0, 20.0, 20.0),
        );
        assert!(fit.fits());
        assert_eq!(describe_fit(&fit), "Test: yes");
    }

    #[test]
    fn overflow_names_only_the_axes_that_overflow() {
        let fit = FitCheck::of(
            &stats(Vec3::new(30.0, 10.0, 25.0)),
            &profile(20.0, 20.0, 20.0),
        );
        assert!(!fit.fits());
        assert_eq!(
            describe_fit(&fit),
            "Test: no, over X by 10.000 mm, Z by 5.000 mm"
        );
    }

    #[test]
    fn scale_accepts_one_number_or_three() {
        assert_eq!(parse_scale("2").unwrap(), Vec3::splat(2.0));
        assert_eq!(parse_scale("1,2,3").unwrap(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn rotation_needs_all_three_angles() {
        assert!(parse_rotation("90").is_err());
        assert_eq!(
            parse_rotation("0, 0, 90").unwrap(),
            Vec3::new(0.0, 0.0, 90.0)
        );
    }

    #[test]
    fn a_non_numeric_component_is_rejected() {
        assert!(parse_scale("1,x,3").unwrap_err().contains("not a number"));
    }
}
