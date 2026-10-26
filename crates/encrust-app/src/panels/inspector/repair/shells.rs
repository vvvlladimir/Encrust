//! The Model mode's Shells section: the one model's shells, picked, flown to and dropped.

use crate::i18n::tr;
use crate::panels::Window;
use crate::scene::ObjectId;
use crate::ui::{
    busy, companion_button, describe, icon, list, list_row, section, section_with_actions, theme,
    tone,
};

/// What a press in the list asked for, done once the list has let the scene go.
enum Asked {
    Toggle(usize),
    Drop(usize),
    Small,
    Invert,
}

/// The shells of the one model the tool works on, each a row to pick and a bin to drop it.
pub fn listed(ui: &mut egui::Ui, window: &mut Window) {
    let targets: Vec<ObjectId> = window.doc.scene.targets().map(|object| object.id).collect();
    let title = tr!("repair-shells-title");
    let [id] = targets[..] else {
        section(ui, &title, None, |ui| {
            tone(ui, &tr!("repair-shells-one"), theme::colors().text_low);
        });
        return;
    };
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let shells = &mut window.doc.repairs.shells;
    shells.list(object);
    let mut asked = None;
    let actions = [
        (icon::PICK_SMALL, tr!("repair-pick-small")),
        (icon::INVERT, tr!("repair-pick-invert")),
    ];
    let actions = actions
        .each_ref()
        .map(|(glyph, tip)| (*glyph, tip.as_str()));
    let (pressed, _) = section_with_actions(ui, &title, &actions, |ui| {
        describe(ui, &tr!("repair-shells-note"));
        let Some(rows) = shells.rows(object) else {
            busy(ui, &tr!("repair-listing"), None, false);
            return;
        };
        let picked = shells.picked(object);
        list(ui, |ui| {
            for (at, row) in rows.iter().enumerate() {
                let name = tr!("repair-shell", number = at + 1);
                let aside = match row.closed {
                    true => tr!("repair-shell-faces", count = row.faces.len()),
                    false => tr!("repair-shell-faces-open", count = row.faces.len()),
                };
                let bin = (icon::REMOVE, tr!("repair-drop-shell"));
                let pressed = list_row(
                    ui,
                    &name,
                    None,
                    picked.contains(&at),
                    Some((bin.0, &bin.1)),
                    None,
                    Some(&aside),
                    None,
                );
                if pressed.removed {
                    asked = Some(Asked::Drop(at));
                } else if pressed.picked {
                    asked = Some(Asked::Toggle(at));
                }
            }
        });
    });
    match pressed {
        Some(0) => asked = Some(Asked::Small),
        Some(_) => asked = Some(Asked::Invert),
        None => {}
    }
    let (repairs, status) = (&mut window.doc.repairs, &mut window.machine.status);
    match asked {
        Some(Asked::Toggle(at)) => repairs.shells.toggle(object, at),
        Some(Asked::Small) => repairs.shells.pick_small(object),
        Some(Asked::Invert) => repairs.shells.invert(object),
        Some(Asked::Drop(at)) => {
            repairs.shells.clear();
            repairs
                .hand
                .drop_shells(&window.doc.scene, id, vec![at], status);
        }
        None => {}
    }
}

/// The second action of the Model mode's foot: dropping the shells picked, once one is.
pub fn drop_picked(ui: &mut egui::Ui, window: &mut Window) {
    let [id] = window
        .doc
        .scene
        .targets()
        .map(|object| object.id)
        .collect::<Vec<_>>()[..]
    else {
        return;
    };
    let Some(object) = window.doc.scene.get(id) else {
        return;
    };
    let picked = window.doc.repairs.shells.picked(object).to_vec();
    if picked.is_empty() {
        return;
    }
    let label = tr!("repair-drop-shells", count = picked.len());
    if companion_button(ui, icon::REMOVE, &label, true).clicked() {
        let repairs = &mut window.doc.repairs;
        repairs.shells.clear();
        let status = &mut window.machine.status;
        repairs
            .hand
            .drop_shells(&window.doc.scene, id, picked, status);
    }
}
