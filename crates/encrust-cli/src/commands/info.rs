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

    /// Report one layer instead of decoding them all, counted from one as a printer's
    /// screen counts: where it sits, how long it burns and how much of the panel it lights.
    #[arg(long, value_name = "N")]
    pub layer: Option<NonZeroU32>,

    /// Where `--layer` writes its image.
    #[arg(long, value_name = "PATH", requires = "layer")]
    pub png: Option<PathBuf>,
}

/// The document `info --layer N --json` prints: the layer, and the image where one was
/// asked for.
#[derive(serde::Serialize)]
struct OneLayer<'a, L: serde::Serialize> {
    layer: L,
    #[serde(skip_serializing_if = "Option::is_none")]
    png: Option<&'a std::path::Path>,
}

impl InfoCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        if let Some(number) = self.layer {
            let layer = sliced_read::layer(&self.file, number.get())?;
            if let Some(png) = &self.png {
                std::fs::write(png, layer.png()?)
                    .with_context(|| format!("cannot write {}", png.display()))?;
            }
            if global.json {
                json::print(&OneLayer {
                    layer: layer.document(),
                    png: self.png.as_deref(),
                })?;
            } else if !global.quiet {
                print!("{layer}");
                if let Some(png) = &self.png {
                    println!("  {:<14}{}", "written to", png.display());
                }
            }
            return Ok(Exit::Success);
        }
        let info = sliced_read::read(&self.file)?;
        if global.json {
            json::print(&info.document())?;
        } else if !global.quiet {
            print!("{info}");
        }
        Ok(Exit::Success)
    }
}
