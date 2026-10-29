use core_format::{
    ChituPeel, FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, chitu_layer_crypt, decode_rle7, layer_in_range, panel_in_range, stated_pwm,
    stated_rests, stated_speed_mm_min, stated_wait_s,
};
use core_raster::Run;

use crate::cipher;
use crate::layout::{MAGIC, POINTER_BYTES, page_address};
use crate::settings::{self as at, SETTINGS_BYTES, Settings};

/// The offset and length of the part of a layer's data enciphered with AES.
type CipheredSpan = (u32, u32);

/// Grey the seven-bit runs decode to.
const GREY_STEPS: u16 = 128;

/// The longest name a file's resin block is believed for, bytes.
const MAX_NAME_BYTES: u32 = 256;

/// Reads the encrypted `.ctb`. Layout is in `docs/formats/ctb-encrypted.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CtbEncryptedReader;

impl SlicedFileReader for CtbEncryptedReader {
    type Open<S: ReadSeek> = OpenCtbEncrypted<S>;

    fn extension(&self) -> &'static str {
        "ctb"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let (version, settings) = read_settings(&mut reads)?;
        let width_px = settings.u32(at::RESOLUTION_PX);
        let height_px = settings.u32(at::RESOLUTION_PX + 4);
        panel_in_range(width_px, height_px)?;

        let (layers, ciphered) = read_layers(&mut reads, &settings)?;
        let machine = read_text(
            &mut reads,
            settings.u32(at::MACHINE_NAME),
            settings.u32(at::MACHINE_NAME + 4),
        )?;
        let resin = match settings.u32(at::RESIN) {
            0 => None,
            address if version >= 5 => read_resin_name(&mut reads, address)?,
            _ => None,
        };
        Ok(OpenCtbEncrypted {
            reads,
            key: settings.u32(at::LAYER_KEY),
            ciphered,
            facts: facts(&settings, version, machine, resin, layers),
        })
    }
}

/// The version the header states and the settings block it addresses, deciphered and held
/// to the signature at the end of the file.
fn read_settings<S: ReadSeek>(reads: &mut Reads<S>) -> Result<(u32, Settings), FormatError> {
    let magic = reads.u32_le()?;
    if magic != MAGIC {
        return Err(FormatError::NotThisFormat {
            format: "ctb",
            found: magic,
        });
    }
    let size = reads.u32_le()?;
    let offset = reads.u32_le()?;
    reads.skip(4)?;
    let version = reads.u32_le()?;
    let signature_size = reads.u32_le()?;
    let signature_at = reads.u32_le()?;
    if size as usize != SETTINGS_BYTES {
        return Err(FormatError::BadBlock {
            what: "settings",
            size: u64::from(size),
        });
    }
    reads.seek_to(u64::from(offset))?;
    let mut settings = Settings(reads.array::<SETTINGS_BYTES>()?);
    cipher::decrypt(&mut settings.0, "settings")?;

    reads.seek_to(u64::from(signature_at))?;
    let signature = reads.bytes(signature_size as usize)?;
    if signature != cipher::signature(settings.u64(at::CHECKSUM)) {
        return Err(FormatError::SignatureMismatch);
    }
    Ok((version, settings))
}

/// Every layer's entry, and where in its data the part enciphered with AES lies.
fn read_layers<S: ReadSeek>(
    reads: &mut Reads<S>,
    settings: &Settings,
) -> Result<(Vec<LayerEntry>, Vec<CipheredSpan>), FormatError> {
    let count = settings.u32(at::LAYER_COUNT);
    let table = u64::from(settings.u32(at::LAYER_POINTERS));
    reads.seek_to(table)?;
    let claimed = reads.claim("layer pointers", u64::from(count), POINTER_BYTES)?;
    let mut layers = Vec::with_capacity(claimed);
    let mut ciphered = Vec::with_capacity(claimed);
    for index in 0..u64::from(count) {
        reads.seek_to(table + index * POINTER_BYTES)?;
        let low = reads.u32_le()?;
        let page = reads.u32_le()?;
        reads.seek_to(page_address(page, low))?;
        reads.skip(4)?;
        let z_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let data_low = reads.u32_le()?;
        let data_page = reads.u32_le()?;
        let size = reads.u32_le()?;
        reads.skip(4)?;
        ciphered.push((reads.u32_le()?, reads.u32_le()?));
        layers.push(LayerEntry {
            z_mm,
            exposure_s,
            offset: page_address(data_page, data_low),
            size,
        });
    }
    Ok((layers, ciphered))
}

/// The resin's name from the version 5 resin block, which addresses its three strings.
fn read_resin_name<S: ReadSeek>(
    reads: &mut Reads<S>,
    address: u32,
) -> Result<Option<String>, FormatError> {
    // The colour, then the type's address and length, then the name's.
    reads.seek_to(u64::from(address) + 16)?;
    let name_at = reads.u32_le()?;
    let name_bytes = reads.u32_le()?;
    if u64::from(name_at) < u64::from(address) + u64::from(core_format::CHITU_RESIN_BYTES) {
        return Ok(None);
    }
    read_text(reads, name_at, name_bytes)
}

