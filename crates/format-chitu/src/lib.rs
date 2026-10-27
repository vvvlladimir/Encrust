//! The Chitu container family: `.ctb` versions 4 and 5, the older `.cbddlp` and
//! `.photon`, `.fdg` and `.phz`. Layout is in `docs/formats/chitu.md`.

mod blocks;
mod cbddlp;
mod fdg;
#[cfg(test)]
mod fixtures;
mod layer;
mod phz;
mod reader;
mod rle1;
mod writer;

#[cfg(test)]
pub(crate) use blocks::HEADER_BYTES;
pub use cbddlp::{CbddlpFlavour, CbddlpSink, CbddlpWriter};
pub use fdg::{FdgLayer, FdgReader, FdgSink, FdgWriter, OpenFdg};
pub use phz::{OpenPhz, PhzReader, PhzWriter};
pub use reader::{ChituReader, OpenChitu};
pub use rle1::{EncodedPasses, GREY_PASSES, decode_passes};
pub use writer::{CtbSink, CtbVersion, CtbWriter};
