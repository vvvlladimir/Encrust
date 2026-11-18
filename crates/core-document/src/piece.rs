use core_geometry::{
    MendMethod, MendSettings, Mesh, MeshDiagnostics, Quat, Transform, Vec3, center_of_mass,
    diagnose, drop_to_plate, mend, transform_mesh,
};

/// A mesh an edit made, in plate coordinates, to stand on the plate as a new object.
#[derive(Debug, Clone)]
pub struct Piece {
    pub role: Role,
    pub mesh: Mesh,
    /// Where it stands; the mesh is already where the edit left it, so this only turns it.
    pub transform: Transform,
    pub diagnostics: MeshDiagnostics,
    /// It did not come out sound and was mended.
    pub mended: bool,
}

/// What a new object is to the one it was made from, which a front end names it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The one half a cut kept, which keeps the model's name.
    Kept,
    Lower,
    Upper,
    /// A loose dowel, counted from one.
    Dowel(usize),
    /// A copy of the whole object.
    Copy,
    /// One body a split or a part-off made, counted from one.
    Part(usize),
}

impl Piece {
    /// `mesh` mended where it did not come out sound: the surface is the edit's own, so
    /// nothing is asked first (ADR 0194).
    pub fn sound(role: Role, mut mesh: Mesh) -> Self {
        let mut diagnostics = diagnose(&mesh);
        let mended = !diagnostics.is_sound();
        if mended {
            mend(&mut mesh, MendMethod::Repair, MendSettings::default());
            diagnostics = diagnose(&mesh);
        }
        Self {
            role,
            mesh,
            transform: Transform::default(),
            diagnostics,
            mended,
        }
    }

    /// Turned about its centre of mass until `normal` looks straight down, and stood on the
    /// plate. A zero normal leaves it as it is.
    #[must_use]
    pub fn laid_on(mut self, normal: Vec3) -> Self {
        let (Some(normal), Some(centre)) = (normal.try_normalize(), center_of_mass(&self.mesh))
        else {
            return self;
        };
        let rotation = Quat::from_rotation_arc(normal, Vec3::NEG_Z);
        let mut transform = Transform {
            translation: centre - rotation * centre,
            rotation,
            ..Transform::default()
        };
        if let Some(bounds) = transform_mesh(&self.mesh, transform).aabb() {
            transform.translation += drop_to_plate(&bounds);
        }
        self.transform = transform;
        self
    }
}
