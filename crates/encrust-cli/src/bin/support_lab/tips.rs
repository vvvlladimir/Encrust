//! Each support on its own line: why it stands, what the film pulls on it, what is behind it
//! and what it stands on; and the model's weakest of them.

use anyhow::{Context, Result};
use core_analysis::{Load, SHIFT_LIMIT_MM, Tip};
use core_engine::{Held, WANTED_MARGIN, carried, held_by};
use core_geometry::{Bvh, Transform};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Placed, Profiles, Reason, SupportPoint};
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
    /// Tips under a layer that moved past the limit across.
    #[serde(default)]
    pub swaying: usize,
    /// The smallest buckling margin of any trunk, `None` where none is pressed.
    #[serde(default)]
    pub weakest_buckling: Option<f32>,
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
    /// The furthest a layer it held moved across under its pull, millimetres.
    #[serde(default)]
    pub shift_mm: Option<f32>,
    /// What its trunk holds in squeeze over the most it was pressed with.
    #[serde(default)]
    pub buckling_margin: Option<f32>,
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

/// Reads every standing tip of `supported` on the spring its tree makes, cut `layer_mm`
/// thick, a section's strength taken at `neck_mpa`.
pub fn read(supported: &Supported, layer_mm: f32, neck_mpa: f32) -> Result<Tips> {
    let standing = shape::standing(supported);
    let radii: Vec<f32> = standing
        .iter()
        .map(|&at| supported.profile_of(at).surface_radius_mm())
        .collect();
    let springs = springs(supported)?;
    let seats: Vec<Tip> = standing
        .iter()
        .zip(&radii)
        .map(|(&at, &radius)| {
            let contact = supported.contacts[at];
            Tip {
                at_mm: [contact.x, contact.y],
                z_mm: contact.z,
                radius_mm: f64::from(radius),
                spring: springs[at].map(|(spring, _)| spring),
                stands_on: springs[at].and_then(|(_, footing)| footing),
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
    let loads = carried(&stack, &supported.model, layer_mm, &seats, &mut || true)?
        .context("nothing stops the read")?;
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
                shift_mm: load.shift_at_mm.map(|_| load.shift_mm as f32),
                buckling_margin: load.buckling_margin.map(|margin| margin as f32),
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

/// The spring the tree over each contact makes, and where it stands on the model, by contact.
fn springs(supported: &Supported) -> Result<Vec<Option<Held>>> {
    let bvh = Bvh::build(&supported.model);
    let placed = Placed::new(&supported.model, &bvh, Transform::default());
    let profiles =
        Profiles::new(&supported.profiles).context("a table of profiles is never empty")?;
    let points: Vec<SupportPoint> = supported
        .contacts
        .iter()
        .zip(&supported.groups)
        .map(|(&contact, &group)| SupportPoint {
            group,
            ..SupportPoint::new(contact)
        })
        .collect();
    Ok(held_by(&supported.trees, &points, (&placed, profiles)))
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
        swaying: loads.iter().filter(|load| load.sways()).count(),
        weakest_buckling: tips
            .iter()
            .filter_map(|tip| tip.buckling_margin)
            .reduce(f32::min),
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
    println!(
        "- on their trees: {} tips under a layer moving past {SHIFT_LIMIT_MM} mm, weakest trunk ×{:.1} from buckling",
        tips.swaying,
        tips.weakest_buckling.unwrap_or(f32::INFINITY),
    );
}
