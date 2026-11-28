//! Supports from the command line, the window's sequence without hand placement: find contacts,
//! columns, trunks, mesh, merge, and Merge into one by points. See `docs/design/supports.md`.

use std::fmt;

use anyhow::Result;
use core_geometry::{Scalar, Vec3};
use core_supports::Unstood;

use crate::hollowing::point;

/// The points one `--merge-supports` names supports by, plate mm: two or more.
#[derive(Debug, Clone, PartialEq)]
pub struct MergePoints(pub Vec<Vec3>);

/// `X,Y,Z;X,Y,Z…` as the points of one merge.
pub fn merge_points(text: &str) -> Result<MergePoints, String> {
    let points = text.split(';').map(point).collect::<Result<Vec<_>, _>>()?;
    if points.len() < 2 {
        return Err(format!("a merge names two supports or more, got {text:?}"));
    }
    Ok(MergePoints(points))
}

/// What automatic placement put under one model.
pub struct SupportReport {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    pub standing: usize,
    /// The contacts the run found but could not stand a column under, plate millimetres,
    /// each with why.
    pub unheld: Vec<(Vec3, Unstood)>,
    pub trees: usize,
    pub faces: usize,
}

impl SupportReport {
    /// How many contacts the run found but could not stand a column under.
    pub fn unsupported(&self) -> usize {
        self.unheld.len()
    }
}

impl fmt::Display for SupportReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Supports")?;
        if self.lifted_mm > 0.0 {
            writeln!(f, "  lifted         {:.2} mm", self.lifted_mm)?;
        }
        writeln!(f, "  contacts       {}", self.contacts.len())?;
        writeln!(f, "  standing       {}", self.standing)?;
        writeln!(f, "  trunks         {}", self.trees)?;
        writeln!(f, "  triangles      {}", self.faces)?;
        if self.unsupported() > 0 {
            writeln!(f, "  no room for    {}", self.unsupported())?;
        }
        for why in [Unstood::HeadBlocked, Unstood::FacesUp, Unstood::NoGround] {
            let count = self.unheld.iter().filter(|(_, lost)| *lost == why).count();
            if count > 0 {
                writeln!(f, "    {:<13}{count}", why_named(why).replace('-', " "))?;
            }
        }
        writeln!(f)
    }
}

/// How the report names why a contact has no support, in the terminal and in `--json`.
pub fn why_named(why: Unstood) -> &'static str {
    match why {
        Unstood::HeadBlocked => "head-blocked",
        Unstood::FacesUp => "faces-up",
        Unstood::NoGround => "no-ground",
    }
}

impl SupportReport {
    /// What `other`, stood beside these on the same model, adds to them.
    pub fn add(&mut self, other: Self) {
        self.contacts.extend(other.contacts);
        self.standing += other.standing;
        self.unheld.extend(other.unheld);
        self.trees += other.trees;
        self.faces += other.faces;
    }
}
