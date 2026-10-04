use std::path::PathBuf;

use anyhow::{Context, Result};
use printer_profiles::Kind;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, profiles};

#[derive(clap::Subcommand, Debug)]
pub enum ProfilesCommand {
    /// List every printer and resin, the user's own marked.
    List,
    /// Print one profile as TOML, the starting point for one of your own.
    Show(ShowArgs),
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// The profile's id, as `profiles list` prints it.
    pub id: String,

    /// Which kind of profile, when a printer, a resin or a support profile share the id.
    #[arg(long, value_enum)]
    pub kind: Option<ProfileKind>,

    /// Write the TOML to this file instead of printing it.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ProfileKind {
    Printer,
    Resin,
    Support,
}

impl From<ProfileKind> for Kind {
    fn from(kind: ProfileKind) -> Self {
        match kind {
            ProfileKind::Printer => Self::Printer,
            ProfileKind::Resin => Self::Resin,
            ProfileKind::Support => Self::Support,
        }
    }
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
            Self::Show(args) => show(args, global)?,
        }
        Ok(Exit::Success)
    }
}

fn show(args: &ShowArgs, global: &GlobalArgs) -> Result<()> {
    let (kind, toml) = profiles::show(&args.id, args.kind.map(Kind::from))?;
    if let Some(path) = &args.output {
        std::fs::write(path, &toml).with_context(|| format!("cannot write {}", path.display()))?;
    }
    if global.json {
        return json::print(&serde_json::json!({
            "id": args.id,
            "kind": kind.label(),
            "output": args.output,
            "toml": args.output.is_none().then_some(&toml),
        }));
    }
    match &args.output {
        Some(path) if global.talks() => {
            println!("{} {} written to {}", kind.label(), args.id, path.display())
        }
        Some(_) => {}
        None => print!("{toml}"),
    }
    Ok(())
}
