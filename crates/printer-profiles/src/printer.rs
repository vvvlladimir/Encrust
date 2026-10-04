use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ProfileError;

/// The sliced-file format a machine's firmware reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Elegoo's `.goo`.
    #[default]
    Goo,
    /// Chitu's `.ctb`, revision 4.
    Ctb4,
    /// Chitu's `.ctb`, revision 5.
    Ctb5,
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
    /// Creality's `.cxdlp`, revision 3.
    Cxdlp3,
    /// The same container at revision 4, which the Halot Mage line reads.
    Cxdlp4,
    /// The `.svgx`, whose layers are polygons rather than pixels.
    Svgx,
    /// The `.cws` archive of greyscale PNGs and the gcode that runs them.
    Cws,
}

/// Which extension an Anycubic machine's firmware looks for.
///
/// Every one of them is the same container; see `docs/formats/anycubic.md`.
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
        }
    }

    /// The extension itself, without the dot.
    pub fn as_str(self) -> &'static str {
        &self.label()[1..]
    }
}

/// Which revision of the Photon Workshop container a machine's firmware reads.
///
/// One extension can be read at two revisions by two machines, so this is stated per
/// machine and not derived from the extension; see ADR 0166.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhotonRevision {
    #[default]
    V1,
    V516,
    V517,
}

impl OutputFormat {
    /// What the format is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Goo => ".goo",
            Self::Ctb4 => ".ctb v4",
            Self::Ctb5 => ".ctb v5",
            Self::Cbddlp => ".cbddlp",
            Self::Photon => ".photon",
            Self::Anycubic { extension, .. } => extension.label(),
            Self::Sl1 => ".sl1",
            Self::Sl1s => ".sl1s",
            Self::Zip => ".zip",
            Self::Cxdlp3 | Self::Cxdlp4 => ".cxdlp",
            Self::Svgx => ".svgx",
            Self::Cws => ".cws",
        }
    }
}

/// How a machine takes a sliced file over the network, where it takes one at all.
///
/// This is what the machine speaks, not how it is reached: the address and the
/// credentials are the window's own settings, see `docs/decisions/0153`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Connection {
    /// Nothing but a USB stick.
    #[default]
    None,
    /// SDCP, which Elegoo boards answer a broadcast on.
    Sdcp,
    /// `PrusaLink` over HTTP, which is typed in because it does not answer one.
    PrusaLink,
}

impl Connection {
    /// The three, in the order the form offers them.
    pub const ALL: [Self; 3] = [Self::None, Self::Sdcp, Self::PrusaLink];

    /// What the form calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "USB only",
            Self::Sdcp => "SDCP",
            Self::PrusaLink => "PrusaLink",
        }
    }
}

/// MSLA panel geometry.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Display {
    pub width_px: u32,
    pub height_px: u32,
    /// Illuminated area, millimetres.
    pub width_mm: f32,
    pub height_mm: f32,
    /// Dimmest grey this panel cures; a mask pixel below it is written black instead.
    /// Zero, the default, keeps every grey. See `docs/decisions/0139`.
    #[serde(default)]
    pub grey_floor: u8,
}

impl Display {
    /// Millimetres per pixel along X and Y.
    pub fn pixel_pitch_mm(&self) -> (f32, f32) {
        (
            self.width_mm / self.width_px as f32,
            self.height_mm / self.height_px as f32,
        )
    }
}

/// Usable build envelope, millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BuildVolume {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Everything about one printer model that the slicer needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterProfile {
    pub name: String,
    pub manufacturer: String,
    /// The machine name a sliced file must carry for this firmware to accept it, where
    /// that is not `name`. See `docs/decisions/0140`.
    #[serde(default)]
    pub machine_name: Option<String>,
    pub display: Display,
    pub build_volume: BuildVolume,
    /// Whether the panel is mirrored relative to model space.
    #[serde(default)]
    pub mirror_x: bool,
    #[serde(default)]
    pub mirror_y: bool,
    /// What this machine's firmware reads. A profile that says nothing gets `.goo`.
    #[serde(default)]
    pub output: OutputFormat,
    /// What this machine takes a file over, where it takes one at all.
    #[serde(default)]
    pub connection: Connection,
    /// What this machine's firmware will obey beyond the header.
    #[serde(default)]
    pub firmware: Firmware,
}

/// What a machine's firmware obeys beyond its header, which is not the same on every
/// machine and cannot be read off the file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    /// Whether it reads the per-layer tables rather than the header alone. Chitu 4.3.9
    /// and later do; an older machine has to be marked `false` by hand.
    #[serde(default = "reads_per_layer")]
    pub per_layer_settings: bool,
    /// Whether it moves the plate to each layer's own Z rather than stepping by the
    /// header's height. Many do not, so a machine has to be marked `true` by hand.
    #[serde(default)]
    pub variable_layer_height: bool,
}

fn reads_per_layer() -> bool {
    true
}

impl Default for Firmware {
    fn default() -> Self {
        Self {
            per_layer_settings: reads_per_layer(),
            variable_layer_height: false,
        }
    }
}

impl Default for PrinterProfile {
    /// A blank machine to start a new profile from: a 4K 6" panel, which is the most
    /// common MSLA screen there has ever been.
    fn default() -> Self {
        Self {
            name: "New printer".to_owned(),
            manufacturer: String::new(),
            machine_name: None,
            display: Display {
                width_px: 4096,
                height_px: 2560,
                width_mm: 143.36,
                height_mm: 89.6,
                grey_floor: 0,
            },
            build_volume: BuildVolume {
                x: 143.36,
                y: 89.6,
                z: 165.0,
            },
            mirror_x: true,
            mirror_y: false,
            output: OutputFormat::Goo,
            connection: Connection::None,
            firmware: Firmware::default(),
        }
    }
}

