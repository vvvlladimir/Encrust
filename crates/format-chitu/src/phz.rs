//! The `.phz` of the Sonic Mini: the `.fdg` layout, previews, records and one-bit codec
//! under a header of its own. Layout is in `docs/formats/phz.md`.

use core_format::{FormatError, PrintJob, SlicedFileWriter, WriteSeek};

use crate::fdg::FdgSink;

mod header;
mod reader;
#[cfg(test)]
mod tests;

pub use reader::PhzReader;

/// Writes the `.phz` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhzWriter;

impl SlicedFileWriter for PhzWriter {
    type Sink<'w> = FdgSink<'w>;

    fn extension(&self) -> &'static str {
        "phz"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        FdgSink::open(job, sink, header::write_header, reader::crypt)
    }
}
