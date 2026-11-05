use anyhow::Result;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::json;
use crate::recipe::library;

#[derive(clap::Subcommand, Debug)]
pub enum RecipesCommand {
    /// List the built-in recipes, then yours from the `recipes/` folder beside your profiles.
    List,
    /// Print one recipe as TOML as it reads back, the starting point for one of your own.
    Show(ShowArgs),
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// A built-in id, one of yours by file stem, or a recipe file.
    #[arg(value_name = "ID|PATH")]
    pub recipe: String,
}

impl RecipesCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        match self {
            Self::List => {
                let listing = library::list();
                if global.json {
                    json::print(&listing)?;
                } else if !global.quiet {
                    print!("{listing}");
                }
            }
            Self::Show(args) => show(args, global)?,
        }
        Ok(Exit::Success)
    }
}

fn show(args: &ShowArgs, global: &GlobalArgs) -> Result<()> {
    let (source, recipe) = library::find(&args.recipe)?;
    let toml = recipe.to_toml();
    if global.json {
        return json::print(&serde_json::json!({
            "id": args.recipe,
            "name": recipe.name,
            "source": source,
            "toml": toml,
        }));
    }
    print!("{toml}");
    Ok(())
}
