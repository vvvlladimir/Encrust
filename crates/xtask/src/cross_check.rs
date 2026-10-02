//! The published machine list, read as a check on what was transcribed.
//!
//! The list is generated from the same source profiles, so it is a cross-check and never
//! the source: a panel that disagrees with it is a transcription slip.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use printer_profiles::PrinterProfile;

/// One row of the list: the panel and the travel, by machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Row {
    pub width_px: u32,
    pub height_px: u32,
    pub width_mm: f32,
    pub height_mm: f32,
    pub height_z_mm: f32,
}

/// Every row, keyed by brand and model with everything but letters and digits dropped,
/// because the list spells a model differently from the file name it came from.
pub struct MachineList(HashMap<String, Row>);

impl MachineList {
    pub fn load(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let rows = source.lines().filter_map(parse_row).collect();
        Ok(Self(rows))
    }

    /// What the list disagrees with, or `None` where it agrees or does not list the
    /// machine at all.
    pub fn disagreement(&self, profile: &PrinterProfile) -> Option<String> {
        let key = key(&profile.manufacturer, &profile.name);
        let row = self.0.get(&key)?;
        let ours = Row {
            width_px: profile.display.width_px,
            height_px: profile.display.height_px,
            width_mm: profile.display.width_mm,
            height_mm: profile.display.height_mm,
            height_z_mm: profile.build_volume.z,
        };
        (!agrees(&ours, row)).then(|| format!("{ours:?} against {row:?}"))
    }

    /// Whether the list carries this machine at all.
    pub fn lists(&self, profile: &PrinterProfile) -> bool {
        self.0
            .contains_key(&key(&profile.manufacturer, &profile.name))
    }
}

/// Two rows agree when the pixel counts match and the millimetres are within the micron
/// the list is written to.
fn agrees(ours: &Row, theirs: &Row) -> bool {
    const TOLERANCE_MM: f32 = 0.001;
    ours.width_px == theirs.width_px
        && ours.height_px == theirs.height_px
        && (ours.width_mm - theirs.width_mm).abs() <= TOLERANCE_MM
        && (ours.height_mm - theirs.height_mm).abs() <= TOLERANCE_MM
        && (ours.height_z_mm - theirs.height_z_mm).abs() <= TOLERANCE_MM
}

fn key(manufacturer: &str, model: &str) -> String {
    format!("{manufacturer}{model}")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// One row of the list, which names its brand, quotes its model and then gives five
/// numbers: the two pixel counts, the panel in millimetres and the travel.
fn parse_row(line: &str) -> Option<(String, Row)> {
    let (brand, rest) = line.split_once("PrinterBrand.")?.1.split_once(',')?;
    let (model, rest) = rest.split_once('"')?.1.split_once('"')?;
    let mut numbers = rest
        .trim_start_matches(',')
        .split(',')
        .map(|field| field.trim().trim_end_matches('f'));
    let mut next = || numbers.next()?.parse::<f32>().ok();
    let row = Row {
        width_px: next()? as u32,
        height_px: next()? as u32,
        width_mm: next()?,
        height_mm: next()?,
        height_z_mm: next()?,
    };
    Some((key(brand.trim(), model), row))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"
        new(PrinterBrand.Elegoo, "Mars 4 Ultra", 8520, 4320, 153.36f, 77.76f, 165f, FlipDirection.Horizontally),
        new(PrinterBrand.Uniformation, "GKtwo", 7680, 4320, 228.089f, 128.3f, 200f,
            FlipDirection.Vertically),
        not a row at all,
    "#;

    fn list() -> MachineList {
        MachineList(LIST.lines().filter_map(parse_row).collect())
    }

    fn profile() -> PrinterProfile {
        let mut profile = PrinterProfile {
            manufacturer: "Elegoo".to_owned(),
            name: "Mars 4 Ultra".to_owned(),
            ..PrinterProfile::default()
        };
        profile.display.width_px = 8520;
        profile.display.height_px = 4320;
        profile.display.width_mm = 153.36;
        profile.display.height_mm = 77.76;
        profile.build_volume.z = 165.0;
        profile
    }

    #[test]
    fn a_row_spread_over_two_lines_is_still_read() {
        assert_eq!(list().0.len(), 2, "the second row ends on the next line");
    }

    #[test]
    fn a_machine_the_list_agrees_with_reports_nothing() {
        assert_eq!(list().disagreement(&profile()), None);
        assert!(list().lists(&profile()));
    }

    #[test]
    fn a_panel_the_list_disagrees_with_is_reported() {
        let mut wrong = profile();
        wrong.display.width_mm = 143.36;
        assert!(list().disagreement(&wrong).is_some());
    }

    #[test]
    fn a_machine_the_list_does_not_carry_is_not_a_disagreement() {
        let mut absent = profile();
        absent.name = "Mars 9 Ultra".to_owned();
        assert_eq!(list().disagreement(&absent), None);
        assert!(!list().lists(&absent));
    }

    #[test]
    fn a_brand_spelt_differently_still_matches() {
        let mut gktwo = profile();
        gktwo.manufacturer = "UniFormation".to_owned();
        gktwo.name = "GKtwo".to_owned();
        gktwo.display.width_px = 7680;
        gktwo.display.height_px = 4320;
        gktwo.display.width_mm = 228.089;
        gktwo.display.height_mm = 128.3;
        gktwo.build_volume.z = 200.0;
        assert_eq!(list().disagreement(&gktwo), None);
    }
}
