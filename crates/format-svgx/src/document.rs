//! The SVG document the layers live in: the settings element at its head, one group per
//! layer, and the attributes a reader takes back out of either. See
//! `docs/formats/svgx.md`.

use core_format::PrintJob;

/// Digits the resin volume is written in, so that the one field a stack only settles at
/// the end can be filled in without moving a byte of the document.
const VOLUME_DIGITS: usize = 13;

/// What the document opens with, up to and including the empty background group.
///
/// The volume is a zero-padded field of a fixed width: everything behind it is written as
/// the layers come, and the measured resin is only known once they have.
pub(crate) fn head(job: &PrintJob) -> String {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);
    let (width_mm, height_mm) = (printer.display.width_mm, printer.display.height_mm);

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
            "<svg version=\"1.1\" xmlns=\"http://www.w3.org/2000/svg\">\n",
            "<printparams machinename=\"{machine}\" materialname=\"{resin}\" ",
            "layerheight=\"{layer_height:.3}\" volume=\"{volume}\" layercount=\"{layers}\" ",
            "lightintensity=\"1\" resolutionx=\"{width_px}\" resolutiony=\"{height_px}\" ",
            "displaywidth=\"{width_mm:.3}\" displayheight=\"{height_mm:.3}\" ",
            "machinez=\"{machine_z:.2}\">\n",
            "<projectiontime attachlayer=\"{bottom_layers}\" buildinlayer=\"{transition}\" ",
            "attachtime=\"{bottom_exposure:.2}\" basetime=\"{exposure:.2}\" />\n",
            "<projectionadjust x=\"100\" y=\"100\" />\n",
            "<printrange minx=\"{min_x:.3}\" miny=\"{min_y:.3}\" minz=\"0\" ",
            "maxx=\"{max_x:.3}\" maxy=\"{max_y:.3}\" maxz=\"{max_z:.3}\" />\n",
            "</printparams>\n",
            "<g id=\"background\">\n</g>\n",
        ),
        machine = escaped(printer.machine_name()),
        resin = escaped(&material.name),
        layer_height = job.nominal_height_mm(),
        volume = volume_field(job.volume_mm3 / 1000.0),
        layers = job.layer_count(),
        width_px = raster.width_px,
        height_px = raster.height_px,
        width_mm = width_mm,
        height_mm = height_mm,
        machine_z = printer.build_volume.z,
        bottom_layers = material.bottom_layers,
        transition = material.transition_layers,
        bottom_exposure = material.bottom_exposure_s,
        exposure = job.header_exposure_s(),
        min_x = -width_mm / 2.0,
        min_y = -height_mm / 2.0,
        max_x = width_mm / 2.0,
        max_y = height_mm / 2.0,
        max_z = job.height_mm(),
    )
}

/// What the document closes with.
pub(crate) const TAIL: &str = "</svg>\n";

/// Where the volume field begins inside `head`, so it can be written again on its own.
pub(crate) fn volume_offset(head: &str) -> Option<usize> {
    head.find("volume=\"").map(|at| at + "volume=\"".len())
}

/// The resin volume as the fixed-width field the head reserves, millilitres.
pub(crate) fn volume_field(millilitres: f32) -> String {
    let text = format!("{:0width$.3}", millilitres.max(0.0), width = VOLUME_DIGITS);
    text[text.len() - VOLUME_DIGITS..].to_owned()
}

/// Text as an attribute value may hold it.
fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The value of `name` in the first element of `xml` that carries it, with the standard
/// entities put back.
pub(crate) fn attribute(xml: &str, name: &str) -> Option<String> {
    let mut rest = xml;
    loop {
        let at = rest.find(name)?;
        let after = &rest[at + name.len()..];
        let before_is_space = rest[..at].ends_with([' ', '\n', '\t', '\r']);
        match after.strip_prefix('=') {
            Some(value) if before_is_space => {
                let quote = value.chars().next()?;
                if quote == '"' || quote == '\'' {
                    let end = value[1..].find(quote)?;
                    return Some(unescaped(&value[1..=end]));
                }
            }
            _ => {}
        }
        rest = after;
    }
}

/// The same for a number, where a value that is not one is as good as absent.
pub(crate) fn number(xml: &str, name: &str) -> Option<f32> {
    attribute(xml, name)?.trim().parse().ok()
}

fn unescaped(value: &str) -> String {
    if !value.contains('&') {
        return value.to_owned();
    }
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::sample_job;

    #[test]
    fn the_head_states_the_panel_and_the_exposures() {
        let mut job = sample_job(4);
        job.material.name = "Resin \"grey\" & co".to_owned();
        let head = head(&job);

        assert_eq!(number(&head, "resolutionx"), Some(8.0));
        assert_eq!(number(&head, "resolutiony"), Some(4.0));
        assert_eq!(number(&head, "layercount"), Some(4.0));
        assert_eq!(
            attribute(&head, "materialname").as_deref(),
            Some("Resin \"grey\" & co"),
            "a name with markup in it survives the round trip"
        );
        assert!(head.contains("<g id=\"background\">"));
    }

    #[test]
    fn the_volume_field_keeps_its_width_whatever_it_holds() {
        let empty = volume_field(0.0);
        assert_eq!(empty.len(), VOLUME_DIGITS);
        assert_eq!(volume_field(12.5).len(), VOLUME_DIGITS);
        assert_eq!(volume_field(1.0e12).len(), VOLUME_DIGITS);

        let head = head(&sample_job(1));
        let at = volume_offset(&head).expect("the head reserves the field");
        assert_eq!(&head[at..at + VOLUME_DIGITS], empty);
        assert_eq!(number(&head, "volume"), Some(0.0));
    }

    #[test]
    fn an_attribute_whose_name_ends_another_is_not_mistaken_for_it() {
        let xml = "<printparams displaywidth=\"192\" width=\"1\" />";
        assert_eq!(number(xml, "width"), Some(1.0));
        assert_eq!(number(xml, "displaywidth"), Some(192.0));
    }

    #[test]
    fn an_attribute_that_is_not_there_is_none() {
        assert_eq!(attribute("<g id=\"layer-0\">", "area"), None);
        assert_eq!(number("<g area=\"nothing\">", "area"), None);
    }
}
