//! How a chord reads: as keycaps, as one run of text, and as the line a file keeps.

use egui::{Key, KeyboardShortcut, Modifiers};

/// What one chord reads as, all its keys run together as a tooltip shows them.
pub fn chord_text(chord: &KeyboardShortcut) -> String {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    chord_keys(chord).join(separator)
}

/// The face of each key a chord holds down, modifiers first, one keycap each.
pub fn chord_keys(chord: &KeyboardShortcut) -> Vec<&'static str> {
    let mut keys = modifier_keys(chord.modifiers);
    keys.push(key_text(chord.logical_key));
    keys
}

/// The face of each modifier held, in the order the platform writes them.
///
/// The Mac's key symbols are Phosphor glyphs: the text fonts have no ⇧, ⌥ or ⏎, and a
/// missing glyph draws as an empty box.
pub fn modifier_keys(modifiers: Modifiers) -> Vec<&'static str> {
    let held = if cfg!(target_os = "macos") {
        // Control, Option, Shift, Command: the order the platform writes them in.
        vec![
            (modifiers.ctrl, crate::ui::icon::KEY_CONTROL),
            (modifiers.alt, crate::ui::icon::KEY_OPTION),
            (modifiers.shift, crate::ui::icon::KEY_SHIFT),
            (
                modifiers.command || modifiers.mac_cmd,
                crate::ui::icon::KEY_COMMAND,
            ),
        ]
    } else {
        vec![
            (modifiers.ctrl || modifiers.command, "Ctrl"),
            (modifiers.alt, "Alt"),
            (modifiers.shift, "Shift"),
        ]
    };
    held.into_iter()
        .filter_map(|(held, face)| held.then_some(face))
        .collect()
}

/// The face of a keycap. egui spells its key names out — "Questionmark", "Backslash" —
/// which is not what a keycap says, and draws arrows the text fonts lack.
fn key_text(key: Key) -> &'static str {
    let mac = cfg!(target_os = "macos");
    match key {
        Key::Escape => "Esc",
        Key::Backspace if mac => crate::ui::icon::KEY_BACKSPACE,
        Key::Backspace => "Backspace",
        Key::Delete => "Del",
        Key::Enter if mac => crate::ui::icon::KEY_RETURN,
        Key::Enter => "Enter",
        Key::Space => "Space",
        Key::ArrowUp => "\u{2191}",
        Key::ArrowDown => "\u{2193}",
        Key::ArrowLeft => "\u{2190}",
        Key::ArrowRight => "\u{2192}",
        _ => key.symbol_or_name(),
    }
}

/// The line a chord is kept as in the preferences: the modifiers in a fixed order, then the
/// key's English name, `Cmd+Shift+Z`. Cmd is Ctrl off the Mac, so a file keeps its keys
/// when moved between machines.
pub fn spec(chord: &KeyboardShortcut) -> String {
    let modifiers = chord.modifiers;
    let ctrl_alone = cfg!(target_os = "macos") || !modifiers.command;
    let held = [
        (modifiers.ctrl && ctrl_alone, "Ctrl"),
        (modifiers.alt, "Alt"),
        (modifiers.shift, "Shift"),
        (modifiers.command || modifiers.mac_cmd, "Cmd"),
    ];
    held.into_iter()
        .filter_map(|(held, name)| held.then_some(name))
        .chain([chord.logical_key.name()])
        .collect::<Vec<_>>()
        .join("+")
}

/// A chord read back from [`spec`]'s line, or `None` for one this build cannot read.
pub fn parse(line: &str) -> Option<KeyboardShortcut> {
    let (held, key) = line.rsplit_once('+').unwrap_or(("", line));
    let mut modifiers = Modifiers::NONE;
    for name in held.split('+').filter(|name| !name.is_empty()) {
        let modifier = match name {
            "Ctrl" => Modifiers::CTRL,
            "Alt" => Modifiers::ALT,
            "Shift" => Modifiers::SHIFT,
            "Cmd" => Modifiers::COMMAND,
            _ => return None,
        };
        modifiers = modifiers.plus(modifier);
    }
    Some(KeyboardShortcut::new(modifiers, Key::from_name(key)?))
}
