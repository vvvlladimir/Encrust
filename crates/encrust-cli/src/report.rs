use std::fmt;
use std::path::PathBuf;

use core_format::ExposureRange;
use core_geometry::{Mended, MeshDiagnostics, Orientation, Scalar, Vec3, Welded};
use printer_profiles::PrinterProfile;

use crate::stats::MeshStats;

/// Whether the model fits the machine it is meant for, and whether it stands inside it.
pub struct FitCheck {
    pub printer: String,
    /// How much larger than the build envelope the model is, per axis, millimetres.
    pub overflow: Vec3,
    /// How far the model reaches outside the envelope where it stands, per axis, mm.
    /// Whatever hangs off the plate is clipped off the masks.
    pub outside: Vec3,
}

/// How far past the envelope counts as past it, millimetres. Under a micron is float dust
/// from the placement, not a model hanging off the plate.
const PAST_THE_PLATE_MM: Scalar = 1.0e-3;

impl FitCheck {
    pub fn of(stats: &MeshStats, profile: &PrinterProfile) -> Self {
        let volume = &profile.build_volume;
        let plate = Vec3::new(volume.x, volume.y, volume.z);
        Self {
            printer: profile.name.clone(),
            overflow: past(stats.size() - plate),
            outside: past((-stats.min).max(stats.max - plate)),
        }
    }

    pub fn fits(&self) -> bool {
        self.overflow == Vec3::ZERO && self.outside == Vec3::ZERO
    }
}

/// What is left of `amount` once the axes inside the envelope, and the dust, are dropped.
fn past(amount: Vec3) -> Vec3 {
    Vec3::select(
        amount.cmpgt(Vec3::splat(PAST_THE_PLATE_MM)),
        amount,
        Vec3::ZERO,
    )
}

/// How a wrap went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wrapped {
    /// Every shell unioned by the exact engine; no face moved but where they crossed.
    Exactly,
    /// Through the field, on this lattice, mm: sharp edges rounded to it.
    OnLattice(Scalar),
    /// The field would have left the model worse, so only the repair stands.
    LeftOut,
}

impl Wrapped {
    /// The lattice it was sampled on, mm, where it was.
    pub fn voxel_mm(self) -> Option<Scalar> {
        match self {
            Self::OnLattice(voxel_mm) => Some(voxel_mm),
            Self::Exactly | Self::LeftOut => None,
        }
    }
}

fn describe_wrap(wrapped: Wrapped) -> String {
    match wrapped {
        Wrapped::Exactly => "exactly".to_owned(),
        Wrapped::OnLattice(voxel_mm) => {
            format!("on a {voxel_mm:.3} mm lattice, edges rounded to it")
        }
        Wrapped::LeftOut => "left out: it would have left the model worse".to_owned(),
    }
}

/// Everything import learned about a model, ready to print.
pub struct ImportReport {
    pub path: PathBuf,
    pub stats: MeshStats,
    pub welded: Welded,
    /// Faces laid over others on the same corners that were dropped.
    pub doubled: usize,
    pub diagnostics: Option<MeshDiagnostics>,
    pub orientation: Option<Orientation>,
    /// What `--repair` changed, when it was asked for.
    pub mended: Option<Mended>,
    /// How `--repair=wrap` or `=deep` went, when asked.
    pub wrapped: Option<Wrapped>,
    /// Pairs of faces crossing each other, when `--intersections` asked for them.
    pub crossing_pairs: Option<usize>,
    pub fit: Option<FitCheck>,
}

impl ImportReport {
    /// True when nothing found would stop the model printing correctly.
    pub fn is_clean(&self) -> bool {
        let sound = self
            .diagnostics
            .as_ref()
            .is_none_or(MeshDiagnostics::is_sound);
        let fits = self.fit.as_ref().is_none_or(FitCheck::fits);
        sound && fits
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
        let dropped: Vec<String> = [
            (self.welded.faces_removed(), "degenerate"),
            (self.doubled, "doubled"),
        ]
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, what)| format!("{count} {what}"))
        .collect();
        if !dropped.is_empty() {
            write!(f, " ({} dropped)", dropped.join(", "))?;
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
                write!(f, "  volume        {:.3} mm^3", stats.volume)?;
                if stats.shells_overlap {
                    // The sum of the shells, since nothing here unites them: ADR 0206.
                    write!(
                        f,
                        " ({} shells added up, so what they share counts twice)",
                        diagnostics.shells
                    )?;
                }
                writeln!(f)?;
            }
            writeln!(f, "  closed        {}", describe_closure(diagnostics))?;
            for defect in defects(diagnostics) {
                writeln!(f, "  defect        {defect}")?;
            }
            let folded = diagnostics.folded_edges();
            if folded > 0 {
                // Sliced as they stand, so a note and not a defect: ADR 0269.
                writeln!(
                    f,
                    "  note          {folded} edges where faces lie folded together"
                )?;
            }
        }

        if let Some(orientation) = &self.orientation {
            writeln!(f, "  orientation   {}", describe_orientation(orientation))?;
        }

        if let Some(pairs) = self.crossing_pairs {
            writeln!(f, "  crossings     {pairs} pairs of faces")?;
        }

        if let Some(mended) = &self.mended {
            writeln!(f, "  repair        {}", describe_repair(mended))?;
            if let Some(wrapped) = self.wrapped {
                writeln!(f, "  wrapped       {}", describe_wrap(wrapped))?;
            }
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
        // Branching is reported here rather than as a defect: a seam two solids share
        // pairs off and closes, see ADR 0205.
        let branching = match diagnostics.non_manifold_edges {
            0 => String::new(),
            count => format!(", {count} branching edges"),
        };
        format!(
            "yes ({shells} {plural}, euler {}{branching})",
            diagnostics.euler_characteristic
        )
    } else {
        format!(
            "no ({shells} {plural}, {} edges that do not close)",
            diagnostics.unbalanced_edges
        )
    }
}

