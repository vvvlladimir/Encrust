use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::commands::Cli;
use crate::exit::Exit;

#[derive(clap::Args, Debug)]
pub struct CompletionsCommand {
    /// The shell to complete for.
    pub shell: Shell,
}

impl CompletionsCommand {
    pub fn run(&self) -> Result<Exit> {
        clap_complete::generate(
            self.shell,
            &mut Cli::command(),
            "encrust",
            &mut std::io::stdout(),
        );
        Ok(Exit::Success)
    }
}
