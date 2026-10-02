use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use core_analysis::Measured;
use core_format::{LayerSink, PrintJob, SlicedFileWriter, WRITE_BUFFER_BYTES, WriteSeek};
use core_geometry::Mesh;
use core_raster::RasterSettings;
use core_slicer::{Sliced, Windows};
use format_anycubic::AnycubicWriter;
use format_chitu::{CbddlpWriter, CtbWriter};
use format_creality::{CxdlpV4Writer, CxdlpVersion, CxdlpWriter};
use format_cws::CwsWriter;
use format_gcode_zip::GcodeZipWriter;
use format_goo::GooWriter;
use format_sl1::Sl1Writer;
use format_svgx::SvgxWriter;
use rayon::prelude::*;

use crate::error::PipelineError;
use crate::fold::{Tolerance, fold_group};
use crate::format::SlicedFormat;

/// What a front end wants to hear about a run while it is happening.
///
/// Every method has a default, so a caller that wants none of it passes `&mut ()`.
pub trait Observer {
    /// One window of the stack, as it comes off the slicer and before it is written.
    fn window(&mut self, _sliced: &Sliced) {}

    /// `done` layers of `total` have reached the file.
    fn layers(&mut self, _done: usize, _total: usize) {}

    /// Asked between two groups of layers. True stops the run and leaves no file behind.
    fn cancelled(&self) -> bool {
        false
    }
}

impl Observer for () {}

/// One run from a cut stack to a sliced file on disk.
pub struct Writing<'a> {
    pub format: SlicedFormat,
    pub path: &'a Path,
    /// Everything the file carries but the masks.
    pub job: &'a PrintJob,
    /// The mesh standing in plate coordinates, cut window by window as the file is
    /// written; see `docs/decisions/0068-the-window-holds-no-stack.md`.
    pub mesh: &'a Mesh,
    pub windows: &'a Windows,
    pub settings: &'a RasterSettings,
    /// Layers rasterised at once. Peak memory is this many masks (ADR 0010).
    pub window: usize,
    /// What the stack folds into: how many bottom layers, and whether islands come out.
    pub fold: Measured,
}

/// What one written file came to.
#[derive(Debug, Clone)]
pub struct Written {
    pub layers: usize,
    pub clipped_layers: usize,
    /// Furthest any layer reached past the edge of the panel, pixels.
    pub max_overflow_px: f32,
    /// What the written masks cure, which is what the resin volume is taken from.
    pub measured: Measured,
}

/// Cuts, rasterises and writes the whole stack, a window at a time.
///
/// `None` means the observer cancelled it, in which case nothing is left on disk: a file
/// stopped half way through the stack would still look printable.
pub fn write(
    request: &Writing<'_>,
    observer: &mut dyn Observer,
) -> Result<Option<Written>, PipelineError> {
    let path = request.path;
    let file = File::create(path).map_err(|source| PipelineError::Create {
        path: path.to_owned(),
        source,
    })?;
    let mut buffered = BufWriter::with_capacity(WRITE_BUFFER_BYTES, file);

    let written = match request.format {
        SlicedFormat::Goo => write_with(&GooWriter, &mut buffered, request, observer),
        SlicedFormat::Ctb(version) => {
            write_with(&CtbWriter::new(version), &mut buffered, request, observer)
        }
        SlicedFormat::Cbddlp(flavour) => write_with(
            &CbddlpWriter::new(flavour),
            &mut buffered,
            request,
            observer,
        ),
        SlicedFormat::Anycubic(flavour, version) => write_with(
            &AnycubicWriter::new(flavour, version),
            &mut buffered,
            request,
            observer,
        ),

        // The layers of an `.sl1` are named after the file they sit in, which is the one
        // thing a writer is not handed; see docs/formats/sl1.md.
        SlicedFormat::Sl1(flavour) => write_with(
            &Sl1Writer::new(flavour, job_dir_of(path)),
            &mut buffered,
            request,
            observer,
        ),
        SlicedFormat::GcodeZip => write_with(&GcodeZipWriter, &mut buffered, request, observer),
        SlicedFormat::Cxdlp(CxdlpVersion::V3) => {
            write_with(&CxdlpWriter, &mut buffered, request, observer)
        }
        SlicedFormat::Cxdlp(CxdlpVersion::V4) => {
            write_with(&CxdlpV4Writer, &mut buffered, request, observer)
        }
        SlicedFormat::Svgx => write_with(&SvgxWriter, &mut buffered, request, observer),
        SlicedFormat::Cws => write_with(&CwsWriter, &mut buffered, request, observer),
    };

    if !matches!(written, Ok(Some(_))) {
        drop(buffered);
        let _ = std::fs::remove_file(path);
    }
    written
}

