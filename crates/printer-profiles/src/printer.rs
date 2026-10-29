use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::Table;

mod output;

pub use output::{AnycubicExtension, OutputFormat, PhotonRevision, PwszExtension};

use crate::{
    Buffer, FrontEdge, MaterialProfile, Mirror, PlateInset, PrinterLimits, PrinterTuning,
    ProfileError, mirror,
};

/// How a machine takes a sliced file over the network, where it takes one at all.
/// What it speaks, not how it is reached; address and credentials: `docs/decisions/0153`.
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
    /// The greys an edge pixel is spread over on this machine; 0 to 255, the default, keeps every grey. See `docs/decisions/0378`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub grey_min: u8,
    #[serde(default = "full_grey", skip_serializing_if = "is_full_grey")]
    pub grey_max: u8,
    /// Whether this machine takes masks without grey only, anti-aliasing off and hidden.
    /// See `docs/decisions/0377`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub binary_only: bool,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_zero(value: &u8) -> bool {
    *value == 0
}

fn full_grey() -> u8 {
    u8::MAX
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_full_grey(value: &u8) -> bool {
    *value == u8::MAX
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
    /// How the panel takes the plate; a profile that says nothing is `Normal`.
    #[serde(default)]
    pub mirror: Mirror,
    /// Which plate edge is the machine's front, for the viewport only; see ADR 0369.
    #[serde(default, skip_serializing_if = "FrontEdge::is_default")]
    pub front_edge: FrontEdge,
    /// The band along the plate's edges models are kept out of, where the maker gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate_inset: Option<PlateInset>,
    /// What this machine's firmware reads. A profile that says nothing gets `.goo`.
    #[serde(default)]
    pub output: OutputFormat,
    /// What this machine takes a file over, where it takes one at all.
    #[serde(default)]
    pub connection: Connection,
    /// The printers of this model on the network that its files go to, in the order they
    /// were listed. See `docs/decisions/0265`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub machines: Vec<NetworkMachine>,
    /// What this machine's firmware will obey beyond the header.
    #[serde(default)]
    pub firmware: Firmware,
    /// Resin settings this machine takes at one value only: the window hides them and every
    /// file states these. See `docs/decisions/0301`.
    #[serde(default, skip_serializing_if = "PrinterTuning::is_empty")]
    pub locked: PrinterTuning,
    /// The buffer this firmware runs and a resin may change; `None` for one without.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<Buffer>,
    /// The span each resin setting may take on this machine. See `docs/decisions/0368`.
    #[serde(default, skip_serializing_if = "PrinterLimits::is_empty")]
    pub limits: PrinterLimits,
}

/// One printer on the network a profile sends to: what the user calls it and where it is.
/// A Prusa machine's login is not here; it is the window's own setting (ADR 0153).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkMachine {
    /// What the user calls it; empty shows the address.
    #[serde(default)]
    pub name: String,
    /// The IP address or host it is reached at.
    pub address: String,
    /// An SDCP board's own id, which still names it after it moves to another address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
}

impl NetworkMachine {
    /// What it is called in front of the user.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.address
        } else {
            &self.name
        }
    }
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
                grey_min: 0,
                grey_max: u8::MAX,
                binary_only: false,
            },
            build_volume: BuildVolume {
                x: 143.36,
                y: 89.6,
                z: 165.0,
            },
            mirror: Mirror::LcdMirror,
            front_edge: FrontEdge::MinY,
            output: OutputFormat::Goo,
            connection: Connection::None,
            machines: Vec::new(),
            firmware: Firmware::default(),
            locked: PrinterTuning::default(),
            buffer: None,
            plate_inset: None,
            limits: PrinterLimits::default(),
        }
    }
}

impl PrinterProfile {
    /// The machine name to write into a sliced file: what the firmware matches, which is
    /// the profile's own name unless it states otherwise.
    pub fn machine_name(&self) -> &str {
        self.machine_name.as_deref().unwrap_or(&self.name)
    }

    /// This machine sent over `other`'s network: its connection and the printers listed,
    /// which are the user's and no part of a model's numbers.
    #[must_use]
    pub fn with_network_of(self, other: &PrinterProfile) -> Self {
        Self {
            connection: other.connection,
            machines: other.machines.clone(),
            ..self
        }
    }

    pub fn from_toml_str(source: &str, path: &Path) -> Result<Self, ProfileError> {
        let parse_error = |source| ProfileError::Parse {
            path: path.to_owned(),
            source,
        };
        let mut table: Table = toml::from_str(source).map_err(parse_error)?;
        mirror::read_old_flags(&mut table);
        let profile: Self = table.try_into().map_err(parse_error)?;
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

    pub(crate) fn validate(&self) -> Result<(), ProfileError> {
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
        let (min, max) = (self.display.grey_min, self.display.grey_max);
        if min > max {
            return Err(ProfileError::InvertedGreyRange { min, max });
        }
        if let Some(index) = self
            .machines
            .iter()
            .position(|machine| machine.address.trim().is_empty())
        {
            return Err(ProfileError::NoAddress { index });
        }
        if let Some(inset) = &self.plate_inset {
            inset.validate(self.build_volume.x, self.build_volume.y)?;
        }
        self.limits.validate()?;
        MaterialProfile::default()
            .with_tuning(&self.locked)
            .validate_values()
            .map_err(|error| match error {
                ProfileError::NonPositive { field, value } => {
                    ProfileError::NonPositiveLocked { field, value }
                }
                other => other,
            })
    }
}

#[cfg(test)]
mod tests;
