//! The cipher the container's settings and signature are covered with: AES-256 in CBC mode
//! under one key every file shares. See `docs/formats/ctb-encrypted.md`.

use aes::Aes256;
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::NoPadding};
use core_format::FormatError;
use sha2::{Digest, Sha256};

/// The block every enciphered span is a whole number of.
pub(crate) const BLOCK_BYTES: usize = 16;

// Kept masked so the bytes do not turn up in a plain search of the source; unmasked on use.
const MASK: &[u8; 7] = b"encrust";
const KEY: [u8; 32] = unmasked([
    0xB5, 0x35, 0xED, 0x41, 0x04, 0xAD, 0x49, 0x7F, 0x8B, 0x2C, 0x50, 0xA8, 0xAC, 0x2F, 0x98, 0xFA,
    0xC8, 0x2F, 0x11, 0x49, 0xE9, 0x1B, 0xD1, 0xCC, 0x30, 0x76, 0x80, 0x64, 0xBD, 0x3C, 0x49, 0x98,
]);
const IV: [u8; 16] = unmasked([
    0x6A, 0x6F, 0x69, 0x77, 0x70, 0x78, 0x72, 0x62, 0x66, 0x65, 0x78, 0x79, 0x7F, 0x79, 0x6C, 0x61,
]);

const fn unmasked<const N: usize>(mut bytes: [u8; N]) -> [u8; N] {
    let mut index = 0;
    while index < N {
        bytes[index] ^= MASK[index % MASK.len()];
        index += 1;
    }
    bytes
}

/// Deciphers `data` in place; `what` names it for the error.
pub(crate) fn decrypt(data: &mut [u8], what: &'static str) -> Result<(), FormatError> {
    aligned(data, what)?;
    let length = data.len();
    cbc::Decryptor::<Aes256>::new(&KEY.into(), &IV.into())
        .decrypt_padded::<NoPadding>(data)
        .map_err(|_| FormatError::BadBlock {
            what,
            size: length as u64,
        })?;
    Ok(())
}

/// Enciphers `data` in place; `what` names it for the error.
pub(crate) fn encrypt(data: &mut [u8], what: &'static str) -> Result<(), FormatError> {
    aligned(data, what)?;
    let length = data.len();
    cbc::Encryptor::<Aes256>::new(&KEY.into(), &IV.into())
        .encrypt_padded::<NoPadding>(data, length)
        .map_err(|_| FormatError::BadBlock {
            what,
            size: length as u64,
        })?;
    Ok(())
}

/// The signature a file carries for its settings' `checksum`: the enciphered SHA-256 of it.
pub(crate) fn signature(checksum: u64) -> [u8; 32] {
    let mut signature: [u8; 32] = Sha256::digest(checksum.to_le_bytes()).into();
    // Thirty-two bytes are two whole blocks, so this cannot fail.
    let _ = encrypt(&mut signature, "signature");
    signature
}

fn aligned(data: &[u8], what: &'static str) -> Result<(), FormatError> {
    if data.len().is_multiple_of(BLOCK_BYTES) {
        Ok(())
    } else {
        Err(FormatError::BadBlock {
            what,
            size: data.len() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_enciphered_deciphers_back() {
        let mut data = *b"thirty-two bytes of plain text!!";
        let plain = data;
        encrypt(&mut data, "test").expect("two whole blocks");
        assert_ne!(data, plain);
        decrypt(&mut data, "test").expect("two whole blocks");
        assert_eq!(data, plain);
    }

    #[test]
    fn a_span_short_of_a_block_is_refused() {
        let refused = decrypt(&mut [0; 17], "settings");
        assert!(matches!(
            refused,
            Err(FormatError::BadBlock {
                what: "settings",
                size: 17
            })
        ));
    }
}
