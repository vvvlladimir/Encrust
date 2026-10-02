use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::rle::EncodedLayer;
use crate::tables::{self, LAYER_DEF_BYTES, LayerDef, NAME_BYTES, Offsets};

/// Which revision of the container is written.
///
/// Every revision holds the same tables in the same order and adds blocks behind them;
/// which machine reads which is in `docs/formats/anycubic.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnycubicVersion {
    #[default]
    V1,
    V516,
    V517,
}

impl AnycubicVersion {
    pub fn number(self) -> u32 {
        match self {
            Self::V1 => 1,
            Self::V516 => 516,
            Self::V517 => 517,
        }
    }

    /// Whether this revision carries the grey table, which arrived at 515.
    pub(crate) fn has_colour_table(self) -> bool {
        self > Self::V1
    }

    /// Whether it carries the two-stage motion and machine blocks, which arrived at 516.
    pub(crate) fn has_machine_block(self) -> bool {
        self >= Self::V516
    }

    /// Whether it carries the slicer and model blocks, which arrived at 517.
    pub(crate) fn has_model_block(self) -> bool {
        self >= Self::V517
    }

    /// How many blocks the file mark claims. The number is not the count of addresses it
    /// writes and the reference states these, so they are transcribed.
    pub(crate) fn table_count(self) -> u32 {
        match self {
            Self::V1 => 4,
            Self::V516 => 8,
            Self::V517 => 9,
        }
    }

    /// Bytes of the file mark: the mark, the revision, the block count and its addresses.
    pub(crate) fn file_mark_bytes(self) -> u32 {
        let addresses = match self {
            Self::V1 => 7,
            Self::V516 => 8,
            Self::V517 => 9,
        };
        NAME_BYTES as u32 + 8 + 4 * addresses
    }

    /// Fields of the header table, which is what its length states.
    pub(crate) fn header_field_bytes(self) -> u32 {
        match self {
            Self::V1 => 80,
            Self::V516 => 84,
            Self::V517 => 92,
        }
    }

    /// What the machine block's property count carries at this revision.
    pub(crate) fn machine_property_fields(self) -> u32 {
        match self {
            Self::V517 => 7,
            _ => 1,
        }
    }
}

/// Which extension an Anycubic file is written under, and so which machine reads it.
///
/// Every one of these is the same container; the extension is what a machine's firmware
/// looks for, so it is the caller's choice and not a property of the file. See
/// `docs/formats/anycubic.md` for the machine each belongs to and the revisions it takes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AnycubicFlavour {
    /// Photon Mono X.
    #[default]
    Pwmx,
    /// Photon Mono.
    Pwmo,
    /// Photon Mono SE.
    Pwms,
    /// Photon Mono SQ.
    Pmsq,
    /// Photon Zero.
    Pw0,
    /// Photon X.
    Pwx,
    /// Photon Ultra.
    Dlp,
    /// Photon D2.
    Dl2p,
    /// Photon Mono 4K.
    Pwma,
    /// Photon Mono X 6K, and the Photon M3 Plus at a revision later.
    Pwmb,
    /// Photon Mono X 6Ks.
    Px6s,
    /// Photon Mono X2.
    Pmx2,
    /// Photon Mono 2.
    Pm3n,
    /// Photon M3.
    Pm3,
    /// Photon M3 Max.
    Pm3m,
    /// Photon M3 Premium.
    Pm3r,
    /// Photon Mono M5.
    Pm5,
}

impl AnycubicFlavour {
    /// Every extension this writer offers, in the order a front end offers them.
    pub const ALL: [Self; 17] = [
        Self::Pwmx,
        Self::Pwmo,
        Self::Pwms,
        Self::Pmsq,
        Self::Pw0,
        Self::Pwx,
        Self::Dlp,
        Self::Dl2p,
        Self::Pwma,
        Self::Pwmb,
        Self::Px6s,
        Self::Pmx2,
        Self::Pm3n,
        Self::Pm3,
        Self::Pm3m,
        Self::Pm3r,
        Self::Pm5,
    ];

