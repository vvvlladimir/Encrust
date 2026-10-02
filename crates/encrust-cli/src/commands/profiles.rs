use anyhow::Result;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, profiles};

#[derive(clap::Subcommand, Debug)]
pub enum ProfilesCommand {
    /// List every printer and resin, the user's own marked.
    List,
}

impl ProfilesCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        match self {
            Self::List => {
                let listing = profiles::list()?;
                if global.json {
                    json::print(&listing)?;
                } else {
                    print!("{listing}");
                }
            }
        }
        Ok(Exit::Success)
    }
}