impl PrinterProfile {
    /// The machine name to write into a sliced file: what the firmware matches, which is
    /// the profile's own name unless it states otherwise.
    pub fn machine_name(&self) -> &str {
        self.machine_name.as_deref().unwrap_or(&self.name)
    }

    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        let profile: Self = toml::from_str(source).map_err(|source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Writes the profile as TOML, refusing to write one that would not load.
    pub fn save(&self, path: &Path) -> Result<(), ProfileError> {
        let source = self.to_toml_string(path)?;
        std::fs::write(path, source).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })
    }

    /// The profile as the TOML it is kept in; `path` is where it is going, for the error.
    pub fn to_toml_string(&self, path: &Path) -> Result<String, ProfileError> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|source| ProfileError::Serialise {
            path: path.to_owned(),
            source,
        })
    }

    pub fn load(path: &Path) -> Result<Self, ProfileError> {
        let source = std::fs::read_to_string(path).map_err(|source| ProfileError::Io {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml_str(&source, path)
    }

    fn validate(&self) -> Result<(), ProfileError> {
        let checks = [
            ("display.width_mm", self.display.width_mm),
            ("display.height_mm", self.display.height_mm),
            ("display.width_px", self.display.width_px as f32),
            ("display.height_px", self.display.height_px as f32),
            ("build_volume.z", self.build_volume.z),
        ];
        for (field, value) in checks {
            if value <= 0.0 {
                return Err(ProfileError::NonPositive { field, value });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
name = "Test"
manufacturer = "Test"

[display]
width_px = 100
height_px = 50
width_mm = 10.0
height_mm = 5.0

[build_volume]
x = 10.0
y = 5.0
z = 100.0
"#;

    fn path() -> &'static Path {
        Path::new("inline.toml")
    }

    #[test]
    fn a_profile_that_names_no_machine_writes_its_own_name() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
        assert_eq!(profile.machine_name(), "Test");

        let named = SAMPLE.replace(
            r#"manufacturer = "Test""#,
            "manufacturer = \"Test\"\nmachine_name = \"TEST Machine 1\"",
        );
        let profile = PrinterProfile::from_toml_str(&named, path()).expect("a valid profile");
        assert_eq!(profile.machine_name(), "TEST Machine 1");
        assert_eq!(
            profile.name, "Test",
            "the picker still shows the short name"
        );
    }

    #[test]
    fn a_profile_that_states_no_floor_keeps_every_grey() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
        assert_eq!(profile.display.grey_floor, 0);
    }

    #[test]
    fn a_profile_that_says_nothing_reads_its_per_layer_tables() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("a valid profile");
        assert!(profile.firmware.per_layer_settings);
        assert!(!profile.firmware.variable_layer_height);

        let older = format!("{SAMPLE}\n[firmware]\nper_layer_settings = false\n");
        let profile = PrinterProfile::from_toml_str(&older, path()).expect("a valid profile");
        assert!(!profile.firmware.per_layer_settings);
    }

    #[test]
    fn the_blank_machine_is_a_profile_that_loads() {
        let blank = PrinterProfile::default();
        let text = toml::to_string(&blank).expect("serialises");
        let parsed = PrinterProfile::from_toml_str(&text, path()).expect("a valid profile");
        assert_eq!(parsed, blank);
    }

    #[test]
    fn a_profile_naming_no_format_gets_goo_and_one_naming_ctb_keeps_it() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
        assert_eq!(profile.output, OutputFormat::Goo);

        let chitu = SAMPLE.replace(
            "manufacturer = \"Test\"",
            "manufacturer = \"Test\"\noutput = \"ctb5\"",
        );
        let profile = PrinterProfile::from_toml_str(&chitu, path()).expect("valid profile");
        assert_eq!(profile.output, OutputFormat::Ctb5);
    }

    #[test]
    fn a_profile_naming_no_connection_takes_files_on_a_stick() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
        assert_eq!(profile.connection, Connection::None);

        let networked = SAMPLE.replace(
            "manufacturer = \"Test\"",
            "manufacturer = \"Test\"\nconnection = \"sdcp\"",
        );
        let profile = PrinterProfile::from_toml_str(&networked, path()).expect("valid profile");
        assert_eq!(profile.connection, Connection::Sdcp);
    }

    #[test]
    fn pitch_is_area_over_pixel_count() {
        let profile = PrinterProfile::from_toml_str(SAMPLE, path()).expect("valid profile");
        assert_eq!(profile.display.pixel_pitch_mm(), (0.1, 0.1));
        assert!(!profile.mirror_x);
    }

    #[test]
    fn zero_dimension_is_rejected() {
        let broken = SAMPLE.replace("width_mm = 10.0", "width_mm = 0.0");
        let err = PrinterProfile::from_toml_str(&broken, path()).unwrap_err();
        assert!(matches!(
            err,
            ProfileError::NonPositive {
                field: "display.width_mm",
                ..
            }
        ));
    }

    #[test]
    fn malformed_toml_reports_its_path() {
        let err = PrinterProfile::from_toml_str("name = ", path()).unwrap_err();
        assert!(matches!(err, ProfileError::Parse { .. }));
    }
}
