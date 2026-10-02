//! The check a downloaded archive passes before a byte of it is unpacked.

use minisign_verify::{PublicKey, Signature};

use super::{UpdateError, Version};

/// The public half of the project's release key. A build refuses an archive signed by
/// anybody else, whoever controls the feed or the connection.
pub const RELEASE_KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/update-key.pub"
));

/// What `release.yml` signs into each archive's trusted comment, so a signature cannot be
/// moved to another archive or replayed for an older version.
pub fn trusted_comment(version: Version, archive: &str) -> String {
    format!("encrust {version} {archive}")
}

/// Checks `bytes` against a detached minisign `signature` made with `key`, and that the
/// signature was made for `expected`.
pub fn verify(key: &str, bytes: &[u8], signature: &str, expected: &str) -> Result<(), UpdateError> {
    let key = PublicKey::decode(key).map_err(UpdateError::Signature)?;
    let signature = Signature::decode(signature).map_err(UpdateError::Signature)?;
    key.verify(bytes, &signature, false)
        .map_err(UpdateError::Signature)?;
    // Read only once the signature holds, since it covers the comment too.
    let found = signature.trusted_comment();
    if found != expected {
        return Err(UpdateError::Signed {
            expected: expected.to_owned(),
            found: found.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/update")
            .join(name);
        std::fs::read(path).expect("the update fixtures are checked in")
    }

    fn text(name: &str) -> String {
        String::from_utf8(fixture(name)).expect("a key or a signature is text")
    }

    /// Signed by `tests/fixtures/update/test-key.pub` with this trusted comment.
    const SIGNED_FOR: &str = "encrust 0.2.0 encrust-test.tar.gz";

    #[test]
    fn the_shipped_key_is_a_minisign_public_key() {
        assert!(PublicKey::decode(RELEASE_KEY).is_ok());
    }

    #[test]
    fn the_comment_names_the_version_and_the_archive() {
        let version = Version::parse("0.2.0").expect("a release version");
        assert_eq!(trusted_comment(version, "encrust-test.tar.gz"), SIGNED_FOR);
    }

    #[test]
    fn an_archive_signed_with_the_key_passes() {
        let checked = verify(
            &text("test-key.pub"),
            &fixture("encrust-test.tar.gz"),
            &text("encrust-test.tar.gz.minisig"),
            SIGNED_FOR,
        );
        assert!(checked.is_ok(), "{checked:?}");
    }

    #[test]
    fn an_archive_signed_by_anybody_else_is_refused() {
        let checked = verify(
            &text("test-key.pub"),
            &fixture("encrust-test.tar.gz"),
            &text("encrust-test.tar.gz.stranger.minisig"),
            SIGNED_FOR,
        );
        assert!(matches!(checked, Err(UpdateError::Signature(_))));
    }

    #[test]
    fn an_archive_changed_after_signing_is_refused() {
        let mut archive = fixture("encrust-test.tar.gz");
        let last = archive.len() - 1;
        archive[last] ^= 1;
        let checked = verify(
            &text("test-key.pub"),
            &archive,
            &text("encrust-test.tar.gz.minisig"),
            SIGNED_FOR,
        );
        assert!(matches!(checked, Err(UpdateError::Signature(_))));
    }

    #[test]
    fn the_test_key_is_not_the_release_key() {
        let checked = verify(
            RELEASE_KEY,
            &fixture("encrust-test.tar.gz"),
            &text("encrust-test.tar.gz.minisig"),
            SIGNED_FOR,
        );
        assert!(matches!(checked, Err(UpdateError::Signature(_))));
    }

    /// A sound signature of an older release, served as the newer one, is a downgrade.
    #[test]
    fn a_signature_made_for_another_version_is_refused() {
        let checked = verify(
            &text("test-key.pub"),
            &fixture("encrust-test.tar.gz"),
            &text("encrust-test.tar.gz.minisig"),
            "encrust 0.3.0 encrust-test.tar.gz",
        );
        assert!(matches!(checked, Err(UpdateError::Signed { .. })));
    }
}
