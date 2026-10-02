/// Applies the `.ctb` layer cipher to one layer's run-length data, in place.
///
/// A four-byte keystream word is derived from the header's key and the layer's index and
/// exhausted a byte at a time, then advanced. It is an exclusive or, so the one function
/// both enciphers and deciphers. See `docs/formats/chitu.md`.
///
/// A key of zero means the data is in the clear and nothing is done, which is what we
/// write; see `docs/decisions/0046-ctb-layer-data-is-written-in-the-clear.md`.
pub fn layer_crypt(key: u32, layer_index: u32, data: &mut [u8]) {
    if key == 0 {
        return;
    }
    let init = key.wrapping_mul(0x2D83_CDAC).wrapping_add(0xD8A8_3423);
    let mut word = layer_index
        .wrapping_mul(0x1E15_30CD)
        .wrapping_add(0xEC3D_47CD)
        .wrapping_mul(init);

    let mut byte_of_word = 0;
    for byte in data {
        *byte ^= (word >> (8 * byte_of_word)) as u8;
        byte_of_word += 1;
        if byte_of_word == 4 {
            word = word.wrapping_add(init);
            byte_of_word = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_of_zero_leaves_the_data_alone() {
        let mut data = [1, 2, 3, 4, 5];
        layer_crypt(0, 7, &mut data);
        assert_eq!(data, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn deciphering_is_the_same_pass_as_enciphering() {
        let plain: Vec<u8> = (0..64).collect();
        let mut data = plain.clone();
        layer_crypt(0x0378_32AA, 3, &mut data);
        assert_ne!(data, plain, "the data is actually covered");
        layer_crypt(0x0378_32AA, 3, &mut data);
        assert_eq!(data, plain);
    }

    #[test]
    fn each_layer_is_covered_by_its_own_keystream() {
        let mut first = vec![0; 16];
        let mut second = vec![0; 16];
        layer_crypt(0x0378_32AA, 0, &mut first);
        layer_crypt(0x0378_32AA, 1, &mut second);
        assert_ne!(first, second, "the layer index is part of the key");
    }

    #[test]
    fn the_keystream_word_advances_every_four_bytes() {
        // Zeroed input makes the output the keystream itself, so the first four bytes must
        // be one little-endian word and the fifth must begin another.
        let mut data = vec![0; 8];
        layer_crypt(1, 0, &mut data);
        let init = 1u32.wrapping_mul(0x2D83_CDAC).wrapping_add(0xD8A8_3423);
        let word = 0xEC3D_47CDu32.wrapping_mul(init);
        assert_eq!(data[..4], word.to_le_bytes());
        assert_eq!(data[4..8], word.wrapping_add(init).to_le_bytes()[..4]);
    }
}
