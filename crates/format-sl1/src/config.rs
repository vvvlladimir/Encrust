use core_format::PrintJob;

use crate::writer::Sl1Flavour;

/// The file the machine's own firmware reads.
///
/// One exposure and one layer height for the whole stack, with the bottom block named by a
/// count rather than by its own numbers; see `docs/formats/sl1.md`.
pub(crate) fn config_ini(job: &PrintJob, flavour: Sl1Flavour, job_dir: &str) -> String {
    let material = &job.material;
    let mut ini = Ini::default();

    ini.text("action", "print");
    ini.text("jobDir", job_dir);
    ini.number("expTime", job.header_exposure_s());
    ini.number("expTimeFirst", material.bottom_exposure_s);
    ini.int("expUserProfile", 0);
    ini.text("fileCreationTimestamp", &job.created_utc());
    ini.int("hollow", 0);
    ini.number("layerHeight", job.nominal_height_mm());
    ini.text("materialName", &material.name);

    // The bottom block fades into the rest over this many layers, which is what our own
    // transition layers are. The reader takes the stack's height from the two counts
    // behind it: `numSlow + numFast`.
    ini.int("numFade", u32::from(material.transition_layers));
    ini.int("numFast", job.layer_count());
    ini.int("numSlow", 0);

    ini.text("printProfile", SLICER);
    ini.int("printTime", job.print_time_s());
    ini.text("printerModel", flavour.printer_model());
    ini.text("printerProfile", job.printer.machine_name());
    ini.text("printerVariant", "default");
    ini.text("prusaSlicerVersion", SLICER);
    ini.number("usedMaterial", job.volume_mm3 / 1000.0);
    ini.finish()
}

/// The file a slicer reads back, describing the machine the stack was cut for.
///
/// We write what we know and nothing else: an unknown key is one a reader skips, but a key
/// we invented a value for is one it believes.
pub(crate) fn prusaslicer_ini(job: &PrintJob, flavour: Sl1Flavour) -> String {
    let (printer, material, display) = (&job.printer, &job.material, &job.printer.display);
    let mut ini = Ini::default();

    ini.text("printer_technology", "SLA");
    ini.text("printer_model", flavour.printer_model());
    ini.text("printer_variant", "default");
    ini.text("printer_vendor", &printer.manufacturer);
    ini.text("printer_settings_id", printer.machine_name());
    ini.text("sla_archive_format", Sl1Flavour::ARCHIVE_FORMAT);

    ini.number("display_width", display.width_mm);
    ini.number("display_height", display.height_mm);
    ini.int("display_pixels_x", display.width_px);
    ini.int("display_pixels_y", display.height_px);
    ini.text(
        "display_orientation",
        if display.height_px >= display.width_px {
            "portrait"
        } else {
            "landscape"
        },
    );
    ini.int("display_mirror_x", u32::from(printer.mirror_x));
    ini.int("display_mirror_y", u32::from(printer.mirror_y));
    ini.number("max_print_height", printer.build_volume.z);
    ini.text("bed_shape", &bed_shape(job));

    ini.number("layer_height", job.nominal_height_mm());
    ini.number("initial_layer_height", job.nominal_height_mm());
    ini.number("exposure_time", job.header_exposure_s());
    ini.number("initial_exposure_time", material.bottom_exposure_s);
    ini.int("faded_layers", u32::from(material.transition_layers));
    ini.text("material_name", &material.name);
    ini.number("material_density", material.density_g_cm3);
    ini.text("thumbnails", &crate::writer::thumbnail_sizes());
    ini.finish()
}

/// The plate as the four corners of a rectangle, the shape the key states.
fn bed_shape(job: &PrintJob) -> String {
    let volume = &job.printer.build_volume;
    format!(
        "0x0,{x}x0,{x}x{y},0x{y}",
        x = trimmed(volume.x),
        y = trimmed(volume.y)
    )
}

/// What the files name us as, in both the profile fields and the version field: a reader
/// shows it to a user, and a machine ignores it.
const SLICER: &str = concat!("Encrust-", env!("CARGO_PKG_VERSION"));

/// A `key = value` file, in the order the keys were added.
#[derive(Default)]
struct Ini {
    lines: String,
}

impl Ini {
    fn text(&mut self, key: &str, value: &str) {
        self.lines.push_str(key);
        self.lines.push_str(" = ");
        self.lines.push_str(value);
        self.lines.push('\n');
    }

    fn int(&mut self, key: &str, value: u32) {
        self.text(key, &value.to_string());
    }

    fn number(&mut self, key: &str, value: f32) {
        self.text(key, &trimmed(value));
    }

    fn finish(self) -> String {
        self.lines
    }
}

/// A number without the trailing zeros a `{}` would leave, because these files are read by
/// people as often as by machines.
fn trimmed(value: f32) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() { "0" } else { text }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::sample_job;

    fn value_of<'a>(ini: &'a str, key: &str) -> Option<&'a str> {
        ini.lines()
            .find_map(|line| line.strip_prefix(&format!("{key} = ")))
    }

    #[test]
    fn the_stack_height_is_the_two_counts_a_reader_adds() {
        let ini = config_ini(&sample_job(7), Sl1Flavour::Sl1, "plate");
        assert_eq!(value_of(&ini, "numFast"), Some("7"));
        assert_eq!(
            value_of(&ini, "numSlow"),
            Some("0"),
            "a reader takes the layer count as numSlow plus numFast"
        );
        assert_eq!(value_of(&ini, "jobDir"), Some("plate"));
        assert_eq!(value_of(&ini, "action"), Some("print"));
    }

    #[test]
    fn the_bottom_block_fades_over_the_transition_layers() {
        let mut job = sample_job(7);
        job.material.transition_layers = 3;
        let ini = config_ini(&job, Sl1Flavour::Sl1, "plate");
        assert_eq!(value_of(&ini, "numFade"), Some("3"));
    }

    #[test]
    fn each_extension_names_its_own_machine() {
        let sl1 = prusaslicer_ini(&sample_job(1), Sl1Flavour::Sl1);
        let sl1s = prusaslicer_ini(&sample_job(1), Sl1Flavour::Sl1s);
        assert_eq!(value_of(&sl1, "printer_model"), Some("SL1"));
        assert_eq!(value_of(&sl1s, "printer_model"), Some("SL1S"));
        for ini in [&sl1, &sl1s] {
            assert_eq!(
                value_of(ini, "sla_archive_format"),
                Some("SL1"),
                "the key names the container, which is one format; a real .sl1s says SL1 too"
            );
        }
    }

    #[test]
    fn the_panel_is_described_in_both_millimetres_and_pixels() {
        let ini = prusaslicer_ini(&sample_job(1), Sl1Flavour::Sl1);
        assert_eq!(value_of(&ini, "display_pixels_x"), Some("8"));
        assert_eq!(value_of(&ini, "display_pixels_y"), Some("4"));
        assert_eq!(value_of(&ini, "display_width"), Some("0.8"));
        assert_eq!(value_of(&ini, "display_height"), Some("0.4"));
        assert_eq!(
            value_of(&ini, "display_orientation"),
            Some("landscape"),
            "the fixture panel is wider than it is tall"
        );
    }

    #[test]
    fn a_number_keeps_only_the_digits_it_needs() {
        assert_eq!(trimmed(0.05), "0.05");
        assert_eq!(trimmed(150.0), "150");
        assert_eq!(trimmed(0.0), "0");
    }
}
