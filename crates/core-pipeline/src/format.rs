use std::path::Path;

use format_anycubic::{AnycubicFlavour, AnycubicVersion};
use format_chitu::{CbddlpFlavour, CtbVersion};
use format_creality::CxdlpVersion;
use format_sl1::Sl1Flavour;
use printer_profiles::{AnycubicExtension, OutputFormat, PhotonRevision};

/// A sliced-file format, and for `.ctb` the revision of it.
///
/// This is both what a front end offers and what the writer is chosen by. The output
/// name has the last word on the family; see
/// `docs/decisions/0047-the-output-extension-picks-the-format.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SlicedFormat {
    #[default]
    Goo,
    Ctb(CtbVersion),
    Cbddlp(CbddlpFlavour),
    Anycubic(AnycubicFlavour, AnycubicVersion),
    Sl1(Sl1Flavour),
    GcodeZip,
    Cxdlp(CxdlpVersion),
    Svgx,
    Cws,
}

impl SlicedFormat {
    /// The thirteen a front end offers, in the order it offers them.
    ///
    /// Only `.pwmx` of the Anycubic family is on the picker: the other sixteen extensions
    /// are the same file under another name, and a user who needs one types it (ADR 0047).
    pub const CHOICES: [Self; 13] = [
        Self::Goo,
        Self::Ctb(CtbVersion::V4),
        Self::Ctb(CtbVersion::V5),
        Self::Cbddlp(CbddlpFlavour::Cbddlp),
        Self::Cbddlp(CbddlpFlavour::Photon),
        Self::Anycubic(AnycubicFlavour::Pwmx, AnycubicVersion::V516),
        Self::Sl1(Sl1Flavour::Sl1),
        Self::Sl1(Sl1Flavour::Sl1s),
        Self::GcodeZip,
        Self::Cxdlp(CxdlpVersion::V3),
        Self::Cxdlp(CxdlpVersion::V4),
        Self::Svgx,
        Self::Cws,
    ];

    /// The format `path` names, at `version` when it names a `.ctb`, or `None` when it
    /// names no sliced file at all — a directory for a PNG stack, say.
    pub fn of(path: &Path, version: CtbVersion) -> Option<Self> {
        let extension = path.extension()?;
        if extension.eq_ignore_ascii_case("goo") {
            Some(Self::Goo)
        } else if extension.eq_ignore_ascii_case("ctb") {
            Some(Self::Ctb(version))
        } else if extension.eq_ignore_ascii_case("cbddlp") {
            Some(Self::Cbddlp(CbddlpFlavour::Cbddlp))
        } else if extension.eq_ignore_ascii_case("photon") {
            Some(Self::Cbddlp(CbddlpFlavour::Photon))
        } else if extension.eq_ignore_ascii_case("zip") {
            Some(Self::GcodeZip)
        } else if extension.eq_ignore_ascii_case("cxdlp") {
            // The name carries no revision either, and version 3 is what all but the
            // newest of these machines read.
            Some(Self::Cxdlp(CxdlpVersion::V3))
        } else if extension.eq_ignore_ascii_case("svgx") {
            Some(Self::Svgx)
        } else if extension.eq_ignore_ascii_case("cws") {
            Some(Self::Cws)
        } else {
            let extension = extension.to_string_lossy();
            Sl1Flavour::of_extension(&extension)
                .map(Self::Sl1)
                .or_else(|| {
                    // Nothing in a file name says which revision, so an extension typed by
                    // hand gets the newest its machines read.
                    AnycubicFlavour::of_extension(&extension)
                        .map(|flavour| Self::Anycubic(flavour, flavour.newest_version()))
                })
        }
    }

