//! What each tool takes, as the JSON Schema a client is shown, and the checks a value passes
//! before it reaches the command line.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use crate::recipe::{Drains, Hollow, Pose, Repair, Supports};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProfiles {
    /// Keeps only the printers whose id, maker or name holds this, ignoring case.
    pub printer_query: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Model {
    /// Absolute path of an STL, OBJ or 3MF file on this computer.
    pub path: PathBuf,
    pub repair: Option<Repair>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Start {
    /// Absolute path of an STL, OBJ or 3MF file on this computer.
    pub model: PathBuf,
    /// The printer the user named: an id `list_profiles` lists, or a printer TOML's path.
    pub printer: String,
    /// The resin the user named: an id `list_profiles` lists, or a resin TOML's path.
    pub resin: String,
    pub repair: Option<Repair>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct JobRef {
    /// The id `start_job` returned.
    pub job: String,
    /// Counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPose {
    pub job: String,
    /// Absent stands the model as its file does. A new pose clears the drains.
    pub pose: Option<Pose>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetHollow {
    pub job: String,
    /// Absent leaves the model solid.
    pub hollow: Option<Hollow>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetDrains {
    pub job: String,
    /// Absent drills no hole.
    pub drains: Option<Drains>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSupports {
    pub job: String,
    /// Absent stands the model on nothing but the plate.
    pub supports: Option<Supports>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Revert {
    pub job: String,
    /// The revision to make the latest again, counted from one.
    #[schemars(range(min = 1))]
    pub revision: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFile {
    pub job: String,
    /// Counted from one; absent for the latest.
    #[schemars(range(min = 1))]
    pub revision: Option<u32>,
    /// Absolute path of the file to write; its extension picks the format (`.goo`, `.ctb`, …).
    pub output: PathBuf,
    /// Writes even when the check names defects. Only when the user accepted them.
    #[serde(default)]
    pub accept_defects: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendFile {
    /// Absolute path of a file `write_file` wrote.
    pub file: PathBuf,
    /// The printer's IP address or host on the local network.
    pub address: String,
    /// Starts the print once the printer has checked it. Only when the user asked to start
    /// and said the vat is filled and the plate and film are clear.
    #[serde(default)]
    pub start: bool,
}

/// A path from the client: absolute, so it means one file whatever the server's directory,
/// and so never read as a flag.
pub fn absolute(path: &Path, what: &str) -> Result<()> {
    if !path.is_absolute() {
        bail!("{what} {} is not an absolute path", path.display());
    }
    Ok(())
}

/// A value from the client that lands on the command line as an argument of its own.
pub fn plain(value: &str, what: &str) -> Result<()> {
    if value.is_empty() || value.starts_with('-') {
        bail!("{what} {value:?} is not a name the command line takes");
    }
    Ok(())
}