/// The text `bytes` long at `address`, trimmed, or `None` where there is none.
fn read_text<S: ReadSeek>(
    reads: &mut Reads<S>,
    address: u32,
    bytes: u32,
) -> Result<Option<String>, FormatError> {
    if address == 0 || bytes == 0 || bytes > MAX_NAME_BYTES {
        return Ok(None);
    }
    reads.seek_to(u64::from(address))?;
    let raw = reads.bytes(bytes as usize)?;
    let text = String::from_utf8_lossy(&raw);
    let text = text.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

fn facts(
    settings: &Settings,
    version: u32,
    machine: Option<String>,
    resin: Option<String>,
    layers: Vec<LayerEntry>,
) -> SlicedFile {
    SlicedFile {
        format: "ctb",
        version: Some(version),
        machine,
        slicer: None,
        resin,
        width_px: settings.u32(at::RESOLUTION_PX),
        height_px: settings.u32(at::RESOLUTION_PX + 4),
        display_mm: Some((
            settings.f32(at::DISPLAY_MM),
            settings.f32(at::DISPLAY_MM + 4),
        ))
        .filter(|&(width, height)| width > 0.0 && height > 0.0),
        layer_height_mm: settings.f32(at::LAYER_HEIGHT_MM),
        exposure_s: settings.f32(at::EXPOSURE_S),
        bottom_exposure_s: settings.f32(at::BOTTOM_EXPOSURE_S),
        bottom_layers: settings.u32(at::BOTTOM_LAYERS),
        print_time_s: Some(settings.u32(at::PRINT_TIME_S)),
        volume_mm3: Some(settings.f32(at::VOLUME_ML) * 1000.0),
        grey_steps: GREY_STEPS,
        stated: stated(settings),
        layers,
    }
}

/// What the settings say of the resin beyond its exposure, kept where a printer could
/// take it (ADR 0293).
fn stated(settings: &Settings) -> PrinterTuning {
    let mut second_stage = [0.0; 6];
    for (index, word) in second_stage.iter_mut().enumerate() {
        *word = settings.f32(at::SECOND_STAGE + 4 * index);
    }
    let peel = ChituPeel {
        bottom_lift_total_mm: settings.f32(at::BOTTOM_LIFT_MM),
        lift_total_mm: settings.f32(at::LIFT_MM),
        second_stage,
        bottom_retract: [
            settings.f32(at::BOTTOM_RETRACT_MM_2),
            settings.f32(at::BOTTOM_RETRACT_SPEED_2),
        ],
    };
    PrinterTuning {
        transition_layers: u16::try_from(settings.u32(at::TRANSITION_LAYERS)).ok(),
        light_off_delay_s: stated_wait_s(settings.f32(at::LIGHT_OFF_S)),
        light_pwm: stated_pwm(settings.u16(at::LIGHT_PWM)),
        bottom_light_pwm: stated_pwm(settings.u16(at::BOTTOM_LIGHT_PWM)),
        lift_speed_mm_min: stated_speed_mm_min(settings.f32(at::LIFT_SPEED)),
        retract_speed_mm_min: stated_speed_mm_min(settings.f32(at::RETRACT_SPEED)),
        bottom_light_off_delay_s: stated_wait_s(settings.f32(at::BOTTOM_LIGHT_OFF_S)),
        bottom_lift_speed_mm_min: stated_speed_mm_min(settings.f32(at::BOTTOM_LIFT_SPEED)),
        bottom_retract_speed_mm_min: stated_speed_mm_min(settings.f32(at::BOTTOM_RETRACT_SPEED)),
        waits: stated_rests(
            settings.f32(at::REST_BEFORE_LIFT_S),
            settings.f32(at::REST_AFTER_LIFT_S),
            settings.f32(at::REST_AFTER_RETRACT_S),
        ),
        ..peel.stated()
    }
}

/// An encrypted `.ctb` with its settings deciphered and its masks still in it.
pub struct OpenCtbEncrypted<S> {
    reads: Reads<S>,
    /// The key the layer runs are covered with, as in the plain `.ctb`.
    key: u32,
    /// Per layer, the offset and length of the part of its data enciphered with AES.
    ciphered: Vec<CipheredSpan>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenCtbEncrypted<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let mut data = self.reads.bytes(entry.size as usize)?;
        let (start, length) = self.ciphered[index as usize];
        if length > 0 {
            let span = (start as usize)
                .checked_add(length as usize)
                .and_then(|end| data.get_mut(start as usize..end))
                .ok_or(FormatError::BadBlock {
                    what: "enciphered layer data",
                    size: u64::from(length),
                })?;
            cipher::decrypt(span, "enciphered layer data")?;
        }
        chitu_layer_crypt(self.key, index, &mut data);
        decode_rle7(&data)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn a_plain_ctb_is_not_taken_for_an_encrypted_one() {
        let mut bytes = 0x12FD_0106u32.to_le_bytes().to_vec();
        bytes.resize(64, 0);
        let refused = CtbEncryptedReader.open(Cursor::new(bytes)).err();
        assert!(matches!(
            refused,
            Some(FormatError::NotThisFormat {
                format: "ctb",
                found: 0x12FD_0106
            })
        ));
    }

    #[test]
    fn a_two_stage_peel_reads_back_as_its_two_stages() {
        let job = crate::fixtures::test_resin_job(2);
        let open = CtbEncryptedReader
            .open(Cursor::new(crate::fixtures::written(&job)))
            .expect("a file we wrote");
        let stated = &open.facts().stated;
        assert_eq!(stated.second_stage, Some(job.material.second_stage));
        assert_eq!(
            (stated.bottom_lift_distance_mm, stated.lift_distance_mm),
            (Some(5.0), Some(2.0)),
            "the first stage, split off the summed climbs"
        );
        assert_eq!(
            stated.bottom_light_off_delay_s,
            Some(job.material.bottom_light_off_s())
        );
    }

    #[test]
    fn settings_of_another_size_are_refused() {
        let mut bytes = Vec::new();
        for word in [MAGIC, 100, 48, 0, 5, 32, 0] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.resize(512, 0);
        let refused = CtbEncryptedReader.open(Cursor::new(bytes)).err();
        assert!(matches!(
            refused,
            Some(FormatError::BadBlock {
                what: "settings",
                size: 100
            })
        ));
    }
}
