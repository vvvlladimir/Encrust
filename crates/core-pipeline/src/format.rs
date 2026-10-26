use std::path::Path;

use printer_profiles::{
    AnycubicExtension, OutputFormat, PhotonRevision, PrinterProfile, PwszExtension,
};

// The variants below carry them, so a front end naming a container gets the type from
// here and takes no dependency on the crate that writes it; the same reason
// `core-format` re-exports `LayerPlan` (ADR 0091, 0209).
pub use format_anycubic::{AnycubicFlavour, AnycubicVersion, PwszFlavour};
pub use format_chitu::{CbddlpFlavour, CtbVersion};
pub use format_creality::CxdlpVersion;
pub use format_sl1::Sl1Flavour;

/// A sliced-file format, and for `.ctb` the revision of it. What a front end offers and the writer
/// is chosen by; the output name has the last word (ADR 0047).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SlicedFormat {
    #[default]
    Goo,
    Ctb(CtbVersion),
    /// The encrypted `.ctb`, which an output name alone never picks: only a profile does.
    CtbEncrypted,
    Cbddlp(CbddlpFlavour),
    Anycubic(AnycubicFlavour, AnycubicVersion),
    Sl1(Sl1Flavour),
    GcodeZip,
    Cxdlp(CxdlpVersion),
    Svgx,
    Cws,
    Pwsz(PwszFlavour),
}

impl SlicedFormat {
    /// The fifteen a front end offers, in the order it offers them. Only `.pwmx` and `.pwsz` of
    /// the Anycubic families are on the picker; the others are renames (ADR 0047).
    pub const CHOICES: [Self; 15] = [
        Self::Goo,
        Self::Ctb(CtbVersion::V4),
        Self::Ctb(CtbVersion::V5),
        Self::CtbEncrypted,
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
        Self::Pwsz(PwszFlavour::Pwsz),
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
                .or_else(|| PwszFlavour::of_extension(&extension).map(Self::Pwsz))
        }
    }

    /// This format at the revision `stated` names, where both name the same container. An output
    /// name picks the container (ADR 0047) but not a revision, so the profile keeps its own.
    #[must_use]
    pub fn at_revision_of(self, stated: OutputFormat) -> Self {
        match (self, Self::from(stated)) {
            (Self::Anycubic(flavour, _), Self::Anycubic(named, revision)) if flavour == named => {
                Self::Anycubic(flavour, revision)
            }
            (Self::Cxdlp(_), Self::Cxdlp(revision)) => Self::Cxdlp(revision),
            (Self::Ctb(_), Self::CtbEncrypted) => Self::CtbEncrypted,
            _ => self,
        }
    }

    /// The choices `printer` can be written into, in picker order. Offer only these: a container
    /// the machine's fields cannot fill fails after the stack is cut (ADR 0210).
    pub fn choices_for(printer: &PrinterProfile) -> impl Iterator<Item = Self> + '_ {
        Self::CHOICES
            .into_iter()
            .filter(|choice| choice.writable_for(printer))
    }

    /// Whether a file of this format can carry what `printer` states. Only the `.cxdlp` family
    /// needs the machine's CL or CT code, so a name without one cannot be written.
    pub fn writable_for(self, printer: &PrinterProfile) -> bool {
        match self {
            Self::Cxdlp(_) => format_creality::model_code(printer.machine_name()).is_some(),
            _ => true,
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
            Self::Ctb(_) | Self::CtbEncrypted => "ctb",
            Self::Cbddlp(flavour) => flavour.extension(),
            Self::Anycubic(flavour, _) => flavour.extension(),
            Self::Sl1(flavour) => flavour.extension(),
            Self::GcodeZip => "zip",
            Self::Cxdlp(_) => "cxdlp",
            Self::Svgx => "svgx",
            Self::Cws => "cws",
            Self::Pwsz(flavour) => flavour.extension(),
        }
    }
}

impl From<OutputFormat> for SlicedFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Goo => Self::Goo,
            OutputFormat::Ctb4 => Self::Ctb(CtbVersion::V4),
            OutputFormat::Ctb5 => Self::Ctb(CtbVersion::V5),
            OutputFormat::CtbEncrypted => Self::CtbEncrypted,
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
            OutputFormat::Pwsz { extension } => Self::Pwsz(pwsz_flavour(extension)),
        }
    }
}

impl From<SlicedFormat> for OutputFormat {
    fn from(format: SlicedFormat) -> Self {
        match format {
            SlicedFormat::Goo => Self::Goo,
            SlicedFormat::Ctb(CtbVersion::V4) => Self::Ctb4,
            SlicedFormat::Ctb(CtbVersion::V5) => Self::Ctb5,
            SlicedFormat::CtbEncrypted => Self::CtbEncrypted,
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
            SlicedFormat::Pwsz(flavour) => Self::Pwsz {
                extension: pwsz_extension(flavour),
            },
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
        AnycubicExtension::Pm5s => AnycubicFlavour::Pm5s,
        AnycubicExtension::M5sp => AnycubicFlavour::M5sp,
        AnycubicExtension::Pm4n => AnycubicFlavour::Pm4n,
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
        AnycubicFlavour::Pm5s => AnycubicExtension::Pm5s,
        AnycubicFlavour::M5sp => AnycubicExtension::M5sp,
        AnycubicFlavour::Pm4n => AnycubicExtension::Pm4n,
    }
}

fn pwsz_flavour(extension: PwszExtension) -> PwszFlavour {
    match extension {
        PwszExtension::Pwsz => PwszFlavour::Pwsz,
        PwszExtension::Pm7 => PwszFlavour::Pm7,
        PwszExtension::Pm7m => PwszFlavour::Pm7m,
        PwszExtension::Pm4u => PwszFlavour::Pm4u,
        PwszExtension::Pp1m => PwszFlavour::Pp1m,
    }
}

fn pwsz_extension(flavour: PwszFlavour) -> PwszExtension {
    match flavour {
        PwszFlavour::Pwsz => PwszExtension::Pwsz,
        PwszFlavour::Pm7 => PwszExtension::Pm7,
        PwszFlavour::Pm7m => PwszExtension::Pm7m,
        PwszFlavour::Pm4u => PwszExtension::Pm4u,
        PwszFlavour::Pp1m => PwszExtension::Pp1m,
    }
}

fn anycubic_version(revision: PhotonRevision) -> AnycubicVersion {
    match revision {
        PhotonRevision::V1 => AnycubicVersion::V1,
        PhotonRevision::V516 => AnycubicVersion::V516,
        PhotonRevision::V517 => AnycubicVersion::V517,
        PhotonRevision::V518 => AnycubicVersion::V518,
    }
}

fn photon_revision(version: AnycubicVersion) -> PhotonRevision {
    match version {
        AnycubicVersion::V1 => PhotonRevision::V1,
        AnycubicVersion::V516 => PhotonRevision::V516,
        AnycubicVersion::V517 => PhotonRevision::V517,
        AnycubicVersion::V518 => PhotonRevision::V518,
    }
}

#[cfg(test)]
mod tests;