    /// This format at the revision `stated` names, where both name the same container.
    ///
    /// An output name has the last word on which container is written (ADR 0047) but it
    /// cannot carry a revision, so the profile keeps the one its machine reads.
    #[must_use]
    pub fn at_revision_of(self, stated: OutputFormat) -> Self {
        match (self, Self::from(stated)) {
            (Self::Anycubic(flavour, _), Self::Anycubic(named, revision)) if flavour == named => {
                Self::Anycubic(flavour, revision)
            }
            (Self::Cxdlp(_), Self::Cxdlp(revision)) => Self::Cxdlp(revision),
            _ => self,
        }
    }

    /// The revision a `.ctb` output is written at. Every other choice still has to name
    /// one, because a caller can put a `.ctb` name under it.
    pub fn ctb_version(self) -> CtbVersion {
        match self {
            Self::Ctb(version) => version,
            _ => CtbVersion::V4,
        }
    }

    /// Extension of the file this writes, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Goo => "goo",
            Self::Ctb(_) => "ctb",
            Self::Cbddlp(flavour) => flavour.extension(),
            Self::Anycubic(flavour, _) => flavour.extension(),
            Self::Sl1(flavour) => flavour.extension(),
            Self::GcodeZip => "zip",
            Self::Cxdlp(_) => "cxdlp",
            Self::Svgx => "svgx",
            Self::Cws => "cws",
        }
    }
}

impl From<OutputFormat> for SlicedFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Goo => Self::Goo,
            OutputFormat::Ctb4 => Self::Ctb(CtbVersion::V4),
            OutputFormat::Ctb5 => Self::Ctb(CtbVersion::V5),
            OutputFormat::Cbddlp => Self::Cbddlp(CbddlpFlavour::Cbddlp),
            OutputFormat::Photon => Self::Cbddlp(CbddlpFlavour::Photon),
            OutputFormat::Anycubic {
                extension,
                revision,
            } => Self::Anycubic(anycubic_flavour(extension), anycubic_version(revision)),
            OutputFormat::Sl1 => Self::Sl1(Sl1Flavour::Sl1),
            OutputFormat::Sl1s => Self::Sl1(Sl1Flavour::Sl1s),
            OutputFormat::Zip => Self::GcodeZip,
            OutputFormat::Cxdlp3 => Self::Cxdlp(CxdlpVersion::V3),
            OutputFormat::Cxdlp4 => Self::Cxdlp(CxdlpVersion::V4),
            OutputFormat::Svgx => Self::Svgx,
            OutputFormat::Cws => Self::Cws,
        }
    }
}

impl From<SlicedFormat> for OutputFormat {
    fn from(format: SlicedFormat) -> Self {
        match format {
            SlicedFormat::Goo => Self::Goo,
            SlicedFormat::Ctb(CtbVersion::V4) => Self::Ctb4,
            SlicedFormat::Ctb(CtbVersion::V5) => Self::Ctb5,
            SlicedFormat::Cbddlp(CbddlpFlavour::Cbddlp) => Self::Cbddlp,
            SlicedFormat::Cbddlp(CbddlpFlavour::Photon) => Self::Photon,
            SlicedFormat::Anycubic(flavour, version) => Self::Anycubic {
                extension: anycubic_extension(flavour),
                revision: photon_revision(version),
            },
            SlicedFormat::Sl1(Sl1Flavour::Sl1) => Self::Sl1,
            SlicedFormat::Sl1(Sl1Flavour::Sl1s) => Self::Sl1s,
            SlicedFormat::GcodeZip => Self::Zip,
            SlicedFormat::Cxdlp(CxdlpVersion::V3) => Self::Cxdlp3,
            SlicedFormat::Cxdlp(CxdlpVersion::V4) => Self::Cxdlp4,
            SlicedFormat::Svgx => Self::Svgx,
            SlicedFormat::Cws => Self::Cws,
        }
    }
}

