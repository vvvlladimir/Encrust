use crate::Mesh;
use crate::validate::duplicate_faces;

/// Drops the faces laid over others on the same three vertices without changing what the
/// surface encloses, and returns where they were in the mesh, ascending.
pub fn remove_duplicate_faces(mesh: &mut Mesh) -> Vec<u32> {
    let extra = duplicate_faces(mesh);
    if extra.is_empty() {
        return Vec::new();
    }
    let mut dropped = vec![false; mesh.faces.len()];
    for index in &extra {
        dropped[*index] = true;
    }
    let mut at = 0;
    mesh.faces.retain(|_| {
        let keep = !dropped[at];
        at += 1;
        keep
    });
    extra.into_iter().map(|index| index as u32).collect()
}
