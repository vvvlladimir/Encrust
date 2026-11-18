use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::i18n::tr;
use crate::panels::fixtures::Parts;
use crate::state::Doc;
use crate::workspace::Tool;

/// BUG: once nothing was written under Grow, a greyed-out Grow gave its reason nowhere: the
/// disabled tooltip never opens on a primary that greys itself out (ADR 0421).
#[test]
fn a_greyed_out_grow_says_why_on_hover() {
    let mut dressed = false;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 400.0))
        .build_ui_state(
            move |ui, parts: &mut Parts| {
                // The fonts are bound a frame after they are handed over.
                if dressed {
                    super::action(ui, &mut parts.window());
                } else {
                    crate::ui::theme::apply(ui.ctx());
                    ui.ctx()
                        .all_styles_mut(|style| style.interaction.tooltip_delay = 0.0);
                    dressed = true;
                }
            },
            Parts::of(Doc::default(), Tool::Supports),
        );
    harness.run_steps(3);
    harness.get_by_label(&tr!("supports-grow")).hover();
    harness.run_steps(3);
    let why = tr!("supports-nothing-visible");
    harness
        .query_by_label_contains(&why)
        .expect("an empty plate is why Grow cannot run, shown on hover");
}
