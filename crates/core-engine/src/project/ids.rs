//! Which object is which on a plate, kept across saves so an edit can name its object
//! (ADR 0466).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::ObjectState;

/// Identifies one object on a plate. Never reused within a plate, so a stale id resolves
/// to nothing instead of to the wrong object.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ObjectId(pub u64);

impl ObjectId {
    /// Not given yet: an object of a file written before ids, until it is read.
    pub const UNSET: Self = Self(0);
}

/// Gives each object that has no id, or one an earlier object took, an id above every one
/// present.
pub fn assign(objects: &mut [ObjectState]) {
    let mut next = objects.iter().map(|object| object.id.0).max().unwrap_or(0) + 1;
    let mut seen = HashSet::with_capacity(objects.len());
    for object in objects {
        if object.id == ObjectId::UNSET || !seen.insert(object.id) {
            object.id = ObjectId(next);
            seen.insert(object.id);
            next += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::read_from;
    use super::super::tests::{archive, object, project};
    use super::*;

    fn with_ids(ids: &[u64]) -> Vec<ObjectState> {
        ids.iter()
            .enumerate()
            .map(|(index, &id)| ObjectState {
                id: ObjectId(id),
                ..object(index)
            })
            .collect()
    }

    fn ids_of(objects: &[ObjectState]) -> Vec<u64> {
        objects.iter().map(|object| object.id.0).collect()
    }

    #[test]
    fn objects_from_before_ids_are_numbered_from_one() {
        let mut objects = with_ids(&[0, 0, 0]);
        assign(&mut objects);
        assert_eq!(ids_of(&objects), [1, 2, 3]);
    }

    #[test]
    fn ids_present_are_kept_and_new_ones_go_above_them() {
        let mut objects = with_ids(&[7, 0, 3]);
        assign(&mut objects);
        assert_eq!(ids_of(&objects), [7, 8, 3]);
    }

    #[test]
    fn a_repeated_id_is_given_a_new_one_after_its_first() {
        let mut objects = with_ids(&[4, 4, 2]);
        assign(&mut objects);
        assert_eq!(ids_of(&objects), [4, 5, 2]);
    }

    #[test]
    fn a_file_written_before_ids_reads_with_them() {
        let saved = project(1);
        let mut older = serde_json::to_value(&saved.manifest).expect("the manifest serialises");
        older["objects"][0]
            .as_object_mut()
            .expect("an object is a JSON object")
            .remove("id");

        let back = read_from(archive(&older, &saved)).expect("a file without ids reads");
        assert_eq!(back.manifest.objects[0].id, ObjectId(1));
    }
}
