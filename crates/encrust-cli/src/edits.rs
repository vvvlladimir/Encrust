//! The changes `--flip-faces`, `--drop-faces`, `--drop-shells` and `--bridge` make by
//! hand, each numbered on the model as import leaves it, which is how the window shows it.

use std::str::FromStr;

use anyhow::{Context as _, Result, bail};
use core_geometry::{Mesh, Region, bridge, flip_faces, list_shells, remove_faces};

use crate::args::ImportArgs;

/// Two open edges to bridge, each as its two vertices: `A-B:C-D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeEdges(pub [u32; 2], pub [u32; 2]);

impl FromStr for BridgeEdges {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let edge = |part: &str| -> Option<[u32; 2]> {
            let (a, b) = part.split_once('-')?;
            Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
        };
        let (first, second) = text
            .split_once(':')
            .ok_or_else(|| format!("`{text}` is not two edges as A-B:C-D"))?;
        match (edge(first), edge(second)) {
            (Some(a), Some(b)) => Ok(Self(a, b)),
            _ => Err(format!(
                "`{text}` is not two edges of vertex numbers as A-B:C-D"
            )),
        }
    }
}

/// What the changes by hand did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edited {
    pub faces_flipped: usize,
    pub faces_added: usize,
    pub faces_dropped: usize,
    pub shells_dropped: usize,
}

/// Turns, bridges and drops what `args` names on `mesh`: the turns and bridges first, the
/// drops last, so no number moves under one still to come. `None` where none was named.
pub fn apply(mesh: &mut Mesh, args: &ImportArgs) -> Result<Option<Edited>> {
    if args.flip_faces.is_empty()
        && args.drop_faces.is_empty()
        && args.drop_shells.is_empty()
        && args.bridge.is_empty()
    {
        return Ok(None);
    }
    let faces = mesh.faces.len();
    let flipped = region(&args.flip_faces, faces).context("--flip-faces")?;
    let mut dropped = region(&args.drop_faces, faces).context("--drop-faces")?;
    let shells = list_shells(mesh);
    let mut numbers = args.drop_shells.clone();
    numbers.sort_unstable();
    numbers.dedup();
    for &number in &numbers {
        let Some(shell) = number.checked_sub(1).and_then(|at| shells.get(at)) else {
            bail!(
                "--drop-shells: there is no shell {number}; the model has {}",
                shells.len()
            );
        };
        for face in &shell.faces {
            dropped.set(*face as usize, true);
        }
    }
    let mut edited = Edited {
        faces_flipped: flip_faces(mesh, &flipped),
        shells_dropped: numbers.len(),
        ..Edited::default()
    };
    for BridgeEdges(a, b) in &args.bridge {
        edited.faces_added += bridge(mesh, *a, *b)
            .with_context(|| format!("--bridge {}-{}:{}-{}", a[0], a[1], b[0], b[1]))?;
    }
    edited.faces_dropped = remove_faces(mesh, &dropped);
    Ok(Some(edited))
}

/// The faces `numbers` names, refusing one past the last of `faces`.
fn region(numbers: &[usize], faces: usize) -> Result<Region> {
    if let Some(face) = numbers.iter().find(|&&face| face >= faces) {
        bail!("there is no face {face}; the model has {faces}, numbered from 0");
    }
    Ok(numbers.iter().copied().collect())
}

/// What the changes by hand did, in the words the report says it in.
pub fn describe(edited: &Edited) -> String {
    let said = |count: usize, verb: &str, noun: &str| {
        let plural = if count == 1 { "" } else { "s" };
        (count > 0).then(|| format!("{verb} {count} {noun}{plural}"))
    };
    [
        said(edited.faces_flipped, "turned", "face"),
        said(edited.faces_added, "added", "face"),
        said(edited.faces_dropped, "dropped", "face"),
        said(edited.shells_dropped, "dropped", "shell"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_edges_read_as_a_dash_between_vertices_and_a_colon_between_edges() {
        assert_eq!("4-5:9-8".parse(), Ok(BridgeEdges([4, 5], [9, 8])));
        assert!("4-5".parse::<BridgeEdges>().is_err());
        assert!("4-x:9-8".parse::<BridgeEdges>().is_err());
    }
}
