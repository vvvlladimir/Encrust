use anyhow::Result;

use crate::i18n::tr;

/// A texture compiled into the window, ready to tile over any model without a file. All
/// are CC0; see docs/decisions/0308-ship-a-dozen-height-maps-in-the-window.md.
pub struct Builtin {
    pub name: fn() -> String,
    pub bytes: &'static [u8],
}

macro_rules! builtin {
    ($name:expr, $file:literal) => {
        Builtin {
            name: $name,
            bytes: include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/textures/",
                $file
            )),
        }
    };
}

/// In the order the panel lays them out, four to a row.
pub const BUILTINS: [Builtin; 12] = [
    builtin!(|| tr!("relief-texture-leather"), "leather.png"),
    builtin!(|| tr!("relief-texture-matte"), "matte.png"),
    builtin!(|| tr!("relief-texture-carbon"), "carbon.png"),
    builtin!(|| tr!("relief-texture-weave"), "weave.png"),
    builtin!(|| tr!("relief-texture-honeycomb"), "honeycomb.png"),
    builtin!(|| tr!("relief-texture-hex-blocks"), "hex-blocks.png"),
    builtin!(|| tr!("relief-texture-reptile"), "reptile.png"),
    builtin!(|| tr!("relief-texture-ripples"), "ripples.png"),
    builtin!(|| tr!("relief-texture-wood"), "wood.png"),
    builtin!(|| tr!("relief-texture-bark"), "bark.png"),
    builtin!(|| tr!("relief-texture-stone"), "stone.png"),
    builtin!(|| tr!("relief-texture-rock"), "rock.png"),
];

impl Builtin {
    /// The texture shrunk to `side_px` a side, as grey bytes row after row from the top.
    pub fn thumbnail(&self, side_px: u32) -> Result<Vec<u8>> {
        let image = image::load_from_memory(self.bytes)?;
        Ok(image
            .resize_exact(side_px, side_px, image::imageops::FilterType::Triangle)
            .to_luma8()
            .into_raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_texture_is_a_square_grey_picture() {
        for builtin in &BUILTINS {
            let image = image::load_from_memory(builtin.bytes).expect("a shipped png decodes");
            assert_eq!(
                image.width(),
                image.height(),
                "{} is tiled on a square, so it must be one",
                (builtin.name)()
            );
            assert_eq!(
                builtin.thumbnail(8).expect("it decoded above").len(),
                64,
                "eight by eight grey pixels"
            );
        }
    }
}
