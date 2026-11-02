//! What the window adds to `core_pipeline::SlicedFormat`: the words on the picker, and
//! the name a file dialog opens with.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use core_pipeline::{CbddlpFlavour, CtbVersion, CxdlpVersion, Sl1Flavour, SlicedFormat};

/// What the picker segment says. The revision is on it because it is the only thing
/// telling the two `.ctb` choices apart.
pub fn label_of(format: SlicedFormat) -> &'static str {
    match format {
        SlicedFormat::Goo => ".goo",
        SlicedFormat::Goo5 => ".goo v5.1",
        SlicedFormat::Prz => ".prz",
        SlicedFormat::Ctb(CtbVersion::V4) => ".ctb v4",
        SlicedFormat::Ctb(CtbVersion::V5) => ".ctb v5",
        SlicedFormat::CtbEncrypted => ".ctb encrypted",
        SlicedFormat::Cbddlp(CbddlpFlavour::Cbddlp) => ".cbddlp",
        SlicedFormat::Cbddlp(CbddlpFlavour::Photon) => ".photon",
        SlicedFormat::Anycubic(flavour, _) => flavour.label(),
        SlicedFormat::Sl1(Sl1Flavour::Sl1) => ".sl1",
        SlicedFormat::Sl1(Sl1Flavour::Sl1s) => ".sl1s",
        SlicedFormat::GcodeZip => ".zip",
        SlicedFormat::Crmaslice => ".crmaslice",
        SlicedFormat::Cxdlp(CxdlpVersion::V3) => ".cxdlp v3",
        SlicedFormat::Cxdlp(CxdlpVersion::V4) => ".cxdlp v4",
        SlicedFormat::Svgx => ".svgx",
        SlicedFormat::Cws => ".cws",
        SlicedFormat::Fdg => ".fdg",
        SlicedFormat::Photons => ".photons",
        SlicedFormat::Phz => ".phz",
        SlicedFormat::Cxline => ".cxline",
        SlicedFormat::Pwsz(flavour) => flavour.label(),
    }
}

/// `path` with `format`'s extension, unless it already names one.
///
/// A name the user typed an extension on keeps it: that is what decides the format, and
/// overriding it would write a `.goo` file called `plate.ctb`.
#[cfg(not(target_arch = "wasm32"))]
pub fn applied_to(format: SlicedFormat, path: PathBuf) -> PathBuf {
    match path.extension() {
        Some(_) => path,
        None => path.with_extension(format.extension()),
    }
}

/// The save dialog's file types: `format` first, under the revision it writes, then each other
/// extension once and bare, because a dialog hands back a name and not the row picked.
#[cfg(not(target_arch = "wasm32"))]
pub fn dialog_filters(
    format: SlicedFormat,
    offered: impl IntoIterator<Item = SlicedFormat>,
) -> Vec<(String, &'static str)> {
    let mut filters = vec![(label_of(format).to_owned(), format.extension())];
    for extension in offered.into_iter().map(SlicedFormat::extension) {
        if filters.iter().all(|(_, listed)| *listed != extension) {
            filters.push((format!(".{extension}"), extension));
        }
    }
    filters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_save_dialog_lists_each_extension_once_and_the_written_revision_first() {
        let filters = dialog_filters(SlicedFormat::Ctb(CtbVersion::V5), SlicedFormat::CHOICES);
        let extensions: Vec<_> = filters.iter().map(|(_, extension)| *extension).collect();
        let mut once = extensions.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(
            extensions.len(),
            once.len(),
            "a dialog hands back a name, not the row picked: {extensions:?}"
        );
        assert_eq!(filters[0], (".ctb v5".to_owned(), "ctb"));
        assert!(
            filters.contains(&(".goo".to_owned(), "goo")),
            "another container is named without a revision it would not get"
        );
    }

    #[test]
    fn a_name_without_an_extension_gets_the_chosen_one() {
        let ctb = SlicedFormat::Ctb(CtbVersion::V5);
        assert_eq!(
            applied_to(ctb, PathBuf::from("/tmp/plate")),
            PathBuf::from("/tmp/plate.ctb")
        );
        assert_eq!(
            applied_to(ctb, PathBuf::from("/tmp/plate.goo")),
            PathBuf::from("/tmp/plate.goo"),
            "an extension the user typed is what decides the format"
        );
    }

    #[test]
    fn every_choice_has_its_own_label() {
        let labels: Vec<_> = SlicedFormat::CHOICES.iter().map(|f| label_of(*f)).collect();
        assert_eq!(
            labels,
            vec![
                ".goo",
                ".goo v5.1",
                ".ctb v4",
                ".ctb v5",
                ".ctb encrypted",
                ".cbddlp",
                ".photon",
                ".pwmx",
                ".sl1",
                ".sl1s",
                ".zip",
                ".cxdlp v3",
                ".cxdlp v4",
                ".svgx",
                ".cws",
                ".fdg",
                ".photons",
                ".phz",
                ".cxline",
                ".pwsz"
            ]
        );
    }
}