/// The output file's own stem, which is what an `.sl1` names its layer entries after.
fn job_dir_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Measures what the stack cures without writing it anywhere, so a preview can say what
/// the print takes. As in `write`, no stack is ever held whole.
pub fn measure(
    mesh: &Mesh,
    windows: &Windows,
    settings: &RasterSettings,
    tolerance: &Tolerance,
    mut fold: Measured,
    observer: &mut dyn Observer,
) -> Result<Option<Measured>, PipelineError> {
    let group = rayon::current_num_threads().max(1);
    let mut cancelled = false;

    windows.stream(mesh, |sliced: &Sliced| -> Result<(), PipelineError> {
        observer.window(sliced);
        for layers in sliced.layers.chunks(group) {
            if cancelled || observer.cancelled() {
                cancelled = true;
                return Ok(());
            }
            fold_group(layers, settings, windows.plan(), tolerance, &mut fold)?;
        }
        Ok(())
    })?;

    Ok((!cancelled).then(|| fold.finish()))
}

/// Writes one format's file, from its header to its last layer.
fn write_with<'w, W>(
    writer: &W,
    file: &'w mut dyn WriteSeek,
    request: &Writing<'_>,
    observer: &mut dyn Observer,
) -> Result<Option<Written>, PipelineError>
where
    W: SlicedFileWriter + 'w,
{
    let failed = |source| PipelineError::Write {
        path: request.path.to_owned(),
        source,
    };

    let mut sink = writer.begin(request.job, file).map_err(failed)?;
    let Some(written) = stream_layers(&mut sink, request, observer)? else {
        return Ok(None);
    };
    sink.finish(written.measured.volume_mm3()).map_err(failed)?;
    Ok(Some(written))
}

/// Cuts, rasterises and pushes the stack a window at a time, in order.
fn stream_layers<S: LayerSink>(
    sink: &mut S,
    request: &Writing<'_>,
    observer: &mut dyn Observer,
) -> Result<Option<Written>, PipelineError> {
    let windows = request.windows;
    let total = windows.layer_count();
    let tolerance = Tolerance::of(&request.job.material);
    let mut written = Written {
        layers: 0,
        clipped_layers: 0,
        max_overflow_px: 0.0,
        measured: request.fold.clone(),
    };
    let mut cancelled = false;

    windows.stream(
        request.mesh,
        |sliced: &Sliced| -> Result<(), PipelineError> {
            observer.window(sliced);
            for layers in sliced.layers.chunks(request.window.max(1)) {
                if cancelled || observer.cancelled() {
                    cancelled = true;
                    return Ok(());
                }
                let folded = fold_group(
                    layers,
                    request.settings,
                    windows.plan(),
                    &tolerance,
                    &mut written.measured,
                )?;
                let encoded: Vec<_> = folded
                    .par_iter()
                    .map(|layer| (S::encode(&layer.written()), layer.overflow_px))
                    .collect();

                for (layer, overflow_px) in encoded {
                    sink.push(layer).map_err(|source| PipelineError::Write {
                        path: request.path.to_owned(),
                        source,
                    })?;
                    written.layers += 1;
                    if overflow_px > 0.0 {
                        written.clipped_layers += 1;
                        written.max_overflow_px = written.max_overflow_px.max(overflow_px);
                    }
                }
                observer.layers(written.measured.layer_count(), total);
            }
            Ok(())
        },
    )?;

    Ok((!cancelled).then_some(written))
}
