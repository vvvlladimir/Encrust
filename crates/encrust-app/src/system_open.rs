//! Files macOS asks the window to open: a double-click in Finder, a drop on the Dock icon,
//! `open`. They come as an Apple Event, never on the command line; see `docs/decisions/0445`.

use std::cell::RefCell;
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSObject,
};

use crate::files::{Arrived, Handed, Wanted};

/// The Apple Event Manager's four-character codes: the core suite, Open Documents, and the
/// parameter holding the files.
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const OPEN_DOCUMENTS: u32 = u32::from_be_bytes(*b"odoc");
const DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "EncrustOpenDocuments"]
    struct Listener;

    impl Listener {
        #[unsafe(method(willFinishLaunching:))]
        fn will_finish_launching(&self, _note: &NSNotification) {
            // AppKit puts its own handler in before this note, so one put in earlier is replaced.
            let manager = NSAppleEventManager::sharedAppleEventManager();
            // SAFETY: the selector below takes the event and its reply, and `self` lives in
            // `LISTENER` for the whole run.
            let () = unsafe {
                msg_send![
                    &manager,
                    setEventHandler: self,
                    andSelector: sel!(openDocuments:withReplyEvent:),
                    forEventClass: CORE_EVENT_CLASS,
                    andEventID: OPEN_DOCUMENTS
                ]
            };
        }

        #[unsafe(method(openDocuments:withReplyEvent:))]
        fn open_documents(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            // SAFETY: the method takes a four-character code and answers a descriptor or nil.
            let files: Option<Retained<NSAppleEventDescriptor>> =
                unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
            let Some(files) = files else {
                return;
            };
            HANDED.with_borrow_mut(|handed| handed.extend(paths(&files)));
            WAKE.with_borrow(|wake| {
                if let Some(ctx) = wake {
                    ctx.request_repaint();
                }
            });
        }
    }
);

impl Listener {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: `init` is NSObject's designated initialiser, called once on a fresh object.
        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    // The notification centre and the event manager hold their target weakly.
    static LISTENER: RefCell<Option<Retained<Listener>>> = const { RefCell::new(None) };
    static HANDED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static WAKE: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

/// Listens for the files macOS hands over, from before the app finishes launching, so the
/// file that launched it is caught too. Called once on the main thread, before the window.
pub fn listen() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let listener = Listener::new(mtm);
    // SAFETY: AppKit's own name, read once the binary is loaded; the selector takes the note.
    unsafe {
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            &listener,
            sel!(willFinishLaunching:),
            Some(NSApplicationWillFinishLaunchingNotification),
            None,
        );
    }
    LISTENER.set(Some(listener));
}

/// Wakes the window when a file is handed over while it waits for input.
pub fn wake(ctx: &egui::Context) {
    WAKE.set(Some(ctx.clone()));
}

/// The files handed over since the last frame, each opened by what it is.
pub fn taken() -> Vec<Arrived> {
    HANDED
        .take()
        .into_iter()
        .map(|path| Arrived::File(Wanted::ModelOrProject, Handed::Path(path)))
        .collect()
}

/// The paths in an event's list of files; one file may come as itself, not as a list of one.
fn paths(files: &NSAppleEventDescriptor) -> Vec<PathBuf> {
    let urls: Vec<_> = match files.numberOfItems() {
        count if count < 1 => vec![files.fileURLValue()],
        count => (1..=count)
            .map(|index| files.descriptorAtIndex(index)?.fileURLValue())
            .collect(),
    };
    urls.into_iter()
        .filter_map(|url| url?.path())
        .map(|path| PathBuf::from(path.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use objc2_foundation::{NSAppleEventDescriptor, NSString, NSURL};

    use super::paths;

    fn file(path: &str) -> objc2::rc::Retained<NSAppleEventDescriptor> {
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        NSAppleEventDescriptor::descriptorWithFileURL(&url)
    }

    #[test]
    fn every_file_of_a_list_is_taken_in_order() {
        let list = NSAppleEventDescriptor::listDescriptor();
        list.insertDescriptor_atIndex(&file("/tmp/a.encrust"), 1);
        list.insertDescriptor_atIndex(&file("/tmp/b.stl"), 2);
        assert_eq!(
            paths(&list),
            [PathBuf::from("/tmp/a.encrust"), PathBuf::from("/tmp/b.stl")],
            "both files, first first"
        );
    }

    #[test]
    fn a_single_file_is_taken_without_a_list() {
        assert_eq!(
            paths(&file("/tmp/a.encrust")),
            [PathBuf::from("/tmp/a.encrust")],
            "the one file"
        );
    }
}
