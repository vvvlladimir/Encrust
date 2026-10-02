use core_format::SlicedFile;

use crate::panels::Window;
use crate::ui::{duration, hint, readings, section};

/// What an opened sliced file says about itself, in place of the machine and resin sections
/// a plate being cut shows.
///
/// Every line is the file's own: a profile loaded in the window says nothing about a file
/// another slicer wrote, and a field the container has no place for is left out rather than
/// filled in from ours.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(facts) = window.machine.preview.read_facts() else {
        return;
    };
    let name = window
        .machine
        .preview
        .read_path()
        .and_then(|path| path.file_name())
        .map_or_else(
            || "Opened file".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );

    let rows = file_readings(facts);
    section(ui, &name, None, |ui| {
        readings(ui, &rows);
        if facts.display_mm.is_none() {
            hint(
                ui,
                "This container records no panel size, so an area cannot be worked out from it.",
            );
        }
    });
}

/// The rows the section lists, in the order a reader would want them.
fn file_readings(facts: &SlicedFile) -> Vec<(&'static str, String)> {
    let mut rows = vec![(
        "Format",
        match facts.version {
            Some(version) => format!(".{} v{version}", facts.format),
            None => format!(".{}", facts.format),
        },
    )];
    for (name, value) in [
        ("Machine", &facts.machine),
        ("Slicer", &facts.slicer),
        ("Resin", &facts.resin),
    ] {
        if let Some(value) = value {
            rows.push((name, value.clone()));
        }
    }

    rows.push((
        "Panel",
        match facts.display_mm {
            Some((width, height)) => format!(
                "{} x {} px, {width:.2} x {height:.2} mm",
                facts.width_px, facts.height_px
            ),
            None => format!("{} x {} px", facts.width_px, facts.height_px),
        },
    ));
    rows.push((
        "Stack",
        format!(
            "{} layers, {:.3} mm tall",
            facts.layer_count(),
            facts.height_mm()
        ),
    ));
    rows.push(("Layer height", format!("{:.4} mm", facts.layer_height_mm)));
    rows.push((
        "Exposure",
        format!(
            "{:.2} s, {:.2} s for the first {}",
            facts.exposure_s, facts.bottom_exposure_s, facts.bottom_layers
        ),
    ));
    rows.push(("Greys", facts.grey_steps.to_string()));

    if let Some(seconds) = facts.print_time_s {
        rows.push(("Print time", duration(seconds)));
    }
    if let Some(volume_mm3) = facts.volume_mm3 {
        rows.push(("States", format!("{:.3} ml of resin", volume_mm3 / 1000.0)));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_format::LayerEntry;

    fn facts() -> SlicedFile {
        SlicedFile {
            format: "ctb",
            version: Some(4),
            machine: Some("Mars 3 Pro".to_owned()),
            slicer: None,
            resin: None,
            width_px: 4098,
            height_px: 2560,
            display_mm: None,
            layer_height_mm: 0.05,
            exposure_s: 3.2,
            bottom_exposure_s: 38.0,
            bottom_layers: 6,
            print_time_s: Some(2695),
            volume_mm3: Some(1000.0),
            grey_steps: 128,
            layers: vec![LayerEntry {
                z_mm: 0.05,
                exposure_s: 3.2,
                offset: 0,
                size: 0,
            }],
        }
    }

    #[test]
    fn a_field_the_container_has_no_place_for_is_left_out() {
        let rows = file_readings(&facts());
        let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
        assert!(names.contains(&"Machine"));
        assert!(
            !names.contains(&"Slicer"),
            "a .ctb does not name the software that wrote it"
        );
    }

    #[test]
    fn the_panel_is_reported_in_millimetres_only_where_the_container_records_them() {
        let value = |facts: &SlicedFile| {
            file_readings(facts)
                .into_iter()
                .find(|(name, _)| *name == "Panel")
                .expect("every file states its resolution")
                .1
        };
        assert_eq!(value(&facts()), "4098 x 2560 px");
        assert_eq!(
            value(&SlicedFile {
                display_mm: Some((143.43, 89.6)),
                ..facts()
            }),
            "4098 x 2560 px, 143.43 x 89.60 mm"
        );
    }

    #[test]
    fn the_version_is_named_where_the_container_states_one() {
        let format_of = |facts: &SlicedFile| file_readings(facts)[0].1.clone();
        assert_eq!(format_of(&facts()), ".ctb v4");
        assert_eq!(
            format_of(&SlicedFile {
                format: "sl1",
                version: None,
                ..facts()
            }),
            ".sl1"
        );
    }
}
