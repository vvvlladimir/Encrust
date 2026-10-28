use core_format::{
    Fields, FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, check_layer_pushed,
    check_layers_finished, validate,
};
use core_raster::LayerRuns;

use crate::rle::EncodedLayer;
use crate::tables::{self, LAYER_DEF_BYTES, LayerDef, Offsets};
use crate::{blocks, extent};

/// Which revision a file states it is; which machine reads which is in
/// `docs/formats/anycubic.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnycubicVersion {
    #[default]
    V1,
    V515,
    V516,
    V517,
    V518,
}

impl AnycubicVersion {
    /// The revision number the file's header states.
    pub fn number(self) -> u32 {
        match self {
            Self::V1 => 1,
            Self::V515 => 515,
            Self::V516 => 516,
            Self::V517 => 517,
            Self::V518 => 518,
        }
    }

    /// What the machine block's property count carries at this revision.
    pub(crate) fn machine_property_fields(self) -> u32 {
        match self {
            Self::V517 => 7,
            Self::V518 => 15,
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
    /// Photon Mono M5s.
    Pm5s,
    /// Photon Mono M5s Pro.
    M5sp,
    /// Photon Mono 4.
    Pm4n,
}

impl AnycubicFlavour {
    /// Every extension this writer offers, in the order a front end offers them.
    pub const ALL: [Self; 20] = [
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
        Self::Pm5s,
        Self::M5sp,
        Self::Pm4n,
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
            Self::Pm5s => ".pm5s",
            Self::M5sp => ".m5sp",
            Self::Pm4n => ".pm4n",
        }
    }

    /// The extension without its dot.
    pub fn extension(self) -> &'static str {
        &self.label()[1..]
    }

    /// The flavour `extension` names, without the dot and in any case.
    pub fn of_extension(extension: &str) -> Option<Self> {
        core_format::flavour_of(&Self::ALL, extension, Self::extension)
    }

    /// The newest revision this extension's machines read, which is what a file names
    /// itself with when nothing else chose one.
    pub fn newest_version(self) -> AnycubicVersion {
        match self {
            Self::Pw0 | Self::Pwx => AnycubicVersion::V1,
            Self::Pwmo | Self::Pwms | Self::Pmsq | Self::Dlp => AnycubicVersion::V515,
            Self::Pwmx | Self::Pwma | Self::Pm3 | Self::Pm3m => AnycubicVersion::V516,
            Self::Pm5s | Self::M5sp | Self::Pm4n => AnycubicVersion::V518,
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
    /// A writer of `flavour` stating `version`.
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
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;
        blocks::panel_in_sixteen_bits(job, "the machine block")?;

        let offsets = Offsets::of(job.layer_count());
        let mut fields = Fields::new(sink);
        tables::write_file_mark(&mut fields, offsets, self.version)?;
        tables::write_front(&mut fields, job)?;

        // The rows are only known once their layers have been compressed, so the space is
        // reserved here and filled in by `finish`.
        fields.zeros((LAYER_DEF_BYTES * u64::from(job.layer_count())) as usize)?;
        tables::write_back(&mut fields, job, self.version)?;
        debug_assert_eq!(fields.position()?, u64::from(offsets.layer_data));
        tracing::debug!(
            layers = job.layer_count(),
            table = offsets.layer_table,
            "anycubic header written"
        );

        Ok(AnycubicSink {
            fields,
            job: job.clone(),
            offsets,
            defs: Vec::with_capacity(job.layer_count() as usize),
            lit_box_px: None,
        })
    }
}

/// An Anycubic file with every table in front of its layers written.
pub struct AnycubicSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    offsets: Offsets,
    defs: Vec<LayerDef>,
    /// What every layer so far lights, for the model block.
    lit_box_px: Option<[u32; 4]>,
}

/// One layer in the PW0 codec, and where it is lit.
pub struct AnycubicLayer {
    runs: EncodedLayer,
    box_px: Option<[u32; 4]>,
}

impl LayerSink for AnycubicSink<'_> {
    type Encoded = AnycubicLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        AnycubicLayer {
            runs: EncodedLayer::encode(layer),
            box_px: extent::lit_box(layer),
        }
    }

    fn push(&mut self, layer: Self::Encoded) -> Result<(), FormatError> {
        let (encoded, index) = (&layer.runs, self.defs.len() as u32);
        check_layer_pushed(&self.job, index, (encoded.width(), encoded.height()))?;

        let material = &self.job.material;
        let peel = material.peel_of_layer(index);
        let (lift_distance_mm, lift_speed_mm_min) = (peel.lift_mm, peel.lift_speed_mm_min);

        let at = self.fields.position()? as u32;
        self.fields.bytes(encoded.data())?;
        self.defs.push(LayerDef {
            data_address: at,
            data_size: encoded.data().len() as u32,
            lift_distance_mm,
            lift_speed_mm_min,
            exposure_s: self.job.exposure_of_layer_s(index),
            layer_height_mm: self.job.layer_height_mm(index),
            lit_pixels: encoded.lit_pixels(),
        });
        if let Some(box_px) = layer.box_px {
            self.lit_box_px = Some(extent::union(self.lit_box_px, box_px));
        }
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.defs.len() as u32;
        check_layers_finished(&self.job, written)?;

        // The rows sit behind the table's own name, length and count.
        self.fields
            .seek_to(u64::from(self.offsets.layer_table) + tables::LAYER_TABLE_HEAD_BYTES)?;
        for def in &self.defs {
            def.write(&mut self.fields)?;
        }
        self.fields.seek_to(
            u64::from(self.offsets.sub_layers) + u64::from(blocks::SUB_LAYER_HEAD_BYTES),
        )?;
        for def in &self.defs {
            def.write_sub_layer(&mut self.fields)?;
        }

        self.fields.seek_to(u64::from(self.offsets.model))?;
        let model_mm = self
            .lit_box_px
            .map(|box_px| extent::box_mm(&self.job, box_px));
        blocks::write_model(&mut self.fields, &self.job, model_mm)?;

        // The header states the resin the stack came to, which is only known now; see
        // docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md.
        self.fields.seek_to(u64::from(self.offsets.header))?;
        tables::write_header(&mut self.fields, &self.job)?;

        self.fields.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