fn anycubic_flavour(extension: AnycubicExtension) -> AnycubicFlavour {
    match extension {
        AnycubicExtension::Pwmx => AnycubicFlavour::Pwmx,
        AnycubicExtension::Pwmo => AnycubicFlavour::Pwmo,
        AnycubicExtension::Pwms => AnycubicFlavour::Pwms,
        AnycubicExtension::Pmsq => AnycubicFlavour::Pmsq,
        AnycubicExtension::Pw0 => AnycubicFlavour::Pw0,
        AnycubicExtension::Pwx => AnycubicFlavour::Pwx,
        AnycubicExtension::Dlp => AnycubicFlavour::Dlp,
        AnycubicExtension::Dl2p => AnycubicFlavour::Dl2p,
        AnycubicExtension::Pwma => AnycubicFlavour::Pwma,
        AnycubicExtension::Pwmb => AnycubicFlavour::Pwmb,
        AnycubicExtension::Px6s => AnycubicFlavour::Px6s,
        AnycubicExtension::Pmx2 => AnycubicFlavour::Pmx2,
        AnycubicExtension::Pm3n => AnycubicFlavour::Pm3n,
        AnycubicExtension::Pm3 => AnycubicFlavour::Pm3,
        AnycubicExtension::Pm3m => AnycubicFlavour::Pm3m,
        AnycubicExtension::Pm3r => AnycubicFlavour::Pm3r,
        AnycubicExtension::Pm5 => AnycubicFlavour::Pm5,
    }
}

fn anycubic_extension(flavour: AnycubicFlavour) -> AnycubicExtension {
    match flavour {
        AnycubicFlavour::Pwmx => AnycubicExtension::Pwmx,
        AnycubicFlavour::Pwmo => AnycubicExtension::Pwmo,
        AnycubicFlavour::Pwms => AnycubicExtension::Pwms,
        AnycubicFlavour::Pmsq => AnycubicExtension::Pmsq,
        AnycubicFlavour::Pw0 => AnycubicExtension::Pw0,
        AnycubicFlavour::Pwx => AnycubicExtension::Pwx,
        AnycubicFlavour::Dlp => AnycubicExtension::Dlp,
        AnycubicFlavour::Dl2p => AnycubicExtension::Dl2p,
        AnycubicFlavour::Pwma => AnycubicExtension::Pwma,
        AnycubicFlavour::Pwmb => AnycubicExtension::Pwmb,
        AnycubicFlavour::Px6s => AnycubicExtension::Px6s,
        AnycubicFlavour::Pmx2 => AnycubicExtension::Pmx2,
        AnycubicFlavour::Pm3n => AnycubicExtension::Pm3n,
        AnycubicFlavour::Pm3 => AnycubicExtension::Pm3,
        AnycubicFlavour::Pm3m => AnycubicExtension::Pm3m,
        AnycubicFlavour::Pm3r => AnycubicExtension::Pm3r,
        AnycubicFlavour::Pm5 => AnycubicExtension::Pm5,
    }
}

fn anycubic_version(revision: PhotonRevision) -> AnycubicVersion {
    match revision {
        PhotonRevision::V1 => AnycubicVersion::V1,
        PhotonRevision::V516 => AnycubicVersion::V516,
        PhotonRevision::V517 => AnycubicVersion::V517,
    }
}

