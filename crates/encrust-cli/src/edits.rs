//! The changes `--flip-faces`, `--drop-faces`, `--drop-shells` and `--bridge` make by
//! hand, each numbered on the model as import leaves it, which is how the window shows it.

use std::str::FromStr;

use anyhow::{Result, anyhow};
use core_document::{ApplyError, MendAsked, mended};
use core_edit::ObjectId;
use core_geometry::{Mesh, list_shells};

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
pub use core_document::Mended as Edited;

/// Turns, bridges and drops what `args` names on `mesh`: the turns and bridges first, the
/// drops last, so no number moves under one still to come. `None` where none was named.
pub fn apply(mesh: &mut Mesh, args: &ImportArgs) -> Result<Option<Edited>> {
    let asked = mend_asked(args);
    if asked.is_empty() {
        return Ok(None);
    }
    let (faces, shells) = (mesh.faces.len(), list_shells(mesh).len());
    mended(mesh, &asked, ObjectId::UNSET)
        .map(Some)
        .map_err(|error| refused(&error, (faces, shells), args))
}

/// The changes by hand `args` name.
pub fn mend_asked(args: &ImportArgs) -> MendAsked {
    MendAsked {
        flip_faces: args.flip_faces.clone(),
        drop_faces: args.drop_faces.clone(),
        drop_shells: args.drop_shells.clone(),
        bridge: args
            .bridge
            .iter()
            .map(|BridgeEdges(a, b)| [*a, *b])
            .collect(),
    }
}

/// Why a change by hand was refused, naming the flag the way the command line has it.
pub(crate) fn refused(
    error: &ApplyError,
    (faces, shells): (usize, usize),
    args: &ImportArgs,
) -> anyhow::Error {
    match error {
        ApplyError::NoSuchFace { number, .. } => {
            let flag = if args.flip_faces.contains(number) {
                "--flip-faces"
            } else {
                "--drop-faces"
            };
            anyhow!("{flag}: there is no face {number}; the model has {faces}, numbered from 0")
        }
        ApplyError::NoSuchShell { number, .. } => {
            anyhow!("--drop-shells: there is no shell {number}; the model has {shells}")
        }
        ApplyError::Failed { reason, .. } => anyhow!("--{reason}"),
        other => anyhow!("{other}"),
    }
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
