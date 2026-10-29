//! The encrypted Chitu `.ctb`: the plain container's fields under one enciphered settings
//! block, signed. Read and written so a user's file prints on the machine they own; see
//! `docs/formats/ctb-encrypted.md` and `docs/decisions/0298`.

mod cipher;
#[cfg(test)]
mod fixtures;
mod layout;
mod reader;
mod settings;
mod writer;

pub use reader::{CtbEncryptedReader, OpenCtbEncrypted};
pub use writer::{CtbEncryptedSink, CtbEncryptedWriter};

/// Whether `head`, the first bytes of a file, are an encrypted `.ctb`'s.
pub fn claims(head: &[u8]) -> bool {
    head.get(..4)
        .is_some_and(|magic| magic == layout::MAGIC.to_le_bytes())
}
