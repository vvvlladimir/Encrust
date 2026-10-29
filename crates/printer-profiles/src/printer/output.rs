//! The sliced-file format a machine reads, and the extension and revision it reads it under.

use serde::{Deserialize, Serialize};

/// The sliced-file format a machine's firmware reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Elegoo's `.goo`.
    #[default]
    Goo,
    /// The same extension at revision 5.1, which the Jupiter 2 reads.
    Goo5,
    /// The same container under the extension Phrozen's newer machines read, `.prz`.
    Prz,
    /// Chitu's `.ctb`, revision 4.
    Ctb4,
    /// Chitu's `.ctb`, revision 5.
    Ctb5,
    /// Chitu's encrypted `.ctb`, which the newer Chitu boards take and nothing else.
    CtbEncrypted,
    /// Chitu's older `.cbddlp`.
    Cbddlp,
    /// The same container under the extension Anycubic's early machines read, `.photon`.
    Photon,
    /// Anycubic's Photon Workshop container, under the extension of the machine that
    /// reads it and at the revision that machine's firmware takes.
    Anycubic {
        extension: AnycubicExtension,
        revision: PhotonRevision,
    },
    /// Prusa's `.sl1`.
    Sl1,
    /// The same container under the extension the SL1S Speed reads.
    Sl1s,
    /// The `.zip` of greyscale PNGs a Chitu board runs as gcode.
    Zip,
    /// The same archive under the extension Carima's machines read, `.crmaslice`.
    Crmaslice,
    /// Creality's `.cxdlp`, revision 3.
    Cxdlp3,
    /// The same container at revision 4, which the Halot Mage line reads.
    Cxdlp4,
    /// The `.svgx`, whose layers are polygons rather than pixels.
    Svgx,
    /// The `.cws` archive of greyscale PNGs and the gcode that runs them.
    Cws,
    /// The `.fdg` Voxelab's Chitu boards read: one bit a pixel.
    Fdg,
    /// The `.photons` the Photon S reads: a header of doubles and one bit a pixel.
    Photons,
    /// The `.phz` the Sonic Mini reads: the `.fdg` layout under a header of its own.
    Phz,
    /// The `.cxline` the D158 and D160 read: the `.cxdlp` header over layers of polygons.
    Cxline,
    /// Anycubic's PWSZ archive, under the extension of the machine that reads it.
    Pwsz { extension: PwszExtension },
}

/// Which extension an Anycubic machine's firmware looks for; all are one container,
/// see `docs/formats/anycubic.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnycubicExtension {
    #[default]
    Pwmx,
    Pwmo,
    Pwms,
    Pmsq,
    Pw0,
    Pwx,
    Dlp,
    Dl2p,
    Pwma,
    Pwmb,
    Px6s,
    Pmx2,
    Pm3n,
    Pm3,
    Pm3m,
    Pm3r,
    Pm5,
    Pm5s,
    M5sp,
    Pm4n,
}

impl AnycubicExtension {
    /// What the extension is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pwmx => ".pwmx",
            Self::Pwmo => ".pwmo",
            Self::Pwms => ".pwms",
            Self::Pmsq => ".pmsq",
            Self::Pw0 => ".pw0",
            Self::Pwx => ".pwx",
            Self::Dlp => ".dlp",
            Self::Dl2p => ".dl2p",
            Self::Pwma => ".pwma",
            Self::Pwmb => ".pwmb",
            Self::Px6s => ".px6s",
            Self::Pmx2 => ".pmx2",
            Self::Pm3n => ".pm3n",
            Self::Pm3 => ".pm3",
            Self::Pm3m => ".pm3m",
            Self::Pm3r => ".pm3r",
            Self::Pm5 => ".pm5",
            Self::Pm5s => ".pm5s",
            Self::M5sp => ".m5sp",
            Self::Pm4n => ".pm4n",
        }
    }

    /// The extension itself, without the dot.
    pub fn as_str(self) -> &'static str {
        &self.label()[1..]
    }
}

/// Which extension a machine reading the PWSZ archive looks for; see
/// `docs/formats/pwsz.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PwszExtension {
    #[default]
    Pwsz,
    Pm7,
    Pm7m,
    Pm4u,
    Pp1m,
}

impl PwszExtension {
    /// What the extension is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pwsz => ".pwsz",
            Self::Pm7 => ".pm7",
            Self::Pm7m => ".pm7m",
            Self::Pm4u => ".pm4u",
            Self::Pp1m => ".pp1m",
        }
    }
}

/// Which revision of the Photon Workshop container a machine's firmware reads, stated per machine
/// as one extension can be read at two revisions (ADR 0166).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhotonRevision {
    #[default]
    V1,
    V515,
    V516,
    V517,
    V518,
}

impl OutputFormat {
    /// What the format is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Goo => ".goo",
            Self::Goo5 => ".goo v5.1",
            Self::Prz => ".prz",
            Self::Ctb4 => ".ctb v4",
            Self::Ctb5 => ".ctb v5",
            Self::CtbEncrypted => ".ctb encrypted",
            Self::Cbddlp => ".cbddlp",
            Self::Photon => ".photon",
            Self::Anycubic { extension, .. } => extension.label(),
            Self::Sl1 => ".sl1",
            Self::Sl1s => ".sl1s",
            Self::Zip => ".zip",
            Self::Crmaslice => ".crmaslice",
            Self::Cxdlp3 => ".cxdlp",
            Self::Cxdlp4 => ".cxdlpv4",
            Self::Svgx => ".svgx",
            Self::Cws => ".cws",
            Self::Fdg => ".fdg",
            Self::Photons => ".photons",
            Self::Phz => ".phz",
            Self::Cxline => ".cxline",
            Self::Pwsz { extension } => extension.label(),
        }
    }
}
