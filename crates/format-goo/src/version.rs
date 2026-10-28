use std::io;

use core_format::{Fields, FormatError, ReadSeek, Reads};

/// Which revision of the container a file is: the machine's profile names it, since a
/// firmware reads one and not the other. See `docs/formats/goo.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GooVersion {
    /// `V3.0`, big-endian, one mask a layer behind its definition.
    #[default]
    V3,
    /// `V5.1`, little-endian, tables up front and each mask in two halves.
    V5,
}

impl GooVersion {
    /// The four bytes that open the file.
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::V3 => "V3.0",
            Self::V5 => "V5.1",
        }
    }

    pub(crate) fn order(self) -> ByteOrder {
        match self {
            Self::V3 => ByteOrder::Big,
            Self::V5 => ByteOrder::Little,
        }
    }

    /// The first-stage retract a file states for a layer `thickness_mm` thick. Version 5.1
    /// states the travel down, the plate ending a layer higher; 3.0 the resin's own.
    pub(crate) fn retract_mm(self, retract_mm: f32, thickness_mm: f32) -> f32 {
        match self {
            Self::V3 => retract_mm,
            Self::V5 => retract_mm - thickness_mm,
        }
    }

    /// The resin's retract a file stating `stated_mm` was written from.
    pub(crate) fn resin_retract_mm(self, stated_mm: f32, thickness_mm: f32) -> f32 {
        match self {
            Self::V3 => stated_mm,
            Self::V5 => stated_mm + thickness_mm,
        }
    }
}

/// How a revision lays out its multi-byte fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ByteOrder {
    Big,
    Little,
}

impl ByteOrder {
    pub(crate) fn put_u16(self, fields: &mut Fields<'_>, value: u16) -> io::Result<()> {
        match self {
            Self::Big => fields.u16_be(value),
            Self::Little => fields.u16_le(value),
        }
    }

    pub(crate) fn put_u32(self, fields: &mut Fields<'_>, value: u32) -> io::Result<()> {
        match self {
            Self::Big => fields.u32_be(value),
            Self::Little => fields.u32_le(value),
        }
    }

    pub(crate) fn put_f32(self, fields: &mut Fields<'_>, value: f32) -> io::Result<()> {
        match self {
            Self::Big => fields.f32_be(value),
            Self::Little => fields.f32_le(value),
        }
    }

    pub(crate) fn u16<S: ReadSeek>(self, reads: &mut Reads<S>) -> Result<u16, FormatError> {
        match self {
            Self::Big => reads.u16_be(),
            Self::Little => reads.u16_le(),
        }
    }

    pub(crate) fn u32<S: ReadSeek>(self, reads: &mut Reads<S>) -> Result<u32, FormatError> {
        match self {
            Self::Big => reads.u32_be(),
            Self::Little => reads.u32_le(),
        }
    }

    pub(crate) fn f32<S: ReadSeek>(self, reads: &mut Reads<S>) -> Result<f32, FormatError> {
        match self {
            Self::Big => reads.f32_be(),
            Self::Little => reads.f32_le(),
        }
    }
}
