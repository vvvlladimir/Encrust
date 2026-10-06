//! A browser's file dialogs and downloads. A dialog is a file input the page clicks for
//! the user; what it picks is read on later turns of the page's loop and waits in
//! [`arrived`] for the next frame. A download is started by the page, whichever thread
//! made the file.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, FileList, HtmlInputElement};

use crate::files::{Arrived, SIBLINGS, Wanted, together};
use crate::job::{Outcome, Progress, SliceRequest, run_into};
use crate::web::opfs;

thread_local! {
    static ARRIVED: RefCell<Vec<Arrived>> = const { RefCell::new(Vec::new()) };
}

/// Opens the browser's file dialog. A model may come with its materials and textures, so
/// several files may be picked for one.
pub fn pick(wanted: Wanted) {
    if let Err(error) = open_input(wanted) {
        arrive(Arrived::Failed(format!(
            "cannot open the file dialog: {}",
            opfs::failed(error)
        )));
    }
}

pub fn arrived() -> Vec<Arrived> {
    ARRIVED.with(|arrived| std::mem::take(&mut *arrived.borrow_mut()))
}

#[wasm_bindgen(module = "/src/web/thread.js")]
extern "C" {
    #[wasm_bindgen(js_name = offerFile)]
    fn offer_file(name: &str, blob: &Blob);
}

/// Hands `bytes` to the user as a file called `name`, from any thread.
pub fn download(name: &str, bytes: &[u8]) -> Result<(), JsValue> {
    // A copy out of the shared memory: a `Blob` refuses a view into a shared buffer.
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    offer_file(name, &Blob::new_with_u8_array_sequence(&parts)?);
    Ok(())
}

/// The one entry in private storage a sliced file is written to, whatever it is called, so
/// the storage holds the last file and never every one the page has made.
const STORED: &str = "sliced-file";

/// Hands the sliced file in private storage to the user under `name`, as the disk holds it.
pub async fn download_stored(name: &str) -> std::io::Result<()> {
    let file: Blob = opfs::read(STORED).await?.into();
    offer_file(name, &file);
    Ok(())
}

/// Writes the sliced file and hands it to the user, on a worker. It goes to private storage,
/// so the stack never sits in memory; a browser that keeps none there, such as a private
/// window, gets it written in memory instead.
pub async fn slice_offered(
    request: &SliceRequest,
    cancel: &AtomicBool,
    report: &mut (dyn FnMut(Progress) + Send),
) -> Outcome {
    let name = request.output.to_string_lossy().into_owned();
    let (outcome, offered) = match opfs::StoredFile::create(STORED).await {
        Ok(file) => {
            // Every write is a call into the browser, so they go out a megabyte at a time.
            let mut sink = std::io::BufWriter::with_capacity(1 << 20, file);
            let outcome = run_into(request, &mut sink, cancel, report);
            // Flushed here rather than on drop, which would lose a failure; the handle
            // closes with it, so the page can read the file.
            let flushed = sink
                .into_inner()
                .map(drop)
                .map_err(|error| error.into_error());
            let offered = match (&outcome, flushed) {
                (_, Err(error)) => Err(error),
                (Outcome::Written { .. }, Ok(())) => download_stored(&name).await,
                _ => Ok(()),
            };
            (outcome, offered)
        }
        Err(_) => {
            let mut sink = std::io::Cursor::new(Vec::new());
            let outcome = run_into(request, &mut sink, cancel, report);
            let offered = match outcome {
                Outcome::Written { .. } => download(&name, sink.get_ref()).map_err(opfs::failed),
                _ => Ok(()),
            };
            (outcome, offered)
        }
    };
    match offered {
        Ok(()) => outcome,
        Err(error) => Outcome::Failed(format!("cannot hand over {name}: {error}")),
    }
}

fn open_input(wanted: Wanted) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let input: HtmlInputElement = document.create_element("input")?.unchecked_into();
    input.set_type("file");
    input.set_accept(&accept(wanted));
    input.set_multiple(wanted.takes_several());

    let read = input.clone();
    let changed = Closure::once_into_js(move || {
        if let Some(files) = read.files() {
            spawn_local(read_all(wanted, files));
        }
    });
    input.add_event_listener_with_callback("change", changed.unchecked_ref())?;
    input.click();
    Ok(())
}

/// What the dialog lets through: a model's own extensions and those of its siblings.
fn accept(wanted: Wanted) -> String {
    let siblings: &[&str] = if wanted.takes_several() {
        &SIBLINGS
    } else {
        &[]
    };
    wanted
        .extensions()
        .iter()
        .chain(siblings)
        .map(|extension| format!(".{extension}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Reads files dropped on the window; they arrive together, so a model brings its
/// textures with it.
pub fn read_dropped(dropped: Vec<egui::DroppedFileHandle>) {
    if dropped.is_empty() {
        return;
    }
    spawn_local(async move {
        let mut read = Vec::new();
        for file in &dropped {
            let name = file.path().to_string_lossy().into_owned();
            match file.bytes_async().await {
                Ok(bytes) => read.push((name, Arc::from(bytes))),
                Err(error) => {
                    return arrive(Arrived::Failed(format!("cannot read {name}: {error}")));
                }
            }
        }
        for handed in together(read) {
            arrive(Arrived::Dropped(handed));
        }
    });
}

async fn read_all(wanted: Wanted, files: FileList) {
    let mut read = Vec::new();
    for index in 0..files.length() {
        let Some(file) = files.get(index) else {
            continue;
        };
        match JsFuture::from(file.array_buffer()).await {
            Ok(buffer) => {
                let bytes: Arc<[u8]> = js_sys::Uint8Array::new(&buffer).to_vec().into();
                read.push((file.name(), bytes));
            }
            Err(error) => {
                arrive(Arrived::Failed(format!(
                    "cannot read {}: {}",
                    file.name(),
                    opfs::failed(error)
                )));
                return;
            }
        }
    }
    let handed = if wanted.takes_several() {
        together(read)
    } else {
        read.into_iter()
            .map(|(name, bytes)| crate::files::Handed::bytes(name, bytes))
            .collect()
    };
    // The siblings picked with a mesh are read with it, not opened on their own.
    let mut asked_for = handed
        .into_iter()
        .filter(|handed| wanted.takes(handed.path()))
        .peekable();
    if asked_for.peek().is_none() {
        return arrive(Arrived::Failed(format!(
            "nothing to open among the files picked; pick a .{}",
            wanted.extensions().join(", .")
        )));
    }
    for handed in asked_for {
        arrive(Arrived::File(wanted, handed));
    }
}

fn arrive(arrived: Arrived) {
    ARRIVED.with(|list| list.borrow_mut().push(arrived));
    crate::web::repaint();
}