    /// What the extension is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pwmx => ".pwmx",
            Self::Pwmo => ".pwmo",
            Self::Pwms => ".pwms",
            Self::Pmsq => ".pmsq",
            Self::Pw0 => ".pw0",
            Self::Pwx => ".pwx",
            Self::Dlp => ".dlp",
            Self::Dl2p => ".dl2p",
            Self::Pwma => ".pwma",
            Self::Pwmb => ".pwmb",
            Self::Px6s => ".px6s",
            Self::Pmx2 => ".pmx2",
            Self::Pm3n => ".pm3n",
            Self::Pm3 => ".pm3",
            Self::Pm3m => ".pm3m",
            Self::Pm3r => ".pm3r",
            Self::Pm5 => ".pm5",
        }
    }

    pub fn extension(self) -> &'static str {
        &self.label()[1..]
    }

    /// The flavour `extension` names, without the dot and in any case.
    pub fn of_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|flavour| extension.eq_ignore_ascii_case(flavour.extension()))
    }

    /// The newest revision this extension's machines read, which is what a file names
    /// itself with when nothing else chose one.
    pub fn newest_version(self) -> AnycubicVersion {
        match self {
            Self::Pw0 | Self::Pwx => AnycubicVersion::V1,
            Self::Pwmx
            | Self::Pwmo
            | Self::Pwms
            | Self::Pmsq
            | Self::Dlp
            | Self::Pwma
            | Self::Pm3
            | Self::Pm3m => AnycubicVersion::V516,
            _ => AnycubicVersion::V517,
        }
    }
}

/// Writes an Anycubic container. Layout is in `docs/formats/anycubic.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnycubicWriter {
    pub flavour: AnycubicFlavour,
    pub version: AnycubicVersion,
}

impl AnycubicWriter {
    pub fn new(flavour: AnycubicFlavour, version: AnycubicVersion) -> Self {
        Self { flavour, version }
    }
}

impl SlicedFileWriter for AnycubicWriter {
    type Sink<'w> = AnycubicSink<'w>;

    fn extension(&self) -> &'static str {
        self.flavour.extension()
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let offsets = Offsets::of(job.layer_count(), self.version);
        let mut fields = Fields::new(sink);
        tables::write_file_mark(&mut fields, offsets, self.version)?;
        tables::write_front(&mut fields, job, self.version)?;

        // The rows are only known once their layers have been compressed, so the space is
        // reserved here and filled in by `finish`.
        fields.zeros((LAYER_DEF_BYTES * u64::from(job.layer_count())) as usize)?;
        tables::write_back(&mut fields, job, self.flavour, self.version)?;
        debug_assert_eq!(fields.position()?, u64::from(offsets.layer_data));
        tracing::debug!(
            layers = job.layer_count(),
            table = offsets.layer_table,
            "anycubic header written"
        );

        Ok(AnycubicSink {
            fields,
            job: job.clone(),
            version: self.version,
            offsets,
            defs: Vec::with_capacity(job.layer_count() as usize),
        })
    }
}

/// An Anycubic file with every table in front of its layers written.
pub struct AnycubicSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    version: AnycubicVersion,
    offsets: Offsets,
    defs: Vec<LayerDef>,
}

