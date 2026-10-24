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

fn tetrahedron() -> Imported {
    let mesh = Mesh::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
    );
    Imported::new(
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
    )
}

fn with_a_model() -> (Parts, ObjectId) {
    let doc = Doc::default();
    let mut parts = Parts {
        mode: Mode::default(),
        tool: Tool::Hollow,
        view: View::framing(&doc.plate),
        doc,
        tools: Tools::default(),
        machine: Machine::default(),
    };
    let id = parts.doc.scene.insert(tetrahedron());
    (parts, id)
}

#[test]
fn an_edit_to_the_support_profile_stays_on_the_picked_model_until_it_is_saved() {
    let (mut parts, first) = with_a_model();
    let second = parts.doc.scene.insert(tetrahedron());
    let plate_mm = parts.tools.supports.profile.middle.diameter_mm;
    parts.doc.scene.select(Some(first));

    editing_supports(&mut parts.window(), |window| {
        window.tools.supports.profile.middle.diameter_mm = plate_mm + 1.0;
    });
    let width_of = |parts: &Parts, id: ObjectId| {
        let object = parts.doc.scene.get(id).expect("there");
        parts.tools.supports.table_for(object)[0].middle.diameter_mm
    };
    assert_eq!(
        width_of(&parts, first),
        plate_mm + 1.0,
        "the picked model took it"
    );
    assert_eq!(width_of(&parts, second), plate_mm, "the other one did not");
    assert_eq!(parts.tools.supports.profile.middle.diameter_mm, plate_mm);

    // Saved over the profile it was taken from: now it is everybody's.
    editing_supports(&mut parts.window(), |window| {
        window.tools.supports.saved = Some(window.tools.supports.profile.clone());
    });
    assert_eq!(
        width_of(&parts, second),
        plate_mm + 1.0,
        "the other one follows"
    );
    assert!(
        parts
            .doc
            .scene
            .get(first)
            .expect("there")
            .own
            .supports
            .is_empty(),
        "numbers that are the plate's again are no longer the model's own"
    );
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
fn a_first_change_to_a_picked_model_becomes_its_own() {
    let (mut parts, id) = with_a_model();
    let plate_mm = parts.tools.hollow.state.thickness_mm;
    editing(&mut parts.window(), Some(id), &hollow(), |window| {
        window.tools.hollow.state.thickness_mm = 6.0;
    });
    let own = &parts.doc.scene.get(id).expect("there").own.hollow;
    assert_eq!(own.as_ref().map(|own| own.thickness_mm), Some(6.0));
    assert_eq!(
        parts.tools.hollow.state.thickness_mm, plate_mm,
        "the plate's stay what the next model starts from"
    );
}

#[test]
fn a_picked_model_left_unchanged_keeps_following_the_plate() {
    let (mut parts, id) = with_a_model();
    editing(&mut parts.window(), Some(id), &hollow(), |_| {});
    assert_eq!(parts.doc.scene.get(id).expect("there").own.hollow, None);
}

#[test]
fn without_a_model_the_plates_numbers_are_edited() {
    let (mut parts, _) = with_a_model();
    editing(&mut parts.window(), None, &hollow(), |window| {
        window.tools.hollow.state.thickness_mm = 6.0;
    });
    assert_eq!(parts.tools.hollow.state.thickness_mm, 6.0);
}
