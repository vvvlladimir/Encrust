//! The Anycubic containers: one table container under twenty extensions, at version 1 and
//! at revisions 516, 517 and 518, the PWSZ archive under five, and the Photon S's
//! `.photons`. Layouts are in `docs/formats/anycubic.md`, `pwsz.md` and `photons.md`.

mod bit_planes;
mod blocks;
mod extent;
#[cfg(test)]
mod fixtures;
mod photons;
mod pwsz;
mod reader;
mod rle;
mod tables;
mod writer;

pub use photons::{OpenPhotons, PhotonsLayer, PhotonsReader, PhotonsSink, PhotonsWriter};
pub use pwsz::{
    OpenPwsz, PwszFlavour, PwszLayer, PwszReader, PwszSink, PwszWriter, claims as pwsz_claims,
};
pub use reader::{AnycubicReader, OpenAnycubic, PwsReader};
pub use rle::{EncodedLayer, GREY_STEPS, decode};
pub use writer::{AnycubicFlavour, AnycubicLayer, AnycubicSink, AnycubicVersion, AnycubicWriter};
