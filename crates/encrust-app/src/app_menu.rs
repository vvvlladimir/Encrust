//! The application menu macOS shows under the app's name, in place of the one winit builds from
//! the binary's name; see `docs/decisions/0444`.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionVersion, NSApplication,
    NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSDictionary, NSObject, NSString, ns_string};

use crate::i18n::{Language, tr};
use crate::settings::Section;
use crate::state::Machine;

/// What a row of the menu asks of the window, which only the window's own frame may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Settings,
    CheckForUpdates,
}

/// What the rows that reach the window call, and what they asked since the last frame.
struct Ivars {
    ctx: egui::Context,
    asked: RefCell<Vec<Command>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "EncrustAppMenuTarget"]
    #[ivars = Ivars]
    struct Target;

    impl Target {
        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            show_about(MainThreadMarker::from(self));
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            self.ask(Command::Settings);
        }

        #[unsafe(method(checkForUpdates:))]
        fn check_for_updates(&self, _sender: Option<&AnyObject>) {
            self.ask(Command::CheckForUpdates);
        }
    }
);

impl Target {
    fn new(mtm: MainThreadMarker, ctx: egui::Context) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars {
            ctx,
            asked: RefCell::default(),
        });
        // SAFETY: `init` is NSObject's designated initialiser, called once on a fresh object.
        unsafe { msg_send![super(this), init] }
    }

    fn ask(&self, command: Command) {
        self.ivars().asked.borrow_mut().push(command);
        self.ivars().ctx.request_repaint();
    }
}

thread_local! {
    // A menu item holds its target weakly, so the target lives here for the whole run.
    static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
    static BUILT_IN: Cell<Option<Language>> = const { Cell::new(None) };
}

/// Puts the menu up in the window's language, again when that changes, and does what its
/// rows asked since the last frame. Called every frame on the main thread.
pub fn follow(ctx: &egui::Context, machine: &mut Machine) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let language = crate::i18n::current();
    if BUILT_IN.get() != Some(language) {
        let target = TARGET.with_borrow_mut(|target| {
            target
                .get_or_insert_with(|| Target::new(mtm, ctx.clone()))
                .clone()
        });
        install(mtm, &target);
        BUILT_IN.set(Some(language));
    }
    let asked = TARGET.with_borrow(|target| {
        target
            .as_ref()
            .map(|target| target.ivars().asked.take())
            .unwrap_or_default()
    });
    for command in asked {
        answer(command, machine);
    }
}

fn answer(command: Command, machine: &mut Machine) {
    if !machine.settings.open {
        crate::panels::toggle_settings(machine);
    }
    if command == Command::CheckForUpdates {
        machine.settings.turn_to(Section::Updates);
        machine.updates.check_now();
    }
}

/// The standard About panel, its version this binary's: the window replaces its binaries
/// when it updates itself and leaves the bundle's Info.plist as it was installed.
fn show_about(mtm: MainThreadMarker) {
    let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
    // SAFETY: AppKit's own keys, read once the binary is loaded.
    let keys = unsafe {
        [
            NSAboutPanelOptionApplicationVersion,
            NSAboutPanelOptionVersion,
        ]
    };
    // An empty build version leaves the parenthesis off rather than show the bundle's.
    let values: [&AnyObject; 2] = [&version, ns_string!("")];
    let options = NSDictionary::from_slices(&keys, &values);
    // SAFETY: both values are strings, the type the two keys take.
    unsafe {
        NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanelWithOptions(&options)
    };
}

fn install(mtm: MainThreadMarker, target: &Target) {
    let app = NSApplication::sharedApplication(mtm);
    let services = NSMenu::new(mtm);
    let menu = NSMenu::new(mtm);
    for item in &rows(mtm, target, &services) {
        menu.addItem(item);
    }
    let bar = NSMenu::new(mtm);
    // macOS titles the first menu with the app's name whatever the item says.
    bar.addItem(&submenu(mtm, "Encrust", &menu));
    app.setServicesMenu(Some(&services));
    app.setMainMenu(Some(&bar));
}

/// The rows in the order Apple's guidelines give them, in the window's language.
fn rows(mtm: MainThreadMarker, target: &Target, services: &NSMenu) -> [Retained<NSMenuItem>; 12] {
    let own = |title: String, action: Sel| {
        let item = row(mtm, &title, Some(action), "", None);
        // SAFETY: the target outlives the item, kept in `TARGET` for the whole run.
        unsafe { item.setTarget(Some(target)) };
        item
    };
    [
        own(tr!("app-menu-about"), sel!(showAbout:)),
        NSMenuItem::separatorItem(mtm),
        own(tr!("app-menu-settings"), sel!(openSettings:)),
        own(tr!("app-menu-check-for-updates"), sel!(checkForUpdates:)),
        NSMenuItem::separatorItem(mtm),
        submenu(mtm, &tr!("app-menu-services"), services),
        NSMenuItem::separatorItem(mtm),
        row(mtm, &tr!("app-menu-hide"), Some(sel!(hide:)), "h", None),
        row(
            mtm,
            &tr!("app-menu-hide-others"),
            Some(sel!(hideOtherApplications:)),
            "h",
            Some(NSEventModifierFlags::Command | NSEventModifierFlags::Option),
        ),
        row(
            mtm,
            &tr!("app-menu-show-all"),
            Some(sel!(unhideAllApplications:)),
            "",
            None,
        ),
        NSMenuItem::separatorItem(mtm),
        row(
            mtm,
            &tr!("app-menu-quit"),
            Some(sel!(terminate:)),
            "q",
            None,
        ),
    ]
}

/// A row with a key and modifiers, Command alone when none are given.
fn row(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
    modifiers: Option<NSEventModifierFlags>,
) -> Retained<NSMenuItem> {
    // SAFETY: the action is one of AppKit's or `Target`'s, each taking the sender alone.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    };
    if let Some(modifiers) = modifiers {
        item.setKeyEquivalentModifierMask(modifiers);
    }
    item
}

fn submenu(mtm: MainThreadMarker, title: &str, menu: &NSMenu) -> Retained<NSMenuItem> {
    let item = row(mtm, title, None, "", None);
    menu.setTitle(&NSString::from_str(title));
    item.setSubmenu(Some(menu));
    item
}

#[cfg(test)]
mod tests {
    use super::{Command, answer};
    use crate::state::Machine;

    #[test]
    fn settings_row_opens_settings_and_never_closes_them() {
        let mut machine = Machine::default();
        answer(Command::Settings, &mut machine);
        assert!(machine.settings.open, "the first press opens Settings");
        answer(Command::Settings, &mut machine);
        assert!(machine.settings.open, "a second press leaves Settings open");
    }
}
