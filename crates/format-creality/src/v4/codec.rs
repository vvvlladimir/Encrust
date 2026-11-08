//! Which layer codec a version 4 machine reads, picked by its model code. See
//! `docs/formats/creality.md`.

use core_format::{FormatError, Rle7Layer, decode_rle7};
use core_raster::{LayerRuns, Run};

use super::rle;
use crate::family::model_code;

const PAIRED_ROW_MODELS: [&str; 3] = ["R009", "R026", "R027"];
const SIX_BIT_ROW_MODELS: [&str; 1] = ["R012"];
const SEVEN_BIT_ROW_MODELS: [&str; 2] = ["R013", "R018"];

/// How a version 4 file packs a layer; the firmware of each machine reads one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerCodec {
    /// Seven-bit runs over the whole image, not restarting on a row.
    WholeImage,
    /// Runs of two-pixel units restarting on every row, in eight levels of grey.
    PairedRows,
    /// Six-bit runs restarting on every row.
    SixBitRows,
    /// Seven-bit runs restarting on every row.
    SevenBitRows,
}

impl LayerCodec {
    /// The codec of the machine a header's model code names.
    pub(super) fn of_model(code: &str) -> Self {
        if PAIRED_ROW_MODELS.contains(&code) {
            Self::PairedRows
        } else if SIX_BIT_ROW_MODELS.contains(&code) {
            Self::SixBitRows
        } else if SEVEN_BIT_ROW_MODELS.contains(&code) {
            Self::SevenBitRows
        } else {
            Self::WholeImage
        }
    }

    /// The codec of a machine by its name, as a profile states it.
    pub fn of_machine(name: &str) -> Self {
        model_code(name).map_or(Self::WholeImage, |code| Self::of_model(&code))
    }

    pub(super) fn encode(self, layer: &LayerRuns) -> Vec<u8> {
        match self {
            Self::WholeImage => Rle7Layer::encode(layer).data().to_vec(),
            Self::PairedRows => rle::encode_paired_rows(layer),
            Self::SixBitRows => rle::encode_rows(layer, 6),
            Self::SevenBitRows => rle::encode_rows(layer, 7),
        }
    }

    pub(super) fn decode(
        self,
        data: &[u8],
        width_px: u32,
        pixel_count: u32,
    ) -> Result<Vec<Run>, FormatError> {
        match self {
            Self::WholeImage => decode_whole_image(data, pixel_count),
            Self::PairedRows => rle::decode_paired_rows(data, width_px, pixel_count),
            Self::SixBitRows => rle::decode_rows(data, pixel_count, 6),
            Self::SevenBitRows => rle::decode_rows(data, pixel_count, 7),
        }
    }

    /// Levels of grey a layer reads back in.
    pub(super) fn grey_steps(self) -> u16 {
        match self {
            Self::WholeImage | Self::SevenBitRows => 128,
            Self::SixBitRows => 64,
            Self::PairedRows => 8,
        }
    }
}

fn decode_whole_image(data: &[u8], pixel_count: u32) -> Result<Vec<Run>, FormatError> {
    let runs = decode_rle7(data)?;
    let covered: u64 = runs.iter().map(|run| u64::from(run.length)).sum();
    if covered > u64::from(pixel_count) {
        return Err(FormatError::MalformedRun { offset: 0 });
    }
    Ok(runs)
}

/// Picks a layer codec at compile time, as a layer is encoded away from the sink.
pub trait LayerCodecKind: Send + Sync + 'static {
    /// The codec this type stands for.
    const CODEC: LayerCodec;
}

/// See [`LayerCodec::WholeImage`].
#[derive(Debug, Default, Clone, Copy)]
pub struct WholeImageRuns;

/// See [`LayerCodec::PairedRows`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PairedRowRuns;

/// See [`LayerCodec::SixBitRows`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SixBitRowRuns;

/// See [`LayerCodec::SevenBitRows`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SevenBitRowRuns;

impl LayerCodecKind for WholeImageRuns {
    const CODEC: LayerCodec = LayerCodec::WholeImage;
}

impl LayerCodecKind for PairedRowRuns {
    const CODEC: LayerCodec = LayerCodec::PairedRows;
}

impl LayerCodecKind for SixBitRowRuns {
    const CODEC: LayerCodec = LayerCodec::SixBitRows;
}

impl LayerCodecKind for SevenBitRowRuns {
    const CODEC: LayerCodec = LayerCodec::SevenBitRows;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eight lit pixels over two rows of four.
    fn lit_layer() -> LayerRuns {
        let mut builder = LayerRuns::builder(4, 2);
        builder.push(8, 255);
        builder.finish()
    }

    #[test]
    fn each_machine_takes_the_codec_its_firmware_reads() {
        let cases = [
            ("HALOT-MAGE PRO", LayerCodec::WholeImage),
            ("HALOT-MAGE", LayerCodec::WholeImage),
            ("HALOT-MAGE S", LayerCodec::WholeImage),
            ("HALOT-R6", LayerCodec::WholeImage),
            ("HALOT-PLAY", LayerCodec::WholeImage),
            ("PIOCREAT C02", LayerCodec::WholeImage),
            ("C01", LayerCodec::WholeImage),
            ("DJ89 PLUS", LayerCodec::WholeImage),
            ("HALOT-X1", LayerCodec::PairedRows),
            ("HALOT-SKY 2025", LayerCodec::PairedRows),
            ("PioNext Ultra", LayerCodec::PairedRows),
            ("HALOT-X1 Max", LayerCodec::SixBitRows),
            ("HALOT-X1 Neo", LayerCodec::SevenBitRows),
            ("PioNext Mini", LayerCodec::SevenBitRows),
        ];
        for (machine, codec) in cases {
            assert_eq!(LayerCodec::of_machine(machine), codec, "{machine}");
        }
    }

    #[test]
    fn a_whole_image_layer_past_the_panel_is_refused() {
        let data = LayerCodec::WholeImage.encode(&lit_layer());
        assert!(matches!(
            LayerCodec::WholeImage.decode(&data, 4, 6),
            Err(FormatError::MalformedRun { offset: 0 })
        ));
    }

    #[test]
    fn a_whole_image_run_does_not_restart_on_a_row() {
        let data = LayerCodec::WholeImage.encode(&lit_layer());
        assert_eq!(data, [0xFF, 0x08], "one run across both rows, 8 pixels");
    }
}