fn photon_revision(version: AnycubicVersion) -> PhotonRevision {
    match version {
        AnycubicVersion::V1 => PhotonRevision::V1,
        AnycubicVersion::V516 => PhotonRevision::V516,
        AnycubicVersion::V517 => PhotonRevision::V517,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_decides_what_is_written() {
        let of = |name: &str| SlicedFormat::of(Path::new(name), CtbVersion::V4);
        assert_eq!(of("cube.goo"), Some(SlicedFormat::Goo));
        assert_eq!(of("cube.GOO"), Some(SlicedFormat::Goo));
        assert_eq!(of("cube.ctb"), Some(SlicedFormat::Ctb(CtbVersion::V4)));
        assert_eq!(
            of("cube.cbddlp"),
            Some(SlicedFormat::Cbddlp(CbddlpFlavour::Cbddlp))
        );
        assert_eq!(
            of("cube.PHOTON"),
            Some(SlicedFormat::Cbddlp(CbddlpFlavour::Photon))
        );
        assert_eq!(
            of("cube.pwmx"),
            Some(SlicedFormat::Anycubic(
                AnycubicFlavour::Pwmx,
                AnycubicVersion::V516
            ))
        );
        assert_eq!(
            of("cube.DLP"),
            Some(SlicedFormat::Anycubic(
                AnycubicFlavour::Dlp,
                AnycubicVersion::V516
            ))
        );
        assert_eq!(of("cube.sl1"), Some(SlicedFormat::Sl1(Sl1Flavour::Sl1)));
        assert_eq!(of("cube.ZIP"), Some(SlicedFormat::GcodeZip));
        assert_eq!(
            of("cube.cxdlp"),
            Some(SlicedFormat::Cxdlp(CxdlpVersion::V3))
        );
        assert_eq!(of("cube.svgx"), Some(SlicedFormat::Svgx));
        assert_eq!(of("cube.cws"), Some(SlicedFormat::Cws));
        assert_eq!(of("cube.sl1s"), Some(SlicedFormat::Sl1(Sl1Flavour::Sl1s)));
        assert_eq!(
            SlicedFormat::of(Path::new("cube.ctb"), CtbVersion::V5),
            Some(SlicedFormat::Ctb(CtbVersion::V5))
        );
        assert_eq!(
            of("out"),
            None,
            "a name without an extension is a PNG directory"
        );
        assert_eq!(of("out.d/cube"), None);
    }

    #[test]
    fn another_choice_still_names_a_revision_for_a_ctb_name_under_it() {
        assert_eq!(SlicedFormat::Goo.ctb_version(), CtbVersion::V4);
        assert_eq!(
            SlicedFormat::Cbddlp(CbddlpFlavour::Photon).ctb_version(),
            CtbVersion::V4
        );
        assert_eq!(
            SlicedFormat::Ctb(CtbVersion::V5).ctb_version(),
            CtbVersion::V5
        );
    }

    #[test]
    fn a_name_without_a_revision_takes_the_one_the_profile_states() {
        let named = SlicedFormat::Anycubic(AnycubicFlavour::Pwmb, AnycubicVersion::V517);
        let stated = OutputFormat::Anycubic {
            extension: AnycubicExtension::Pwmb,
            revision: PhotonRevision::V516,
        };
        assert_eq!(
            named.at_revision_of(stated),
            SlicedFormat::Anycubic(AnycubicFlavour::Pwmb, AnycubicVersion::V516)
        );
        assert_eq!(
            SlicedFormat::Goo.at_revision_of(stated),
            SlicedFormat::Goo,
            "a name of another container keeps what it named"
        );
        assert_eq!(
            SlicedFormat::Anycubic(AnycubicFlavour::Pm5, AnycubicVersion::V517)
                .at_revision_of(stated),
            SlicedFormat::Anycubic(AnycubicFlavour::Pm5, AnycubicVersion::V517),
            "another extension is another container"
        );
    }

    #[test]
    fn every_shipped_machine_names_a_container_a_writer_exists_for() {
        let catalogue =
            printer_profiles::Catalogue::bundled().expect("the shipped catalogue is valid");

        for entry in catalogue.printers() {
            let format = SlicedFormat::from(entry.profile.output);
            let named = format!("job.{}", format.extension());

            // The revision is not in a file name, so only the writer has to come back:
            // a profile is what states which revision of a container a machine reads.
            let routed = SlicedFormat::of(Path::new(&named), format.ctb_version())
                .unwrap_or_else(|| panic!("{} writes {named}, which routes nowhere", entry.id));
            assert_eq!(
                routed.extension(),
                format.extension(),
                "{} is written as {named}, which routes to another writer",
                entry.id
            );
        }
    }

    #[test]
    fn a_format_survives_the_round_trip_through_a_profile() {
        for format in SlicedFormat::CHOICES {
            let family: OutputFormat = format.into();
            assert_eq!(SlicedFormat::from(family), format);
        }
    }
}
