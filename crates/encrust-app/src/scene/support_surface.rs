use super::object::SceneObject;
use core_geometry::{Bvh, Mesh, Vec3};
use core_supports::{Placed, WallSide};
use printer_profiles::SupportProfile;
use std::ops::Range;
use std::sync::Arc;

/// A printed surface, its hierarchy, built once per surface, and the faces bounding the
/// cavity.
#[derive(Debug, Clone)]
pub(super) struct SupportShell {
    pub(super) mesh: Arc<Mesh>,
    pub(super) bvh: Arc<Bvh>,
    pub(super) inner: Range<usize>,
}

impl SceneObject {
    /// The surface supports stand against, as the model prints, drilled and hollowed, and its
    /// hierarchy; the model itself until that hierarchy is built (ADR 0334).
    pub fn support_surface(&self) -> (&Arc<Mesh>, &Arc<Bvh>) {
        match (&self.support_shell, self.wanted_surface()) {
            (Some(kept), Some((wanted, _))) if Arc::ptr_eq(&kept.mesh, wanted) => {
                (&kept.mesh, &kept.bvh)
            }
            _ => (&self.mesh, &self.bvh),
        }
    }

    /// The model as a support placement asks about it, standing where it stands, against
    /// [`Self::support_surface`].
    pub fn support_placed(&self) -> Placed<'_> {
        let (mesh, bvh) = self.support_surface();
        Placed::new(mesh, bvh, self.transform)
    }

    /// Rebuilds the supports to `table` where an edit asks, standing against
    /// [`Self::support_surface`] with the patches read off the model itself.
    pub fn refresh_supports(&mut self, table: &[SupportProfile]) {
        let model = Arc::clone(&self.mesh);
        let (surface, bvh) = self.support_surface();
        let (surface, bvh) = (Arc::clone(surface), Arc::clone(bvh));
        self.supports
            .refresh_on(&model, &surface, &bvh, self.transform, table);
    }

    /// Whether the supports have changed and what is printed of them has not been built
    /// again yet: their trees may still be growing on a worker (ADR 0340).
    pub fn supports_pending(&self) -> bool {
        self.supports.meshes().is_none()
            && !(self.supports.is_empty() && self.supports.drawn().is_empty())
    }

    /// Builds the hierarchy of the surface supports stand against when that surface
    /// changed, here and now, and answers whether it did; the window builds it on a worker
    /// through [`crate::surfaces::SupportSurfaces`].
    pub fn keep_support_surface(&mut self) -> bool {
        let forgot = self.forget_stale_support_surface();
        let Some(mesh) = self.unbuilt_support_surface().cloned() else {
            return forgot;
        };
        let bvh = Arc::new(Bvh::build(&mesh));
        self.keep_support_bvh(&mesh, bvh) || forgot
    }

    /// The printed surface supports are to stand against whose hierarchy is not built yet.
    pub fn unbuilt_support_surface(&self) -> Option<&Arc<Mesh>> {
        let (wanted, _) = self.wanted_surface()?;
        let kept = self.support_shell.as_ref();
        (!kept.is_some_and(|kept| Arc::ptr_eq(&kept.mesh, wanted))).then_some(wanted)
    }

    /// Keeps `bvh` as the hierarchy of `mesh` while that is still the printed surface, and
    /// answers whether it was kept.
    pub fn keep_support_bvh(&mut self, mesh: &Arc<Mesh>, bvh: Arc<Bvh>) -> bool {
        let Some((wanted, inner)) = self.wanted_surface() else {
            return false;
        };
        if !Arc::ptr_eq(wanted, mesh) || self.unbuilt_support_surface().is_none() {
            return false;
        }
        self.support_shell = Some(SupportShell {
            mesh: Arc::clone(mesh),
            bvh,
            inner,
        });
        true
    }

    /// Lets go of the surface kept for supports once the model prints as itself again, and
    /// answers whether it did.
    pub fn forget_stale_support_surface(&mut self) -> bool {
        let stale = self.support_shell.is_some() && self.wanted_surface().is_none();
        if stale {
            self.support_shell = None;
        }
        stale
    }

    /// The printed surface when it is not the model itself, and which of its faces lie
    /// inside: the drilled mesh, else the shell, its lattice counted with the cavity.
    pub(super) fn wanted_surface(&self) -> Option<(&Arc<Mesh>, Range<usize>)> {
        let (mesh, cavity) = match self.hollow.drilled(self.uncut()) {
            Some(drilled) => (&drilled.mesh, drilled.cavity.clone()),
            None => (self.hollow.shell()?, self.hollow.cavity_faces()),
        };
        let inner = cavity.map_or(0..0, |cavity| {
            cavity.start..mesh.faces.len().max(cavity.end)
        });
        Some((mesh, inner))
    }

    /// Whether face `face` of [`Self::support_surface`] lies inside the model: on its
    /// cavity, or on the lattice standing in it.
    pub fn is_inner_face(&self, face: usize) -> bool {
        self.surface_kept()
            .is_some_and(|kept| kept.inner.contains(&face))
    }

    /// The side of the wall a run holding up the cavity, or everything outside it, keeps
    /// to, or `None` for a model with no cavity supports stand against.
    pub fn wall_side(&self, inside: bool) -> Option<WallSide> {
        let kept = self.surface_kept().filter(|kept| !kept.inner.is_empty())?;
        Some(WallSide {
            bvh: Arc::clone(&kept.bvh),
            inner: kept.inner.clone(),
            into_model: self.transform.to_matrix().inverse(),
            inside,
        })
    }

    /// Whether a support touching the model at `contact`, in its own space, holds it up
    /// from inside its cavity: the surface nearest it bounds the cavity.
    pub fn holds_inside(&self, contact: Vec3) -> bool {
        self.support_surface()
            .1
            .closest(contact)
            .is_some_and(|closest| self.is_inner_face(closest.face))
    }

    /// Whether the model has a cavity supports stand against, so a run can hold it up.
    pub fn supported_inside(&self) -> bool {
        self.surface_kept()
            .is_some_and(|kept| !kept.inner.is_empty())
    }

    /// The surface kept for supports, while it is still the one the model prints as.
    pub(super) fn surface_kept(&self) -> Option<&SupportShell> {
        let kept = self.support_shell.as_ref()?;
        Arc::ptr_eq(&kept.mesh, self.support_surface().0).then_some(kept)
    }
}
