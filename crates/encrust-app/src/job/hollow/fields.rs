//! Cavity fields kept between hollowing runs, so that moving a blocker or digging a channel
//! cuts the cavity again without building its field; see `docs/design/hollowing.md`.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use core_geometry::{Mat3, Mesh};
use core_volume::{CavityField, HollowSettings};

/// What the kept fields may hold together, in bytes. A field over it is not kept, and the
/// longest untouched go first to make room.
#[cfg(not(target_arch = "wasm32"))]
pub const FIELD_BUDGET_BYTES: usize = 512 << 20;
/// A browser never takes back memory a page has grown into, so the web build keeps less.
#[cfg(target_arch = "wasm32")]
pub const FIELD_BUDGET_BYTES: usize = 128 << 20;

/// Whether the kept fields make way for a field being built or a plate being sliced. In a
/// browser they would otherwise stand on top of that peak for good.
const YIELDS: bool = cfg!(target_arch = "wasm32");

/// The fields of the models hollowed lately, most recently used last.
#[derive(Debug)]
pub struct Fields {
    budget_bytes: usize,
    kept: Vec<Kept>,
}

/// One field and what it was built from: the mesh, held weakly so the cache never keeps a
/// model alive, the shape it stood at, and its settings with the blockers and infill taken
/// out, since the field holds neither.
#[derive(Debug)]
struct Kept {
    mesh: Weak<Mesh>,
    shape: Mat3,
    settings: HollowSettings,
    field: Arc<CavityField>,
    bytes: usize,
}

/// The fields one window keeps, shared with the worker that builds them.
pub type SharedFields = Arc<Mutex<Fields>>;

impl Default for Fields {
    fn default() -> Self {
        Self::new(FIELD_BUDGET_BYTES)
    }
}

impl Fields {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            kept: Vec::new(),
        }
    }

    /// The field `mesh` was built into at `shape` and `settings`, if it is still kept.
    pub fn get(
        &mut self,
        mesh: &Arc<Mesh>,
        shape: Mat3,
        settings: &HollowSettings,
    ) -> Option<Arc<CavityField>> {
        let settings = field_part(settings);
        let at = self.kept.iter().position(|kept| {
            kept.shape == shape
                && kept.settings == settings
                && kept.mesh.strong_count() > 0
                && Weak::ptr_eq(&kept.mesh, &Arc::downgrade(mesh))
        })?;
        let kept = self.kept.remove(at);
        let field = Arc::clone(&kept.field);
        self.kept.push(kept);
        Some(field)
    }

    /// Keeps `field` if it fits the budget, letting the longest untouched go to make room.
    pub fn keep(
        &mut self,
        mesh: &Arc<Mesh>,
        shape: Mat3,
        settings: &HollowSettings,
        field: Arc<CavityField>,
    ) {
        self.kept.retain(|kept| kept.mesh.strong_count() > 0);
        let bytes = field.bytes();
        if bytes > self.budget_bytes {
            return;
        }
        while self.bytes() + bytes > self.budget_bytes {
            self.kept.remove(0);
        }
        self.kept.push(Kept {
            mesh: Arc::downgrade(mesh),
            shape,
            settings: field_part(settings),
            field,
            bytes,
        });
    }

    /// Lets every field go where the kept ones must make way for heavier work.
    pub fn make_way(&mut self) {
        if YIELDS {
            self.kept.clear();
        }
    }

    /// What the kept fields hold together, in bytes.
    pub fn bytes(&self) -> usize {
        self.kept.iter().map(|kept| kept.bytes).sum()
    }
}

/// The fields behind `shared`, whatever a worker that panicked holding them left: a field
/// is either kept whole or not at all.
pub fn lock(shared: &SharedFields) -> MutexGuard<'_, Fields> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What of `settings` a field is built from.
fn field_part(settings: &HollowSettings) -> HollowSettings {
    HollowSettings {
        blockers: Vec::new(),
        infill: None,
        ..settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Bvh, Vec3};
    use core_volume::{Blocker, Cancel, cavity_field};

    fn tetrahedron() -> Arc<Mesh> {
        Arc::new(Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        ))
    }

    fn field_of(mesh: &Mesh, settings: &HollowSettings) -> Arc<CavityField> {
        Arc::new(
            cavity_field(
                mesh,
                &Bvh::build(mesh),
                settings,
                Mat3::IDENTITY,
                Cancel::never(),
            )
            .expect("a 20 mm tetrahedron has a cavity field"),
        )
    }

    #[test]
    fn a_kept_field_serves_new_blockers_but_not_a_new_wall() {
        let mesh = tetrahedron();
        let asked = HollowSettings::for_wall(1.5);
        let mut fields = Fields::default();
        fields.keep(&mesh, Mat3::IDENTITY, &asked, field_of(&mesh, &asked));

        let blocked = HollowSettings {
            blockers: vec![Blocker::ball(Vec3::splat(5.0), 2.0)],
            ..asked.clone()
        };
        assert!(
            fields.get(&mesh, Mat3::IDENTITY, &blocked).is_some(),
            "a blocker is cut out of the field, so it does not need another"
        );
        assert!(
            fields
                .get(&mesh, Mat3::IDENTITY, &HollowSettings::for_wall(2.0))
                .is_none(),
            "a wall of its own is a field of its own"
        );
        assert!(
            fields
                .get(&mesh, Mat3::from_diagonal(Vec3::splat(2.0)), &asked)
                .is_none(),
            "a wall is measured on the plate, so a model resized needs another field"
        );
        assert!(
            fields.get(&tetrahedron(), Mat3::IDENTITY, &asked).is_none(),
            "an equal mesh is not the same model"
        );
    }

    #[test]
    fn a_field_over_the_budget_is_not_kept_and_the_oldest_makes_room() {
        let mesh = tetrahedron();
        let thin = HollowSettings::for_wall(1.5);
        let thick = HollowSettings::for_wall(2.0);
        let one = field_of(&mesh, &thin);
        let other = field_of(&mesh, &thick);

        let mut tight = Fields::new(one.bytes() - 1);
        tight.keep(&mesh, Mat3::IDENTITY, &thin, Arc::clone(&one));
        assert_eq!(
            tight.bytes(),
            0,
            "a field larger than the whole budget is left to go"
        );

        let mut room_for_one = Fields::new(one.bytes() + other.bytes() - 1);
        room_for_one.keep(&mesh, Mat3::IDENTITY, &thin, one);
        room_for_one.keep(&mesh, Mat3::IDENTITY, &thick, other);
        assert!(
            room_for_one.get(&mesh, Mat3::IDENTITY, &thin).is_none()
                && room_for_one.get(&mesh, Mat3::IDENTITY, &thick).is_some(),
            "the field untouched longest goes to make room for the new one"
        );
    }

    #[test]
    fn a_model_let_go_takes_its_field_with_it() {
        let mesh = tetrahedron();
        let asked = HollowSettings::for_wall(1.5);
        let mut fields = Fields::default();
        fields.keep(&mesh, Mat3::IDENTITY, &asked, field_of(&mesh, &asked));
        drop(mesh);

        let other = tetrahedron();
        fields.keep(&other, Mat3::IDENTITY, &asked, field_of(&other, &asked));
        assert_eq!(
            fields.kept.len(),
            1,
            "a field is held for a model, never in its place"
        );
    }
}
