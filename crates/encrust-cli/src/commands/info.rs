use std::path::PathBuf;

use anyhow::Result;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, sliced_read};

#[derive(clap::Args, Debug)]
pub struct InfoCommand {
    /// Sliced file in any container this writes, whoever wrote it.
    pub file: PathBuf,
}

impl InfoCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        let info = sliced_read::read(&self.file)?;
        if global.json {
            json::print(&info.document())?;
        } else {
            print!("{info}");
        }
        Ok(Exit::Success)
    }
}
