use std::path::PathBuf;

use anyhow::{Context, Result};
use printer_profiles::{ForeignResin, Kind};

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, profiles};

#[derive(clap::Subcommand, Debug)]
pub enum ProfilesCommand {
    /// List every printer, resin, support profile and tool preset, the user's own marked.
    List,
    /// Print one profile as TOML, the starting point for one of your own.
    Show(ShowArgs),
    /// Import an exported profile, a resin from another program, or the resin a sliced file was
    /// printed with, under a new id of its own; an import never overwrites.
    Import(ImportArgs),
}

#[derive(clap::Args, Debug)]
pub struct ImportArgs {
    /// The exported profile, a TOML file; a resin exported as `.cfgx` or `.ini`; or a sliced
    /// file to take the resin of.
    pub file: PathBuf,

    /// What a TOML profile holds. Left out, the file is another program's resin or a sliced
    /// file.
    #[arg(long, value_enum)]
    pub kind: Option<ProfileKind>,

    /// The printer a resin is set up on, by id; it keeps whatever else it was tuned for. A
    /// resin from another program goes on the machine it names unless this names another.
    #[arg(long, value_name = "ID")]
    pub printer: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// The profile's id, as `profiles list` prints it.
    pub id: String,

    /// Which kind of profile, when two kinds share the id.
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
    /// Named settings of the Hollow tool.
    Hollow,
    /// Named settings of the Drain tool.
    Drain,
}

impl From<ProfileKind> for Kind {
    fn from(kind: ProfileKind) -> Self {
        match kind {
            ProfileKind::Printer => Self::Printer,
            ProfileKind::Resin => Self::Resin,
            ProfileKind::Support => Self::Support,
            ProfileKind::Hollow => Self::Hollow,
            ProfileKind::Drain => Self::Drain,
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
                } else if !global.quiet {
                    print!("{listing}");
                }
            }
            Self::Show(args) => show(args, global)?,
            Self::Import(args) => import(args, global)?,
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

fn import(args: &ImportArgs, global: &GlobalArgs) -> Result<()> {
    let Some(kind) = args.kind.map(Kind::from) else {
        return import_resin_of(args, global);
    };
    let id = profiles::import(&args.file, kind, args.printer.as_deref())?;
    if global.json {
        return json::print(&serde_json::json!({ "id": id, "kind": kind.label() }));
    }
    if global.talks() {
        println!("{} {} imported as {id}", kind.label(), args.file.display());
    }
    Ok(())
}

fn import_resin_of(args: &ImportArgs, global: &GlobalArgs) -> Result<()> {
    let printer = args.printer.as_deref();
    let (id, printer) = if ForeignResin::reads(&args.file) {
        profiles::import_foreign(&args.file, printer)?
    } else {
        profiles::import_sliced(&args.file, printer)?
    };
    if global.json {
        return json::print(&serde_json::json!({
            "id": id,
            "kind": Kind::Resin.label(),
            "printer": printer,
        }));
    }
    if global.talks() {
        println!(
            "resin of {} imported as {id}, set up on {printer}",
            args.file.display()
        );
    }
    Ok(())
}
