use std::fmt::Write as _;

use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::document::{self, TAIL};
use crate::header::{self, HEADER_BYTES};
use crate::trace::{self, Ring};

/// Writes the `.svgx`. Layout is in `docs/formats/svgx.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SvgxWriter;

impl SlicedFileWriter for SvgxWriter {
    type Sink<'w> = SvgxSink<'w>;

    fn extension(&self) -> &'static str {
        "svgx"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        // The container states the panel in millimetres and nothing else says how big a
        // pixel is, so a profile that does not carry the panel cannot be written.
        let (width_mm, height_mm) = (job.printer.display.width_mm, job.printer.display.height_mm);
        if width_mm <= 0.0 || height_mm <= 0.0 {
            return Err(FormatError::Missing {
                what: "the panel size in millimetres".to_owned(),
            });
        }

        let mut fields = Fields::new(sink);
        fields.zeros(HEADER_BYTES as usize)?;
        let previews = header::write_previews(&mut fields, job)?;

        let document = fields.position()? as u32;
        let head = document::head(job);
        let volume_at = u64::from(document)
            + document::volume_offset(&head).ok_or_else(|| FormatError::Encoding {
                what: "the document",
                reason: "it reserves no field for the resin volume".to_owned(),
            })? as u64;
        fields.bytes(head.as_bytes())?;

        tracing::debug!(layers = job.layer_count(), document, "svgx header written");
        Ok(SvgxSink {
            fields,
            job: job.clone(),
            previews,
            document,
            volume_at,
            written: 0,
        })
    }
}

/// An `.svgx` with its document open and its layer groups still to come.
pub struct SvgxSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    previews: [u32; 2],
    /// Where the document begins, which the header addresses.
    document: u32,
    /// Where the resin volume field stands in the file, so the measured value can be put
    /// in it once the stack is written.
    volume_at: u64,
    written: u32,
}

impl LayerSink for SvgxSink<'_> {
    /// The panel the layer covers and its outline, traced where the work can be shared out
    /// over the stack. What a ring is in millimetres only the sink knows.
    type Encoded = (u32, u32, Vec<Ring>);

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        (layer.width(), layer.height(), trace::rings_of(layer))
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let (width, height, rings) = encoded;
        let index = self.written;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        if width != self.job.raster.width_px || height != self.job.raster.height_px {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width,
                height,
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        let body = group_body(&self.job, &rings);
        self.fields
            .bytes(format!("<g id=\"layer-{index}\" {body}</g>\n").as_bytes())?;
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        if self.written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: self.written,
            });
        }
        self.fields.bytes(TAIL.as_bytes())?;

        self.fields.seek_to(self.volume_at)?;
        self.fields
            .bytes(document::volume_field(volume_mm3 / 1000.0).as_bytes())?;

        self.fields.seek_to(0)?;
        header::write_header(&mut self.fields, self.previews, self.document)?;
        self.fields.flush()?;
        Ok(())
    }
}

/// The attributes and the path of one layer's group: everything but its id.
///
/// Every ring of the layer goes into one path, holes included, because even-odd filling
/// takes a ring inside another as a hole whatever order they are in — which is what saves
/// the writer from working out which ring sits in which.
fn group_body(job: &PrintJob, rings: &[Ring]) -> String {
    let (pitch_x, pitch_y) = (job.raster.pitch.x, job.raster.pitch.y);
    let (half_x, half_y) = (
        job.printer.display.width_mm / 2.0,
        job.printer.display.height_mm / 2.0,
    );

    let area_mm2: f64 = rings
        .iter()
        .map(|ring| trace::double_area_px(ring) as f64 * f64::from(pitch_x * pitch_y) / 2.0)
        .sum();
    let perimeter_mm: f64 = rings
        .iter()
        .map(|ring| trace::length_px(ring) as f64 * f64::from(pitch_x.max(pitch_y)))
        .sum();

    let mut body = format!("area=\"{area_mm2:.3}\" perimeter=\"{perimeter_mm:.3}\">\n");
    if !rings.is_empty() {
        body.push_str("<path d=\"");
        for ring in rings {
            write_ring(&mut body, ring, (pitch_x, pitch_y), (half_x, half_y));
        }
        body.push_str("\" style=\"fill:white\" fill-rule=\"evenodd\" />\n");
    }
    body
}

/// One ring as an SVG subpath, in millimetres from the middle of the panel.
fn write_ring(into: &mut String, ring: &Ring, pitch: (f32, f32), half: (f32, f32)) {
    for (index, &(x, y)) in ring.iter().enumerate() {
        let x_mm = x as f32 * pitch.0 - half.0;
        let y_mm = y as f32 * pitch.1 - half.1;
        let lead = match index {
            0 => "M ",
            1 => "L ",
            _ => "",
        };
        let _ = write!(into, "{lead}{x_mm:.3} {y_mm:.3} ");
    }
    into.push_str("Z ");
}
