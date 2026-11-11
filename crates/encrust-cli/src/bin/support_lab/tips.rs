//! Each support on its own line: why it stands, what the film pulls on it, what is behind it
//! and what it stands on; and the model's weakest of them.

use anyhow::{Context, Result};
use core_analysis::{Load, Tip};
use core_engine::{WANTED_MARGIN, carried};
use core_geometry::Bvh;
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::Reason;
use serde::{Deserialize, Serialize};

use crate::shape;
use crate::stand::Supported;

/// One model's supports, tip by tip.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tips {
    pub tips: Vec<TipRow>,
    /// The smallest margin of any tip, `None` when nothing pulls on one.
    pub weakest_margin: Option<f32>,
    pub under_wanted: usize,
    /// Tips that never found the part they touch.
    pub unseated: usize,
}

/// One standing support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipRow {
    /// Where it touches the model, plate millimetres.
    pub at: [f32; 3],
    /// The group of the run's profiles it is built to.
    pub group: u16,
    pub tip_mm: f32,
    pub why: Option<Why>,
    /// The peak the pull comes to in its section, newtons, at height `worst_z_mm`.
    pub pull_n: Option<f32>,
    pub worst_z_mm: Option<f32>,
    /// What its section holds over that pull: below 1 it tears.
    pub margin: Option<f32>,
    /// Material behind it along the surface's normal, millimetres.
    pub behind_mm: Option<f32>,
    /// Its tree's height off what it lands on, millimetres.
    pub trunk_mm: Option<f32>,
    /// That height over the width of the tree's root.
    pub slenderness: Option<f32>,
}

/// Why placement asked for a support, as its report states it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    Island { area_mm2: f32, share_mm3: f32 },
    Overhang { share_mm3: f32 },
    Step { new_mm3: f32 },
    Steady,
}

impl From<Reason> for Why {
    fn from(reason: Reason) -> Self {
        match reason {
            Reason::Island {
                area_mm2,
                share_mm3,
            } => Self::Island {
                area_mm2,
                share_mm3,
            },
            Reason::Overhang { share_mm3 } => Self::Overhang { share_mm3 },
            Reason::Step { new_mm3 } => Self::Step { new_mm3 },
            Reason::Steady => Self::Steady,
        }
    }
}

/// Reads every standing tip of `supported`, cut `layer_mm` thick, a section's strength taken
/// at `neck_mpa`.
pub fn read(supported: &Supported, layer_mm: f32, neck_mpa: f32) -> Result<Tips> {
    let standing = shape::standing(supported);
    let radii: Vec<f32> = standing
        .iter()
        .map(|&at| supported.profile_of(at).surface_radius_mm())
        .collect();
    let seats: Vec<Tip> = standing
        .iter()
        .zip(&radii)
        .map(|(&at, &radius)| {
            let contact = supported.contacts[at];
            Tip {
                at_mm: [contact.x, contact.y],
                z_mm: contact.z,
                radius_mm: f64::from(radius),
            }
        })
        .collect();
    let settings = SliceSettings {
        layer_height: layer_mm,
        ..SliceSettings::default()
    };
    let stack = PlaneSliceEngine
        .slice(&supported.model, &settings)
        .context("the model cannot be cut for its tips")?;
    let loads = carried(&stack, &supported.model, layer_mm, &seats)?;
    let bvh = Bvh::build(&supported.model);
    let trunks = trunks(supported);
    let tips: Vec<TipRow> = standing
        .iter()
        .zip(&radii)
        .zip(&loads)
        .map(|((&at, &radius), load)| {
            let contact = supported.contacts[at];
            let (pull_n, margin) = reading(load, radius, neck_mpa);
            let (trunk_mm, slenderness) = trunks[at].unzip();
            TipRow {
                at: contact.to_array(),
                group: supported.groups[at],
                tip_mm: supported.profile_of(at).tip.contact_diameter_mm,
                why: supported.reasons.get(at).copied().map(Why::from),
                pull_n,
                worst_z_mm: load.z_mm,
                margin,
                behind_mm: shape::surface_at(&supported.model, &bvh, contact)
                    .and_then(|(_, behind)| behind),
                trunk_mm,
                slenderness,
            }
        })
        .collect();
    Ok(summed(tips, &loads))
}

fn reading(load: &Load, radius_mm: f32, neck_mpa: f32) -> (Option<f32>, Option<f32>) {
    if load.z_mm.is_none() {
        return (None, None);
    }
    let area = std::f64::consts::PI * f64::from(radius_mm) * f64::from(radius_mm);
    (
        Some((load.stress_mpa * area) as f32),
        Some((f64::from(neck_mpa) / load.stress_mpa) as f32),
    )
}

/// The height and slenderness of the tree over each contact.
fn trunks(supported: &Supported) -> Vec<Option<(f32, f32)>> {
    let mut trunks = vec![None; supported.contacts.len()];
    for tree in &supported.trees {
        let trunk = shape::trunk(tree);
        for point in tree.points() {
            trunks[point] = Some(trunk);
        }
    }
    trunks
}

fn summed(tips: Vec<TipRow>, loads: &[Load]) -> Tips {
    let margins = || tips.iter().filter_map(|tip| tip.margin);
    Tips {
        weakest_margin: margins().reduce(f32::min),
        under_wanted: margins()
            .filter(|margin| f64::from(*margin) < WANTED_MARGIN)
            .count(),
        unseated: loads.iter().filter(|load| !load.seated).count(),
        tips,
    }
}

/// The line a model's block gives its tips.
pub fn print(tips: &Tips) {
    let weakest = tips
        .tips
        .iter()
        .filter_map(|tip| Some((tip, tip.margin?)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((tip, margin)) = weakest else {
        println!(
            "- per tip: nothing pulls on any tip · {} unseated",
            tips.unseated
        );
        return;
    };
    let [x, y, z] = tip.at;
    println!(
        "- per tip: weakest ×{margin:.2} ({:.2} N at ({x:.1}, {y:.1}, {z:.1}), worst at {:.1} mm), {} under ×{WANTED_MARGIN} · {} unseated",
        tip.pull_n.unwrap_or(0.0),
        tip.worst_z_mm.unwrap_or(0.0),
        tips.under_wanted,
        tips.unseated,
    );
}
