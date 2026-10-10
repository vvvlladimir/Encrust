//! macOS lays its window buttons out on a 28 point title bar, and the top bar is taller, so
//! they are moved onto its centre line; see `docs/decisions/0217`.

use objc2_app_kit::{NSView, NSWindowButton};
use objc2_foundation::NSPoint;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Where the close button's left edge stands, points from the window's left edge: as far
/// in as the buttons stand down from the bar's top, so they sit square in the corner.
pub const LEFT: f32 = 16.0;

/// Points the three buttons and the gap after them take from the bar's left end.
pub const ROOM: f32 = 84.0;

/// Moves the buttons onto the centre line of a bar `bar_h` points tall, unless they stand
/// there already. macOS lays them out again on a resize and on leaving full screen, so this
/// runs every frame and does nothing on almost all of them.
pub fn centre(frame: &eframe::Frame, bar_h: f32) {
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: the handle is the content view of the window this frame is drawing, alive for
    // the frame, and eframe calls the app on the main thread AppKit requires.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let Some(window) = view.window() else {
        return;
    };
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|kind| window.standardWindowButton(kind));
    let [Some(close), Some(minimise), Some(zoom)] = buttons else {
        return;
    };

    let button = close.frame();
    let bar_h = f64::from(bar_h);
    let top = (bar_h - button.size.height) / 2.0;
    if (button.origin.y - top).abs() < 0.5 && (button.origin.x - f64::from(LEFT)).abs() < 0.5 {
        return;
    }
    // The buttons stand in a title bar view inside a container pinned to the window's top.
    // SAFETY: both are AppKit's own views, retained for as long as the window is.
    let Some(container) = (unsafe { close.superview().and_then(|bar| bar.superview()) }) else {
        return;
    };
    let mut bounds = container.frame();
    bounds.size.height = bar_h;
    bounds.origin.y = window.frame().size.height - bar_h;
    container.setFrame(bounds);

    let spacing = minimise.frame().origin.x - button.origin.x;
    for (index, each) in [close, minimise, zoom].iter().enumerate() {
        let x = f64::from(LEFT) + spacing * index as f64;
        each.setFrameOrigin(NSPoint::new(x, top));
    }
}
