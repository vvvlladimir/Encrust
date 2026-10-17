//! The fixed sizes and marks of the container, and how it addresses past four gigabytes.

/// The magic heading every encrypted `.ctb`.
pub(crate) const MAGIC: u32 = 0x12FD_0107;

/// The plain header in front of the settings block.
pub(crate) const HEADER_BYTES: u64 = 48;

/// One row of the layer pointer table.
pub(crate) const POINTER_BYTES: u64 = 16;

/// The definition in front of each layer's data.
pub(crate) const LAYER_DEF_BYTES: u64 = 88;

/// The resin block of version 5, without the strings behind it.
pub(crate) const RESIN_BYTES: u64 = 40;

/// The file address a page and an offset within it name; a page is four gigabytes.
pub(crate) fn page_address(page: u32, offset: u32) -> u64 {
    u64::from(page) << 32 | u64::from(offset)
}

/// `address` as its page and the offset within it.
pub(crate) fn paged(address: u64) -> (u32, u32) {
    ((address >> 32) as u32, address as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_past_four_gigabytes_is_its_page_and_offset() {
        let address = 5 * (1 << 30) + 12;
        let (page, offset) = paged(address);
        assert_eq!((page, offset), (1, (1 << 30) + 12));
        assert_eq!(page_address(page, offset), address);
    }
}
