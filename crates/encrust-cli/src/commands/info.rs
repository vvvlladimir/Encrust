use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, sliced_read};

// TODO(step-B5): `--thumbnail out.png` waits on every reader handing over its preview records.
#[derive(clap::Args, Debug)]
pub struct InfoCommand {
    /// Sliced file in any container this writes, whoever wrote it.
    pub file: PathBuf,

    /// Take one layer out as a PNG instead of decoding them all, counted from one as a
    /// printer's screen counts.
    #[arg(long, value_name = "N", requires = "png")]
    pub layer: Option<NonZeroU32>,

    /// Where `--layer` writes its image.
    #[arg(long, value_name = "PATH", requires = "layer")]
    pub png: Option<PathBuf>,
}

impl InfoCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        if let (Some(number), Some(png)) = (self.layer, &self.png) {
            let bytes = sliced_read::layer_png(&self.file, number.get())?;
            std::fs::write(png, bytes)
                .with_context(|| format!("cannot write {}", png.display()))?;
            if global.json {
                json::print(&serde_json::json!({ "layer": number, "png": png }))?;
            } else if global.talks() {
                println!("Layer {number} written to {}", png.display());
            }
            return Ok(Exit::Success);
        }
        let info = sliced_read::read(&self.file)?;
        if global.json {
            json::print(&info.document())?;
        } else {
            print!("{info}");
        }
        Ok(Exit::Success)
    }
}
