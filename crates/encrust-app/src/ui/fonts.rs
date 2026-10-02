use egui::{FontData, FontDefinitions, FontFamily};

use crate::ui::theme::{MEDIUM, SEMIBOLD};

/// The faces are compiled in rather than loaded from disk, so the window looks the same on
/// a machine that has none of them installed and needs no files beside the binary.
const GEIST: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/Geist-Regular.ttf"
));
const GEIST_MEDIUM: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/Geist-Medium.ttf"
));
const GEIST_SEMIBOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/Geist-SemiBold.ttf"
));
const GEIST_MONO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/GeistMono-Regular.ttf"
));

/// Geist for text, Geist Mono for numbers, Phosphor merged in as a fallback so an icon is
/// a glyph in an ordinary text run rather than a second widget.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    for (name, bytes) in [
        ("geist", GEIST),
        (MEDIUM, GEIST_MEDIUM),
        (SEMIBOLD, GEIST_SEMIBOLD),
        ("geist-mono", GEIST_MONO),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), FontData::from_static(bytes).into());
    }

    put_first(&mut fonts, FontFamily::Proportional, "geist");
    put_first(&mut fonts, FontFamily::Monospace, "geist-mono");
    for weight in [MEDIUM, SEMIBOLD] {
        let family = fonts
            .families
            .entry(FontFamily::Name(weight.into()))
            .or_default();
        family.insert(0, weight.to_owned());
    }

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    // The icons have to reach the numeric family too: a mono readout carries them.
    put_last(&mut fonts, FontFamily::Monospace, "phosphor");
    for weight in [MEDIUM, SEMIBOLD] {
        put_last(&mut fonts, FontFamily::Name(weight.into()), "phosphor");
    }

    fonts
}

/// Puts `name` ahead of whatever egui ships, keeping the rest as a fallback for the
/// glyphs Geist does not carry.
fn put_first(fonts: &mut FontDefinitions, family: FontFamily, name: &str) {
    fonts
        .families
        .entry(family)
        .or_default()
        .insert(0, name.to_owned());
}

fn put_last(fonts: &mut FontDefinitions, family: FontFamily, name: &str) {
    let family = fonts.families.entry(family).or_default();
    if !family.iter().any(|entry| entry == name) {
        family.push(name.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_family_starts_with_its_bundled_face_and_can_still_draw_an_icon() {
        let fonts = definitions();
        for (family, first) in [
            (FontFamily::Proportional, "geist"),
            (FontFamily::Monospace, "geist-mono"),
            (FontFamily::Name(MEDIUM.into()), MEDIUM),
            (FontFamily::Name(SEMIBOLD.into()), SEMIBOLD),
        ] {
            let names = fonts
                .families
                .get(&family)
                .unwrap_or_else(|| panic!("{family:?} is registered"));
            assert_eq!(names.first().map(String::as_str), Some(first));
            assert!(
                names.iter().any(|name| name == "phosphor"),
                "{family:?} can fall back to the icon font"
            );
        }
    }
}
