use crate::panels::Window;
use crate::scene::{ObjectId, SceneObject};
use crate::state::Tools;
use crate::ui::{Segment, Segmented, field_label, hint, theme};

/// Where a tool keeps its numbers: the plate's in the tool, a model's own on the model.
pub struct Scoped<T> {
    pub plate: fn(&mut Tools) -> &mut T,
    pub own: fn(&mut SceneObject) -> &mut Option<T>,
    pub read: fn(&SceneObject) -> &Option<T>,
}

/// Whether the numbers below are the plate's or the one picked model's own, and the switch
/// between them. Answers the model whose own numbers the panel is editing. See ADR 0227.
pub fn switch<T: Clone>(
    ui: &mut egui::Ui,
    window: &mut Window,
    scoped: &Scoped<T>,
) -> Option<ObjectId> {
    let picked = match window.doc.scene.selection() {
        [id] => Some(*id),
        _ => None,
    };
    let own = picked
        .and_then(|id| window.doc.scene.get(id))
        .is_some_and(|object| (scoped.read)(object).is_some());
    let mut model = own;
    ui.horizontal(|ui| {
        field_label(ui, "Applies to", theme::colors().text_mid, "");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let choices = [Segment::new(false, "Plate"), Segment::new(true, "Model")];
            ui.add_enabled_ui(picked.is_some(), |ui| {
                Segmented::new(&choices)
                    .width(theme::FIELD_W)
                    .show(ui, &mut model);
            });
        });
    });
    if model != own
        && let Some(object) = picked.and_then(|id| window.doc.scene.get_mut(id))
    {
        *(scoped.own)(object) = model.then(|| (scoped.plate)(window.tools).clone());
    }

    let keeping = window
        .doc
        .scene
        .here()
        .filter(|object| (scoped.read)(object).is_some())
        .count();
    match (picked, model, keeping) {
        (None, _, 0) => hint(ui, "Pick one model to give it numbers of its own."),
        (_, true, _) => hint(ui, "These numbers are this model's alone."),
        (_, false, 0) => {}
        (_, false, 1) => hint(ui, "1 model keeps numbers of its own."),
        (_, false, n) => hint(ui, &format!("{n} models keep numbers of their own.")),
    }
    model.then_some(picked).flatten()
}

/// Runs `draw` with the tool holding `id`'s own numbers in place of the plate's, and puts
/// what it changed back on the model. Without a model, `draw` edits the plate's. The model
/// keeps its numbers while the panel draws, so what reads them this frame still sees them.
pub fn editing<T: Clone>(
    window: &mut Window,
    id: Option<ObjectId>,
    scoped: &Scoped<T>,
    draw: impl FnOnce(&mut Window),
) {
    let own = id
        .and_then(|id| window.doc.scene.get(id))
        .and_then(|object| (scoped.read)(object).clone());
    let Some(own) = own else {
        draw(window);
        return;
    };
    let plate = std::mem::replace((scoped.plate)(window.tools), own);
    draw(window);
    let edited = std::mem::replace((scoped.plate)(window.tools), plate);
    if let Some(object) = id.and_then(|id| window.doc.scene.get_mut(id)) {
        *(scoped.own)(object) = Some(edited);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};

    use super::*;
    use crate::scene::{ImportSummary, Imported};
    use crate::state::{Doc, Machine, View};
    use crate::workspace::{Mode, Tool};

    /// What a window is drawn over, owned by the test.
    struct Parts {
        mode: Mode,
        tool: Tool,
        doc: Doc,
        view: View,
        tools: Tools,
        machine: Machine,
    }

    impl Parts {
        fn window(&mut self) -> Window<'_> {
            Window {
                mode: &mut self.mode,
                tool: &mut self.tool,
                doc: &mut self.doc,
                view: &mut self.view,
                tools: &mut self.tools,
                machine: &mut self.machine,
            }
        }
    }

    fn hollow() -> Scoped<core_engine::project::HollowState> {
        Scoped {
            plate: |tools| &mut tools.hollow.state,
            own: |object| &mut object.own.hollow,
            read: |object| &object.own.hollow,
        }
    }

    fn with_a_model() -> (Parts, ObjectId) {
        let mesh = Mesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let doc = Doc::default();
        let mut parts = Parts {
            mode: Mode::default(),
            tool: Tool::Hollow,
            view: View::framing(&doc.plate),
            doc,
            tools: Tools::default(),
            machine: Machine::default(),
        };
        let id = parts.doc.scene.insert(Imported::new(
            "tetrahedron".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        (parts, id)
    }

    #[test]
    fn a_models_own_numbers_are_edited_without_touching_the_plates() {
        let (mut parts, id) = with_a_model();
        let plate_mm = parts.tools.hollow.state.thickness_mm;
        let mut own = parts.tools.hollow.state.clone();
        own.thickness_mm = 4.0;
        parts.doc.scene.get_mut(id).expect("inserted").own.hollow = Some(own);

        editing(&mut parts.window(), Some(id), &hollow(), |window| {
            assert_eq!(
                window.tools.hollow.state.thickness_mm, 4.0,
                "the panel shows the model's"
            );
            window.tools.hollow.state.thickness_mm = 5.0;
        });
        let object = parts.doc.scene.get(id).expect("still there");
        assert_eq!(
            object.own.hollow.as_ref().map(|own| own.thickness_mm),
            Some(5.0)
        );
        assert_eq!(
            parts.tools.hollow.state.thickness_mm, plate_mm,
            "the plate's are untouched"
        );
    }

    #[test]
    fn without_numbers_of_its_own_a_model_edits_the_plates() {
        let (mut parts, id) = with_a_model();
        editing(&mut parts.window(), Some(id), &hollow(), |window| {
            window.tools.hollow.state.thickness_mm = 6.0;
        });
        assert_eq!(parts.tools.hollow.state.thickness_mm, 6.0);
        assert_eq!(parts.doc.scene.get(id).expect("there").own.hollow, None);
    }
}
