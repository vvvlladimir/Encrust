//! The Chitu container family: `.ctb` versions 4 and 5, and the older `.cbddlp` and
//! `.photon`. Layout is in `docs/formats/chitu.md`.

mod blocks;
mod cbddlp;
mod crypt;
#[cfg(test)]
mod fixtures;
mod layer;
mod reader;
mod rle1;
mod writer;

#[cfg(test)]
pub(crate) use blocks::HEADER_BYTES;
pub use cbddlp::{CbddlpFlavour, CbddlpSink, CbddlpWriter};
pub use crypt::layer_crypt;
pub use reader::{ChituReader, OpenChitu};
pub use rle1::{EncodedPasses, GREY_PASSES, decode_passes};
pub use writer::{CtbSink, CtbVersion, CtbWriter};
