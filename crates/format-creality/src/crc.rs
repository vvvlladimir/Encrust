//! The checksum a `.cxdlp` ends with: a reflected CRC-32 over every byte before it, with
//! no final inversion. See `docs/formats/creality.md`.
//!
//! The file is written once and never read back, so the two halves either side of the
//! layer-area table — which is only known after the layers that follow it — are summed
//! separately and joined by `combine`.

/// The polynomial, reflected, as the table is built from.
const POLYNOMIAL: u32 = 0xEDB8_8320;

/// A CRC taken over bytes as they are written.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Crc32 {
    state: u32,
    length: u64,
}

impl Crc32 {
    pub(crate) fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let index = ((self.state as u8) ^ byte) as usize;
            self.state = table()[index] ^ (self.state >> 8);
        }
        self.length += bytes.len() as u64;
    }

    pub(crate) fn value(self) -> u32 {
        self.state
    }

    /// This CRC followed by `next`, as if the bytes of both had been summed in order.
    pub(crate) fn followed_by(self, next: Self) -> Self {
        Self {
            state: combine(self.state, next.state, next.length),
            length: self.length + next.length,
        }
    }
}

/// The CRC of `a` followed by `b`, where `b` covered `length_b` bytes.
///
/// A CRC is linear over GF(2): running `length_b` zero bytes through the register is a
/// matrix, and squaring that matrix repeatedly reaches any length in a few dozen steps.
fn combine(a: u32, b: u32, length_b: u64) -> u32 {
    if length_b == 0 {
        return a;
    }

    // One zero bit, then one zero byte, as the bases every other length is built from.
    let mut odd = [0u32; 32];
    odd[0] = POLYNOMIAL;
    let mut row = 1u32;
    for cell in odd.iter_mut().skip(1) {
        *cell = row;
        row <<= 1;
    }

    let mut even = [0u32; 32];
    square(&mut even, &odd);
    square(&mut odd, &even);

    let mut crc = a;
    let mut left = length_b;
    loop {
        square(&mut even, &odd);
        if left & 1 != 0 {
            crc = times(&even, crc);
        }
        left >>= 1;
        if left == 0 {
            break;
        }

        square(&mut odd, &even);
        if left & 1 != 0 {
            crc = times(&odd, crc);
        }
        left >>= 1;
        if left == 0 {
            break;
        }
    }
    crc ^ b
}

/// The matrix `matrix` applied to one register value.
fn times(matrix: &[u32; 32], value: u32) -> u32 {
    let mut sum = 0;
    let mut value = value;
    let mut index = 0;
    while value != 0 {
        if value & 1 != 0 {
            sum ^= matrix[index];
        }
        value >>= 1;
        index += 1;
    }
    sum
}

/// `into` becomes `from` applied twice, which doubles the length it stands for.
fn square(into: &mut [u32; 32], from: &[u32; 32]) {
    for (cell, &row) in into.iter_mut().zip(from.iter()) {
        *cell = times(from, row);
    }
}

/// The byte table, built once on first use.
fn table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u32; 256];
        for (index, cell) in table.iter_mut().enumerate() {
            let mut value = index as u32;
            for _ in 0..8 {
                value = if value & 1 == 0 {
                    value >> 1
                } else {
                    (value >> 1) ^ POLYNOMIAL
                };
            }
            *cell = value;
        }
        table
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn over(bytes: &[u8]) -> Crc32 {
        let mut crc = Crc32::default();
        crc.update(bytes);
        crc
    }

    #[test]
    fn the_sum_is_the_reflected_crc_with_neither_inversion() {
        // Checked against the standard CRC-32, whose own value for this message is the
        // known 0xCBF43926: inverting the first four bytes stands in for the register
        // starting at all ones, and complementing the result stands in for the final xor.
        let mut inverted = *b"123456789";
        for byte in &mut inverted[..4] {
            *byte ^= 0xFF;
        }
        assert_eq!(!over(&inverted).value(), 0xCBF4_3926);

        assert_eq!(over(b"123456789").value(), 0x2DFD_2D88);
        assert_eq!(over(&[]).value(), 0);
    }

    #[test]
    fn two_halves_joined_are_the_sum_of_the_whole() {
        let whole = b"a header, a table of areas, and the layers behind it".as_slice();
        for split in [0, 1, 7, 20, whole.len() - 1, whole.len()] {
            let joined = over(&whole[..split]).followed_by(over(&whole[split..]));
            assert_eq!(
                joined.value(),
                over(whole).value(),
                "a split at {split} must sum to the same as the whole"
            );
            assert_eq!(joined.length, whole.len() as u64);
        }
    }

    #[test]
    fn three_parts_join_in_the_order_they_were_written() {
        let parts: [&[u8]; 3] = [b"head", b"table", b"layers"];
        let joined = over(parts[0])
            .followed_by(over(parts[1]))
            .followed_by(over(parts[2]));
        assert_eq!(joined.value(), over(b"headtablelayers").value());
    }
}