impl LayerSink for AnycubicSink<'_> {
    type Encoded = EncodedLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.defs.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        if encoded.width() != self.job.raster.width_px
            || encoded.height() != self.job.raster.height_px
        {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width: encoded.width(),
                height: encoded.height(),
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        let material = &self.job.material;
        let bottom = material.is_bottom_layer(index);
        let (lift_distance_mm, lift_speed_mm_min) = if bottom {
            (
                material.bottom_lift_distance_mm,
                material.bottom_lift_speed_mm_min,
            )
        } else {
            (material.lift_distance_mm, material.lift_speed_mm_min)
        };

        let at = self.fields.position()? as u32;
        self.fields.bytes(encoded.data())?;
        self.defs.push(LayerDef {
            data_address: at,
            data_size: encoded.data().len() as u32,
            lift_distance_mm,
            lift_speed_mm_s: lift_speed_mm_min / 60.0,
            exposure_s: self.job.exposure_of_layer_s(index),
            layer_height_mm: self.job.layer_height_mm(index),
            lit_pixels: encoded.lit_pixels(),
        });
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.defs.len() as u32;
        if written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written,
            });
        }

        // The rows sit behind the table's own name, length and count.
        self.fields
            .seek_to(u64::from(self.offsets.layer_table) + tables::LAYER_TABLE_HEAD_BYTES)?;
        for def in &self.defs {
            def.write(&mut self.fields)?;
        }

        // The header states the resin the stack came to, which is only known now; see
        // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md.
        self.fields.seek_to(u64::from(self.offsets.header))?;
        tables::write_header(&mut self.fields, &self.job, self.version)?;

        self.fields.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    fn layer() -> LayerRuns {
        LayerRuns::builder(8, 4).finish()
    }

    #[test]
    fn every_extension_round_trips_through_its_own_name() {
        for flavour in AnycubicFlavour::ALL {
            assert_eq!(
                AnycubicFlavour::of_extension(flavour.extension()),
                Some(flavour)
            );
        }
        assert_eq!(
            AnycubicFlavour::of_extension("PWMX"),
            Some(AnycubicFlavour::Pwmx)
        );
        assert_eq!(AnycubicFlavour::of_extension("goo"), None);
        assert_eq!(AnycubicWriter::default().extension(), "pwmx");
    }

    #[test]
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = AnycubicWriter::default()
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
        assert!(buffer.into_inner().is_empty());
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = AnycubicWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(EncodedLayer::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = AnycubicWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(AnycubicSink::encode(&layer()))
            .expect("the first layer");
        let err = sink.push(AnycubicSink::encode(&layer())).unwrap_err();
        assert!(matches!(
            err,
            FormatError::LayerCountMismatch {
                expected: 1,
                written: 2
            }
        ));
    }

    #[test]
    fn fewer_layers_than_promised_are_refused_at_the_end() {
        let mut buffer = Cursor::new(Vec::new());
        let sink = AnycubicWriter::default()
            .begin(&sample_job(2), &mut buffer)
            .expect("the job is sound");
        let err = sink.finish(0.0).unwrap_err();
        assert!(matches!(
            err,
            FormatError::LayerCountMismatch {
                expected: 2,
                written: 0
            }
        ));
    }

    #[test]
    fn a_bottom_layer_lifts_by_its_own_numbers() {
        let mut job = sample_job(3);
        job.material.bottom_layers = 1;
        job.material.bottom_lift_distance_mm = 9.0;
        job.material.bottom_lift_speed_mm_min = 30.0;
        job.material.lift_distance_mm = 5.0;
        job.material.lift_speed_mm_min = 60.0;

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut sink = AnycubicWriter::default()
                .begin(&job, &mut buffer)
                .expect("the job is sound");
            for _ in 0..3 {
                sink.push(AnycubicSink::encode(&layer())).expect("a layer");
            }
            sink.finish(0.0).expect("every promised layer arrived");
        }
        let bytes = buffer.into_inner();
        let table = tables::Offsets::of(3, AnycubicVersion::default()).layer_table as usize
            + tables::LAYER_TABLE_HEAD_BYTES as usize;
        let f32_at = |at: usize| {
            f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };

        let row = |index: usize| table + index * LAYER_DEF_BYTES as usize;
        assert_eq!((f32_at(row(0) + 8), f32_at(row(0) + 12)), (9.0, 0.5));
        assert_eq!(
            (f32_at(row(1) + 8), f32_at(row(1) + 12)),
            (5.0, 1.0),
            "a speed is millimetres a second in this container"
        );
    }
}
