use core_edit::ObjectId;
use core_engine::project::{ModelMeshes, ObjectState, Project, assign_ids};

/// The plate as `.encrust` holds it, every object with an id of its own.
#[derive(Debug)]
pub struct Document {
    project: Project,
}

impl Document {
    /// The plate `project` holds, an id given to each object that has none or a taken one.
    pub fn from_project(mut project: Project) -> Self {
        assign_ids(&mut project.manifest.objects);
        Self { project }
    }

    /// The plate as a project file holds it, ids and all.
    pub fn into_project(self) -> Project {
        self.project
    }

    /// The object `id` names, with its meshes, if it is on the plate.
    pub fn object(&self, id: ObjectId) -> Option<(&ObjectState, &ModelMeshes)> {
        let index = self.index_of(id)?;
        Some((
            &self.project.manifest.objects[index],
            &self.project.models[index],
        ))
    }

    fn index_of(&self, id: ObjectId) -> Option<usize> {
        self.project
            .manifest
            .objects
            .iter()
            .position(|object| object.id == id)
    }
}

#[cfg(test)]
mod tests {
    use core_engine::project::load;

    use super::*;

    /// A project of version 4, before ids: one 10 mm cube named "cube".
    const CUBE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../encrust-cli/tests/fixtures/cube.encrust"
    );

    #[test]
    fn a_plate_read_from_before_ids_finds_its_object_by_the_id_it_was_given() {
        let project = load(std::path::Path::new(CUBE)).expect("the cube fixture reads");
        let document = Document::from_project(project);

        let (object, _) = document
            .object(ObjectId(1))
            .expect("the one object is given the first id");
        assert_eq!(object.name, "cube");
        assert!(document.object(ObjectId(2)).is_none());
    }
}
