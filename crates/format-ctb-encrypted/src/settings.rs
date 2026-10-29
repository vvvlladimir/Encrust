//! The 288-byte settings block, enciphered in the file: the plain `.ctb`'s header, print
//! parameters, slicer info and version 4 block run together. Offsets in
//! `docs/formats/ctb-encrypted.md`.

/// Bytes of the block, which is eighteen cipher blocks.
pub(crate) const SETTINGS_BYTES: usize = 288;

pub(crate) const CHECKSUM: usize = 0x00;
pub(crate) const LAYER_POINTERS: usize = 0x08;
pub(crate) const DISPLAY_MM: usize = 0x0C;
pub(crate) const MACHINE_Z_MM: usize = 0x14;
pub(crate) const LAYER_HEIGHT_MM: usize = 0x24;
pub(crate) const EXPOSURE_S: usize = 0x28;
pub(crate) const BOTTOM_EXPOSURE_S: usize = 0x2C;
pub(crate) const LIGHT_OFF_S: usize = 0x30;
pub(crate) const BOTTOM_LAYERS: usize = 0x34;
pub(crate) const RESOLUTION_PX: usize = 0x38;
pub(crate) const LAYER_COUNT: usize = 0x40;
pub(crate) const LARGE_PREVIEW: usize = 0x44;
pub(crate) const SMALL_PREVIEW: usize = 0x48;
pub(crate) const PRINT_TIME_S: usize = 0x4C;
pub(crate) const PROJECTOR: usize = 0x50;
pub(crate) const BOTTOM_LIFT_MM: usize = 0x54;
pub(crate) const BOTTOM_LIFT_SPEED: usize = 0x58;
pub(crate) const LIFT_MM: usize = 0x5C;
pub(crate) const LIFT_SPEED: usize = 0x60;
pub(crate) const RETRACT_SPEED: usize = 0x64;
pub(crate) const VOLUME_ML: usize = 0x68;
pub(crate) const WEIGHT_G: usize = 0x6C;
pub(crate) const COST: usize = 0x70;
pub(crate) const BOTTOM_LIGHT_OFF_S: usize = 0x74;
pub(crate) const ONE: usize = 0x78;
pub(crate) const LIGHT_PWM: usize = 0x7C;
pub(crate) const BOTTOM_LIGHT_PWM: usize = 0x7E;
pub(crate) const LAYER_KEY: usize = 0x80;
/// Second-stage bottom lift, its speed, lift, its speed, retract, its speed.
pub(crate) const SECOND_STAGE: usize = 0x84;
pub(crate) const FIRST_REST_BEFORE_LIFT_S: usize = 0x9C;
pub(crate) const MACHINE_NAME: usize = 0xA0;
/// The plain `.ctb`'s slicer flags: 7 in the low byte, per-layer settings in the high.
pub(crate) const FLAGS: usize = 0xA8;
pub(crate) const MODIFIED_MINUTES: usize = 0xAC;
pub(crate) const ANTI_ALIAS_LEVEL: usize = 0xB0;
pub(crate) const FIRST_REST_AFTER_RETRACT_S: usize = 0xB4;
pub(crate) const FIRST_REST_AFTER_LIFT_S: usize = 0xB8;
pub(crate) const TRANSITION_LAYERS: usize = 0xBC;
pub(crate) const BOTTOM_RETRACT_SPEED: usize = 0xC0;
pub(crate) const BOTTOM_RETRACT_SPEED_2: usize = 0xC4;
pub(crate) const FOUR: [usize; 2] = [0xCC, 0xD4];
pub(crate) const REST_AFTER_RETRACT_S: usize = 0xD8;
pub(crate) const REST_AFTER_LIFT_S: usize = 0xDC;
pub(crate) const REST_BEFORE_LIFT_S: usize = 0xE0;
pub(crate) const BOTTOM_RETRACT_MM_2: usize = 0xE4;
pub(crate) const LAYER_TIME_S: usize = 0xE8;
pub(crate) const LARGEST_LAYER_BYTES: usize = 0xEC;
pub(crate) const LAYER_COUNT_AGAIN: usize = 0xF4;
pub(crate) const DISCLAIMER: usize = 0x108;
/// A word set to one behind the disclaimer's length in every file.
pub(crate) const ONE_AGAIN: usize = 0x110;
pub(crate) const RESIN: usize = 0x114;

/// The deciphered block, read field by field.
pub(crate) struct Settings(pub [u8; SETTINGS_BYTES]);

impl Settings {
    pub(crate) fn u16(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.0[at], self.0[at + 1]])
    }

    pub(crate) fn u32(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.word(at))
    }

    pub(crate) fn u64(&self, at: usize) -> u64 {
        u64::from(self.u32(at)) | u64::from(self.u32(at + 4)) << 32
    }

    pub(crate) fn f32(&self, at: usize) -> f32 {
        f32::from_le_bytes(self.word(at))
    }

    pub(crate) fn put_u16(&mut self, at: usize, value: u16) {
        self.0[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_u32(&mut self, at: usize, value: u32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_f32(&mut self, at: usize, value: f32) {
        self.0[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn word(&self, at: usize) -> [u8; 4] {
        [self.0[at], self.0[at + 1], self.0[at + 2], self.0[at + 3]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_field_ends_the_block() {
        // Two padding words follow the resin address, and the block is whole after them.
        assert_eq!(RESIN + 4 + 8, SETTINGS_BYTES);
    }
}
