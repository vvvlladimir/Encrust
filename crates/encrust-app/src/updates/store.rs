//! What the Microsoft Store holds for a copy installed from it (ADR 0292).

use windows::Foundation::Uri;
use windows::Services::Store::StoreContext;
use windows::System::Launcher;
use windows::core::HSTRING;

use super::feed::{Offer, Version};
use super::{STORE_LINK, UpdateError};

/// The package the Store would update this copy to, when it has one. Only a look: the Store
/// downloads and installs it, and shows nothing until asked.
pub fn check() -> Result<Option<Offer>, UpdateError> {
    let updates = StoreContext::GetDefault()
        .and_then(|store| store.GetAppAndOptionalStorePackageUpdatesAsync())
        .and_then(|look| look.join())
        .map_err(UpdateError::Store)?;
    let Some(update) = updates.into_iter().next() else {
        return Ok(None);
    };
    let version = update
        .Package()
        .and_then(|package| package.Id())
        .and_then(|id| id.Version())
        .map_err(UpdateError::Store)?;
    // The package carries the release as `major.minor.patch.0`; see release.yml.
    let version = Version::parse(&format!(
        "{}.{}.{}",
        version.Major, version.Minor, version.Build
    ))?;
    Ok(Some(Offer {
        version,
        notes: Vec::new(),
        download: None,
        published_s: None,
    }))
}

/// Opens this app's page in the Store app, where its Update button is.
pub fn open() -> Result<(), UpdateError> {
    Uri::CreateUri(&HSTRING::from(STORE_LINK))
        .and_then(|uri| Launcher::LaunchUriAsync(&uri))
        .and_then(|launch| launch.join())
        .map(|_| ())
        .map_err(UpdateError::Store)
}
