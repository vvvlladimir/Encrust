//! The browser's private file system: where a sliced file is written as it is cut, so that
//! a stack of hundreds of megabytes never sits in the module's memory. A worker writes it
//! through a synchronous handle, which only a worker is given, and the page's thread hands
//! it to the user as a download.

use std::io::{self, Seek, SeekFrom, Write};

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
    FileSystemReadWriteOptions, FileSystemSyncAccessHandle, StorageManager,
};

/// A file in private storage, open for writing at any offset.
pub struct StoredFile {
    handle: FileSystemSyncAccessHandle,
    at: u64,
}

impl StoredFile {
    /// Creates `name`, emptying it if it was there. Only a worker may call this.
    pub async fn create(name: &str) -> io::Result<Self> {
        let file = file(name).await?;
        let handle: FileSystemSyncAccessHandle = JsFuture::from(file.create_sync_access_handle())
            .await
            .map_err(failed)?
            .unchecked_into();
        handle.truncate_with_f64(0.0).map_err(failed)?;
        Ok(Self { handle, at: 0 })
    }
}

impl Write for StoredFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let options = FileSystemReadWriteOptions::new();
        options.set_at(self.at as f64);
        let written = self
            .handle
            .write_with_u8_array_and_options(bytes, &options)
            .map_err(failed)? as usize;
        self.at += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.handle.flush().map_err(failed)
    }
}

impl Seek for StoredFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let from_end = || {
            self.handle
                .get_size()
                .map(|size| size as u64)
                .map_err(failed)
        };
        let at = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(delta) => from_end()?.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.at.checked_add_signed(delta),
        };
        self.at = at.ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(self.at)
    }
}

/// Closing hands the file back, so the page can read it and the next run can open it.
impl Drop for StoredFile {
    fn drop(&mut self) {
        let _ = self.handle.flush();
        self.handle.close();
    }
}

/// The file `name` in private storage, as the browser reads it: a `Blob` backed by disk.
pub async fn read(name: &str) -> io::Result<web_sys::File> {
    let file = file(name).await?;
    Ok(JsFuture::from(file.get_file())
        .await
        .map_err(failed)?
        .unchecked_into())
}

async fn file(name: &str) -> io::Result<FileSystemFileHandle> {
    let root: FileSystemDirectoryHandle = JsFuture::from(storage()?.get_directory())
        .await
        .map_err(failed)?
        .unchecked_into();
    let options = FileSystemGetFileOptions::new();
    options.set_create(true);
    Ok(
        JsFuture::from(root.get_file_handle_with_options(name, &options))
            .await
            .map_err(failed)?
            .unchecked_into(),
    )
}

/// `navigator.storage`, which a page and a worker both have under different types.
fn storage() -> io::Result<StorageManager> {
    let navigator = js_sys::Reflect::get(&js_sys::global(), &"navigator".into()).map_err(failed)?;
    let storage = js_sys::Reflect::get(&navigator, &"storage".into()).map_err(failed)?;
    if storage.is_undefined() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this browser has no private file storage",
        ));
    }
    Ok(storage.unchecked_into())
}

/// A JavaScript exception as an I/O error, its message kept.
pub fn failed(error: wasm_bindgen::JsValue) -> io::Error {
    let message = error.dyn_ref::<js_sys::Error>().map_or_else(
        || format!("{error:?}"),
        |error| String::from(error.message()),
    );
    io::Error::other(message)
}
