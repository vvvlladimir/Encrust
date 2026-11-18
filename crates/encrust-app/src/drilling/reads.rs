use std::sync::{Arc, Weak};

use core_geometry::Mesh;
use core_volume::ModelRead;

/// Each mesh's read into the engine, shared by the run that drills it and the one that cuts its
/// plugs, and gone with the last of them: the whole read is too big to keep (ADR 0412).
#[derive(Debug, Default)]
pub struct ModelReads {
    held: Vec<Weak<ModelRead>>,
}

impl ModelReads {
    /// The read of `mesh`, the one a run holds already where there is one.
    pub fn of(&mut self, mesh: &Arc<Mesh>) -> Arc<ModelRead> {
        self.held.retain(|held| held.strong_count() > 0);
        let same = |read: &Arc<ModelRead>| Arc::ptr_eq(read.model(), mesh);
        if let Some(read) = self.held.iter().filter_map(Weak::upgrade).find(same) {
            return read;
        }
        let read = Arc::new(ModelRead::new(Arc::clone(mesh)));
        self.held.push(Arc::downgrade(&read));
        read
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use super::*;

    #[test]
    fn a_read_is_shared_while_a_run_holds_it_and_gone_after() {
        let mesh = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        let mut reads = ModelReads::default();
        let held = reads.of(&mesh);
        assert!(
            Arc::ptr_eq(&held, &reads.of(&mesh)),
            "one read for both runs"
        );
        let other = Arc::new(cuboid(Vec3::ZERO, Vec3::ONE));
        assert!(
            !Arc::ptr_eq(&held, &reads.of(&other)),
            "a new mesh is read anew"
        );

        let gone = Arc::downgrade(&held);
        drop(held);
        assert!(
            gone.upgrade().is_none(),
            "nothing keeps it once the runs end"
        );
    }
}
