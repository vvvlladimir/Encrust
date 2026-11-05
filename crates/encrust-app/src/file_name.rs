//! What a sliced file is called: ADR 0387's template over a model, the machine, the resin
//! and the plate, with the chosen format's extension.

use core_engine::{NameFields, file_stem};

use crate::scene::Scene;
use crate::slicing::Slicing;

/// What a file sliced now is called: `template` over the selected model, or the first one,
/// the machine, the resin and the plate in hand, with the chosen format's extension.
pub fn file_name(scene: &Scene, slicing: &Slicing, template: &str) -> String {
    let name = scene
        .selected()
        .and_then(|id| scene.get(id))
        .or_else(|| scene.objects().first())
        .map_or("model", |object| object.name.as_str());
    let stem = stem(slicing, template, name, scene.active_plate());
    format!("{stem}.{}", slicing.format.extension())
}

/// The stem of `plate`'s file in a run over every plate: `template` over the plate's first
/// model and its own number.
pub fn plate_stem(scene: &Scene, slicing: &Slicing, template: &str, plate: u32) -> String {
    let name = scene
        .on_plate(plate)
        .next()
        .map_or("model", |object| object.name.as_str());
    stem(slicing, template, name, plate)
}

/// `template` filled for the model `name` on `plate`, with the machine and resin in hand.
fn stem(slicing: &Slicing, template: &str, name: &str, plate: u32) -> String {
    let (model, model_ext) = split_mesh_extension(name);
    let fields = NameFields {
        model,
        model_ext,
        printer: slicing.printer.as_ref().map_or("", |printer| &printer.name),
        resin: &slicing.material.name,
        layer_height_mm: slicing.layer_height_mm(),
        plate: plate + 1,
        when: jiff::Zoned::now().datetime(),
    };
    file_stem(template, &fields)
}

/// A model's name, which keeps the extension of the file it came from, and that extension
/// apart; a name ending in no mesh extension is all name.
fn split_mesh_extension(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && crate::files::MESHES
                    .iter()
                    .any(|mesh| mesh.eq_ignore_ascii_case(ext)) =>
        {
            (stem, ext)
        }
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::SlicedFormat;
    use crate::scene::ImportSummary;
    use core_engine::DEFAULT_TEMPLATE;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};

    fn scene_with_a_model(name: &str) -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            name.to_owned(),
            std::sync::Arc::new(mesh.clone()),
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
        scene
    }

    #[test]
    fn the_save_dialog_opens_on_the_selected_models_name() {
        let scene = scene_with_a_model("bracket.stl");
        let mut slicing = Slicing::default();
        slicing.format = SlicedFormat::Goo;
        assert_eq!(
            file_name(&scene, &slicing, DEFAULT_TEMPLATE),
            "bracket.goo",
            "the model's own extension is not part of its name"
        );
        assert_eq!(
            file_name(&scene, &slicing, "{model}_{ext}"),
            "bracket_stl.goo"
        );
        assert_eq!(
            file_name(&Scene::default(), &slicing, DEFAULT_TEMPLATE),
            "model.goo"
        );
        slicing.format = SlicedFormat::Ctb(core_pipeline::CtbVersion::V5);
        assert_eq!(
            file_name(&scene, &slicing, DEFAULT_TEMPLATE),
            "bracket.ctb",
            "the chosen format names the file the dialog opens on"
        );
    }

    #[test]
    fn the_name_follows_the_template_written_in_settings() {
        let scene = scene_with_a_model("bracket.v2");
        let mut slicing = Slicing::default();
        slicing.format = SlicedFormat::Goo;
        slicing.material.name = "Grey".to_owned();
        slicing.material.layer_height_mm = 0.05;
        assert_eq!(
            file_name(&scene, &slicing, "{model}_{resin}_{layer}um_p{plate}"),
            "bracket.v2_Grey_50um_p1.goo",
            "a dot that ends no mesh extension stays in the name"
        );
    }
}
