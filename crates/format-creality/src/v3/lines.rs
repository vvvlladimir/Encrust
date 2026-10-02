//! The layer codec of `.cxdlp` version 3: a layer is a list of vertical lines, each six
//! bytes. Field widths and the traversal order are in `docs/formats/creality.md`.

use core_format::FormatError;
use core_raster::{LayerRuns, Run};

/// Bytes one line takes: five of packed coordinates and one of grey.
pub(crate) const LINE_BYTES: usize = 6;

/// Widest panel a line can name, from the fourteen bits `startX` has.
const MAX_X: u32 = 0x3FFF;

/// Tallest panel a line can name, from the thirteen bits `startY` and `endY` have.
const MAX_Y: u32 = 0x1FFF;

/// One layer as the lines the container holds, with the lit area they add up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedLayer {
    width: u32,
    height: u32,
    /// Lit pixels, which the area field is taken from once the pixel pitch is known.
    lit_px: u64,
    lines: u32,
    data: Vec<u8>,
}

impl EncodedLayer {
    /// Compresses a layer, column by column.
    ///
    /// The container walks X and runs along Y, which is across our own runs, so this is
    /// the one codec that expands the mask to pixels first.
    pub fn encode(layer: &LayerRuns) -> Result<Self, FormatError> {
        let (width, height) = (layer.width(), layer.height());
        if width.saturating_sub(1) > MAX_X || height.saturating_sub(1) > MAX_Y {
            return Err(FormatError::Encoding {
                what: "a layer",
                reason: format!(
                    "a {width}x{height} panel is past the {MAX_X}x{MAX_Y} a line can name"
                ),
            });
        }

        let mask = layer.to_mask();
        let pixels = mask.pixels();
        let mut data = Vec::new();
        let (mut lines, mut lit_px) = (0u32, 0u64);

        for x in 0..width {
            // A run is a column of one grey: it ends where the grey changes or goes dark.
            let mut open: Option<(u32, u8)> = None;
            for y in 0..height {
                let value = pixels[(y * width + x) as usize];
                match open {
                    Some((_, grey)) if grey == value => continue,
                    Some((start, grey)) => {
                        push_line(&mut data, start, y - 1, x, grey);
                        lines += 1;
                        lit_px += u64::from(y - start);
                    }
                    None => {}
                }
                open = (value > 0).then_some((y, value));
            }
            if let Some((start, grey)) = open {
                push_line(&mut data, start, height - 1, x, grey);
                lines += 1;
                lit_px += u64::from(height - start);
            }
        }

        Ok(Self {
            width,
            height,
            lit_px,
            lines,
            data,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn lines(&self) -> u32 {
        self.lines
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// What the layer cures in square millimetres times a thousand, which is the unit the
    /// area fields are in.
    pub fn area_field(&self, pixel_area_mm2: f32) -> u32 {
        let area = self.lit_px as f32 * pixel_area_mm2 * 1000.0;
        area.max(0.0).min(f32::from(u16::MAX) * 65535.0) as u32
    }
}

/// Packs one line: thirteen bits of `start_y`, thirteen of `end_y`, fourteen of `x`, then
/// the grey. `end_y` is the last row the line covers, not the row after it.
fn push_line(data: &mut Vec<u8>, start_y: u32, end_y: u32, x: u32, grey: u8) {
    data.push(((start_y >> 5) & 0xFF) as u8);
    data.push((((start_y << 3) + (end_y >> 10)) & 0xFF) as u8);
    data.push(((end_y >> 2) & 0xFF) as u8);
    data.push((((end_y << 6) + (x >> 8)) & 0xFF) as u8);
    data.push((x & 0xFF) as u8);
    data.push(grey);
}

/// Expands the lines of one layer back into runs in our own reading order.
///
/// Anything a line names outside the panel is refused rather than clamped: a file that
/// asks for a pixel it has no room for is not a file this panel was sliced for.
pub fn decode(data: &[u8], width: u32, height: u32) -> Result<Vec<Run>, FormatError> {
    if !data.len().is_multiple_of(LINE_BYTES) {
        return Err(FormatError::MalformedRun {
            offset: data.len() - data.len() % LINE_BYTES,
        });
    }

    let mut mask = vec![0u8; (width as usize) * (height as usize)];
    for (index, line) in data.as_chunks::<LINE_BYTES>().0.iter().enumerate() {
        let start_y = ((u32::from(line[0]) << 8) + u32::from(line[1])) >> 3 & 0x1FFF;
        let end_y = ((u32::from(line[1]) << 16) + (u32::from(line[2]) << 8) + u32::from(line[3]))
            >> 6
            & 0x1FFF;
        let x = ((u32::from(line[3]) << 8) + u32::from(line[4])) & 0x3FFF;
        if x >= width || end_y >= height || end_y < start_y {
            return Err(FormatError::MalformedRun {
                offset: index * LINE_BYTES,
            });
        }
        for y in start_y..=end_y {
            mask[(y * width + x) as usize] = line[5];
        }
    }

    let mut runs: Vec<Run> = Vec::new();
    for value in mask {
        match runs.last_mut() {
            Some(run) if run.value == value => run.length += 1,
            _ => runs.push(Run { value, length: 1 }),
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs_of(width: u32, height: u32, pixels: &[u8]) -> LayerRuns {
        let mut mask = core_raster::LayerMask::new(width, height);
        mask.pixels_mut().copy_from_slice(pixels);
        LayerRuns::from_mask(&mask)
    }

    #[test]
    fn a_column_of_one_grey_is_one_line() {
        // 2x4, the left column lit solid and the right dark.
        let pixels = [200, 0, 200, 0, 200, 0, 200, 0];
        let encoded = EncodedLayer::encode(&runs_of(2, 4, &pixels)).expect("it encodes");

        assert_eq!(encoded.lines(), 1);
        assert_eq!(encoded.data().len(), LINE_BYTES);
        assert_eq!(encoded.data()[5], 200, "the grey goes in as it stands");
        assert_eq!(
            decode(encoded.data(), 2, 4).expect("it decodes"),
            runs_of(2, 4, &pixels).runs()
        );
    }

    #[test]
    fn a_column_broken_by_a_gap_is_two_lines() {
        let pixels = [255, 0, 255, 0, 0, 0, 255, 0];
        let encoded = EncodedLayer::encode(&runs_of(2, 4, &pixels)).expect("it encodes");

        assert_eq!(encoded.lines(), 2, "the dark row ends the first line");
        assert_eq!(
            decode(encoded.data(), 2, 4).expect("it decodes"),
            runs_of(2, 4, &pixels).runs()
        );
    }

    #[test]
    fn a_change_of_grey_ends_a_line_without_a_gap() {
        let pixels = [255, 0, 255, 0, 120, 0, 120, 0];
        let encoded = EncodedLayer::encode(&runs_of(2, 4, &pixels)).expect("it encodes");

        assert_eq!(encoded.lines(), 2);
        assert_eq!(
            decode(encoded.data(), 2, 4).expect("it decodes"),
            runs_of(2, 4, &pixels).runs(),
            "an eight-bit line carries every grey the mask had"
        );
    }

    #[test]
    fn a_blank_layer_is_no_lines_at_all() {
        let encoded = EncodedLayer::encode(&LayerRuns::builder(8, 8).finish()).expect("it encodes");
        assert_eq!(encoded.lines(), 0);
        assert!(encoded.data().is_empty());
        assert_eq!(encoded.area_field(0.01), 0);
    }

    #[test]
    fn the_whole_panel_round_trips_through_the_coordinates() {
        let (width, height) = (37u32, 19u32);
        let pixels: Vec<u8> = (0..width * height)
            .map(|index| {
                if index % 7 == 0 {
                    0
                } else {
                    (index % 256) as u8
                }
            })
            .collect();
        let layer = runs_of(width, height, &pixels);
        let encoded = EncodedLayer::encode(&layer).expect("it encodes");

        assert_eq!(
            decode(encoded.data(), width, height).expect("it decodes"),
            layer.runs()
        );
    }

    #[test]
    fn a_panel_taller_than_the_thirteen_bits_a_line_has_is_refused() {
        let err = EncodedLayer::encode(&LayerRuns::builder(64, 9000).finish()).unwrap_err();
        assert!(matches!(err, FormatError::Encoding { what, .. } if what == "a layer"));
    }

    #[test]
    fn a_line_naming_a_row_the_panel_does_not_have_is_refused() {
        let mut data = Vec::new();
        push_line(&mut data, 2, 40, 0, 255);
        let err = decode(&data, 8, 8).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }

    #[test]
    fn a_truncated_line_is_refused() {
        let err = decode(&[0, 0, 0], 8, 8).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { .. }));
    }
}
