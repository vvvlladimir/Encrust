use super::*;

#[test]
fn eight_hex_digits_carry_the_opacity() {
    assert_eq!(parse_hex("#12AB3480"), Some([0x12, 0xAB, 0x34, 0x80]));
}

#[test]
fn six_hex_digits_are_opaque_and_the_hash_is_optional() {
    assert_eq!(parse_hex("12ab34"), Some([0x12, 0xAB, 0x34, 0xFF]));
}

#[test]
fn anything_but_six_or_eight_hex_digits_is_refused() {
    for text in ["", "#123", "#12AB3", "#12AB34GG", "#12AB34800", "#ÄÄÄÄÄÄ"] {
        assert_eq!(parse_hex(text), None, "{text:?}");
    }
}

#[test]
fn a_colour_written_as_hex_reads_back_the_same() {
    let rgba = [0x01, 0x80, 0xFE, 0x33];
    assert_eq!(parse_hex(&hex_of(rgba)), Some(rgba));
}
