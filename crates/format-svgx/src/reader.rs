use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range,
};
use core_raster::{
    Grey, LayerRuns, PixelPitch, RasterSettings, Rasterizer, Run, ScanlineRasterizer, Shading,
};
use core_slicer::{Contour, Layer};
use glam::Vec2;

use crate::document::{attribute, number};
use crate::header::HEADER_BYTES;
use crate::index;

/// Reads the `.svgx`. Layout is in `docs/formats/svgx.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SvgxReader;

impl SlicedFileReader for SvgxReader {
    type Open<S: ReadSeek> = OpenSvgx<S>;

    fn extension(&self) -> &'static str {
        "svgx"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let end = reads.length()?;

        let identifier = reads.text(16)?;
        if !identifier.starts_with("DLP-II") {
            return Err(FormatError::NotThisFormat {
                format: "svgx",
                found: 0,
            });
        }
        reads.skip(4 * 2)?;
        let document = u64::from(reads.u32_le()?);
        if document < HEADER_BYTES || document >= end {
            return Err(FormatError::AddressPastEnd {
                offset: document,
                end,
            });
        }

        // The document is read once to find its settings and where each layer group
        // stands; a group is parsed only when its layer is asked for.
        let (settings, groups) = index::scan(&mut reads, document, end)?;
        let panel = panel_of(&settings)?;
        let layer_height_mm = number(&settings, "layerheight").unwrap_or_default();
        let exposure_s = number(&settings, "basetime").unwrap_or_default();
        let bottom_exposure_s = number(&settings, "attachtime").unwrap_or(exposure_s);
        let bottom_layers = number(&settings, "attachlayer").unwrap_or_default() as u32;

        let layers = groups
            .iter()
            .enumerate()
            .map(|(index, group)| LayerEntry {
                z_mm: layer_height_mm * (index + 1) as f32,
                exposure_s: if (index as u32) < bottom_layers {
                    bottom_exposure_s
                } else {
                    exposure_s
                },
                offset: group.offset,
                size: group.size,
            })
            .collect();

        Ok(OpenSvgx {
            reads,
            panel,
            facts: SlicedFile {
                format: "svgx",
                version: None,
                machine: attribute(&settings, "machinename"),
                slicer: None,
                resin: attribute(&settings, "materialname"),
                width_px: panel.width_px,
                height_px: panel.height_px,
                display_mm: Some((
                    panel.width_px as f32 * panel.pitch.x,
                    panel.height_px as f32 * panel.pitch.y,
                )),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: None,
                volume_mm3: number(&settings, "volume").map(|millilitres| millilitres * 1000.0),
                // A path is filled or it is not: the container carries no grey at all.
                grey_steps: 2,
                layers,
            },
        })
    }
}

/// How the document's millimetres land on the panel's pixels.
fn panel_of(settings: &str) -> Result<RasterSettings, FormatError> {
    let field = |name: &'static str| {
        number(settings, name)
            .filter(|value| *value > 0.0)
            .ok_or(FormatError::Missing {
                what: format!("the document's {name}"),
            })
    };
    let width_px = field("resolutionx")? as u32;
    let height_px = field("resolutiony")? as u32;
    panel_in_range(width_px, height_px)?;
    let width_mm = field("displaywidth")?;
    let height_mm = field("displayheight")?;

    Ok(RasterSettings {
        width_px,
        height_px,
        pitch: PixelPitch {
            x: width_mm / width_px as f32,
            y: height_mm / height_px as f32,
        },
        mirror_x: false,
        mirror_y: false,
        // The paths are already what the panel is to light, so they are filled as they
        // stand rather than shaded by how much of a pixel they cover.
        shading: Shading::Binary,
        grey: Grey::default(),
        blur_px: 0,
    })
}

/// An `.svgx` with its document indexed and its layers still in it.
pub struct OpenSvgx<S> {
    reads: Reads<S>,
    panel: RasterSettings,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenSvgx<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let group = self.reads.bytes(entry.size as usize)?;
        let group = String::from_utf8_lossy(&group);

        let contours = contours_of(&group, &self.panel);
        let rastered = ScanlineRasterizer
            .rasterize(&Layer::new(0.0, contours), &self.panel)
            .map_err(|source| FormatError::Encoding {
                what: "a layer",
                reason: source.to_string(),
            })?;
        Ok(runs_of(&rastered.runs))
    }
}

fn runs_of(runs: &LayerRuns) -> Vec<Run> {
    runs.runs().to_vec()
}

/// The rings of one group, in plate millimetres: the document measures from the middle of
/// the panel and a contour from its near corner.
fn contours_of(group: &str, panel: &RasterSettings) -> Vec<Contour> {
    let half = Vec2::new(
        panel.width_px as f32 * panel.pitch.x / 2.0,
        panel.height_px as f32 * panel.pitch.y / 2.0,
    );
    let mut contours = Vec::new();
    let mut rest = group;

    while let Some(at) = rest.find("d=\"") {
        let value = &rest[at + 3..];
        let Some(end) = value.find('"') else { break };
        for points in subpaths(&value[..end]) {
            if let Some(contour) = Contour::from_points(
                points
                    .into_iter()
                    .map(|point| point + half)
                    .collect::<Vec<_>>(),
            ) {
                contours.push(contour);
            }
        }
        rest = &value[end..];
    }
    contours
}

/// One path's `d` broken into its closed subpaths. Only the moves and lines a writer of
/// this container produces are read: a curve has no meaning on a pixel grid.
fn subpaths(data: &str) -> Vec<Vec<Vec2>> {
    let mut subpaths = Vec::new();
    let mut points: Vec<Vec2> = Vec::new();
    let mut pending: Option<f32> = None;

    for word in data.split_ascii_whitespace() {
        match word {
            "M" | "Z" | "z" => {
                if !points.is_empty() {
                    subpaths.push(std::mem::take(&mut points));
                }
                pending = None;
            }
            "L" | "l" | "m" => pending = None,
            _ => match (word.parse::<f32>(), pending.take()) {
                (Ok(value), None) => pending = Some(value),
                (Ok(y), Some(x)) => points.push(Vec2::new(x, y)),
                (Err(_), _) => pending = None,
            },
        }
    }
    if !points.is_empty() {
        subpaths.push(points);
    }
    subpaths
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn something_that_is_not_one_of_these_is_refused() {
        let mut source = Cursor::new(vec![0x42; 256]);
        let err = SvgxReader
            .open(&mut source)
            .err()
            .expect("nothing but an svgx is read as one");
        assert!(matches!(
            err,
            FormatError::NotThisFormat { format: "svgx", .. }
        ));
    }

    #[test]
    fn a_subpath_is_closed_at_its_z() {
        let paths = subpaths("M 0 0 L 1 0 1 1 0 1 Z M 2 2 L 3 2 3 3 Z");
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].len(), 4);
        assert_eq!(paths[1].len(), 3);
    }

    #[test]
    fn a_path_with_no_numbers_in_it_yields_nothing() {
        assert!(subpaths("M Z").is_empty());
        assert!(subpaths("").is_empty());
    }
}