fn defects(diagnostics: &MeshDiagnostics) -> Vec<String> {
    let counts = [
        (diagnostics.boundary_edges, "open edges"),
        (diagnostics.degenerate_faces, "degenerate faces"),
        (diagnostics.duplicate_faces, "duplicate faces"),
        (diagnostics.unreferenced_vertices, "unused vertices"),
    ];
    counts
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .chain((diagnostics.holes > 0).then(|| match diagnostics.holes {
            1 => "1 hole".to_owned(),
            holes => format!("{holes} holes"),
        }))
        .collect()
}

fn describe_orientation(orientation: &Orientation) -> String {
    if !orientation.orientable {
        return match orientation.flipped_faces {
            0 => "left as found where no turning mends it".to_owned(),
            flipped => format!("fixed {flipped} inverted faces, the rest left as found"),
        };
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

fn describe_repair(mended: &Mended) -> String {
    if mended.kept_as_found {
        return "left as found, since repairing would leave more wrong".to_owned();
    }
    let holes = |count: usize| if count == 1 { "hole" } else { "holes" };
    let (closed, left) = (mended.filled.loops_filled, mended.filled.loops_left);
    let mut done = Vec::new();
    if closed > 0 {
        done.push(format!("closed {closed} {}", holes(closed)));
    }
    let seams = mended.stitched.edges_split;
    if seams > 0 {
        let plural = if seams == 1 { "seam" } else { "seams" };
        done.push(format!("stitched {seams} {plural}"));
    }
    if mended.shells_dropped > 0 {
        let plural = if mended.shells_dropped == 1 {
            "shell"
        } else {
            "shells"
        };
        done.push(format!("dropped {} small {plural}", mended.shells_dropped));
    }
    if mended.duplicates_removed > 0 {
        done.push(format!(
            "dropped {} doubled faces",
            mended.duplicates_removed
        ));
    }
    if left > 0 {
        done.push(format!("{left} {} left open", holes(left)));
    }
    if done.is_empty() {
        return "nothing to repair".to_owned();
    }
    done.join(", ")
}

/// An axis the model is too big for is named as that; one it merely stands off is named
/// as where it stands, because the two ask for different things of the user.
fn describe_fit(fit: &FitCheck) -> String {
    if fit.fits() {
        return format!("{}: yes", fit.printer);
    }
    let mut over = Vec::new();
    let mut outside = Vec::new();
    for (axis, index) in [("X", 0), ("Y", 1), ("Z", 2)] {
        if fit.overflow[index] > 0.0 {
            over.push(format!("{axis} by {:.3} mm", fit.overflow[index]));
        } else if fit.outside[index] > 0.0 {
            outside.push(format!("{axis} by {:.3} mm", fit.outside[index]));
        }
    }
    let mut parts = Vec::new();
    if !over.is_empty() {
        parts.push(format!("over {}", over.join(", ")));
    }
    if !outside.is_empty() {
        parts.push(format!("off the plate in {}", outside.join(", ")));
    }
    format!("{}: no, {}", fit.printer, parts.join(", "))
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
        standing(Vec3::ZERO, size)
    }

    fn standing(min: Vec3, max: Vec3) -> MeshStats {
        MeshStats {
            vertices: 0,
            faces: 0,
            min,
            max,
            surface_area: 0.0,
            volume: 0.0,
            shells_overlap: false,
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
    fn a_model_small_enough_but_off_the_edge_does_not_fit() {
        let fit = FitCheck::of(
            &standing(Vec3::new(-3.0, 5.0, 0.0), Vec3::new(7.0, 15.0, 10.0)),
            &profile(20.0, 20.0, 20.0),
        );
        assert!(
            !fit.fits(),
            "what hangs off the plate is clipped off the mask"
        );
        assert_eq!(
            describe_fit(&fit),
            "Test: no, off the plate in X by 3.000 mm"
        );
    }

    #[test]
    fn float_dust_under_the_plate_is_not_a_model_off_it() {
        let fit = FitCheck::of(
            &standing(Vec3::new(0.0, 0.0, -1.0e-5), Vec3::new(10.0, 10.0, 10.0)),
            &profile(20.0, 20.0, 20.0),
        );
        assert!(fit.fits(), "{} mm is placement dust", 1.0e-5);
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
