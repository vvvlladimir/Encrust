//! What a failed print most likely came from: its symptoms read against the check of the
//! file it was printed from, by the map in `failures.toml` (`docs/design/print-failures.md`).

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(test)]
mod tests;

const MAP: &str = include_str!("diagnose/failures.toml");

/// What a person saw on a print, by the names the map gives them.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symptom {
    /// The print, or a part of it, never stuck to the plate or came off it.
    OffPlate,
    /// The layers came apart, or a line runs round the print.
    LayerSplit,
    /// A hole, a crater or a blowout in a wall.
    Hole,
    /// A support broke or tore out of the model.
    SupportPulled,
    /// Support marks or scars on a face meant to be seen.
    FaceMark,
    /// A hollow print cracked or leaks.
    ResinCrack,
}

impl Symptom {
    pub fn id(self) -> &'static str {
        match self {
            Self::OffPlate => "off-plate",
            Self::LayerSplit => "layer-split",
            Self::Hole => "hole",
            Self::SupportPulled => "support-pulled",
            Self::FaceMark => "face-mark",
            Self::ResinCrack => "resin-crack",
        }
    }
}

/// The part of preparing, or of printing, a cause belongs to.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    Pose,
    Hollow,
    Drains,
    Supports,
    Repair,
    Resin,
    /// Washing and curing the print once it is off the plate.
    After,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Map {
    symptom: Vec<Meaning>,
    cause: Vec<Listed>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Meaning {
    id: String,
    means: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listed {
    symptom: String,
    sign: Option<String>,
    cause: String,
    step: Step,
    #[serde(rename = "try")]
    change: String,
}

/// Each symptom with its likely causes, those the check shows first.
#[derive(Serialize, Debug)]
pub struct Diagnosis {
    pub symptoms: Vec<Read>,
}

#[derive(Serialize, Debug)]
pub struct Read {
    pub symptom: &'static str,
    pub means: String,
    pub causes: Vec<Cause>,
}

#[derive(Serialize, Debug)]
pub struct Cause {
    pub cause: String,
    pub step: Step,
    #[serde(rename = "try")]
    pub change: String,
    /// What the check shows of it; absent for a cause the check cannot see.
    pub seen: Option<String>,
}

/// The likely causes of each of `symptoms`, read against `report`, the `--json` report of the
/// check or slice of the file that was printed.
pub fn diagnose(report: &Value, symptoms: &[Symptom]) -> Result<Diagnosis> {
    let map: Map = toml::from_str(MAP).context("the map of print failures does not read")?;
    let symptoms = symptoms
        .iter()
        .map(|&symptom| read(&map, report, symptom))
        .collect::<Result<_>>()?;
    Ok(Diagnosis { symptoms })
}

fn read(map: &Map, report: &Value, symptom: Symptom) -> Result<Read> {
    let id = symptom.id();
    let means = map
        .symptom
        .iter()
        .find(|meaning| meaning.id == id)
        .with_context(|| format!("the map of print failures has no symptom {id}"))?
        .means
        .clone();
    let mut seen = Vec::new();
    let mut unseen = Vec::new();
    for listed in map.cause.iter().filter(|listed| listed.symptom == id) {
        let cause = |seen| Cause {
            cause: listed.cause.clone(),
            step: listed.step,
            change: listed.change.clone(),
            seen,
        };
        match &listed.sign {
            Some(sign) => {
                if let Some(evidence) = shown(sign, report)? {
                    seen.push(cause(Some(evidence)));
                }
            }
            None => unseen.push(cause(None)),
        }
    }
    seen.append(&mut unseen);
    Ok(Read {
        symptom: id,
        means,
        causes: seen,
    })
}

/// What `report` shows of the sign `name`, or nothing where it does not show it.
fn shown(name: &str, report: &Value) -> Result<Option<String>> {
    let cured = &report["print"]["cured"];
    let count = |value: &Value| value.as_u64().unwrap_or(0);
    let models = report["models"].as_array().map_or(&[][..], Vec::as_slice);
    let summed = |pointer: &str| {
        models
            .iter()
            .filter_map(|model| model.pointer(pointer)?.as_u64())
            .sum::<u64>()
    };
    let risk = |kinds: &[&str], what: &str| risk_at(cured, kinds, what);
    Ok(match name {
        "likely-fails" => cured["likely_fails_layer"]
            .as_u64()
            .map(|layer| format!("the check expects the print to fail at layer {layer}")),
        "suction-cup" => risk(&["suction-cup"], "suction cups"),
        "peel" => risk(&["peel"], "layers that peel past their limit"),
        "lever" => risk(&["lever"], "parts on a neck past its limit"),
        "island" => risk(&["island", "floating"], "islands"),
        "thin" => risk(&["thin"], "places too thin to hold"),
        "trapped-resin" => (count(&report["slicing"]["trapped_pockets"]) > 0).then(|| {
            format!(
                "{} pockets hold {:.0} mm³ of resin with no way out",
                count(&report["slicing"]["trapped_pockets"]),
                report["slicing"]["trapped_mm3"].as_f64().unwrap_or(0.0)
            )
        }),
        "hollow" => models
            .iter()
            .find_map(|model| model["hollow"]["wall_mm"].as_f64())
            .map(|wall| format!("hollowed with a {wall} mm wall")),
        "supported" => {
            let contacts = summed("/supports/contacts");
            (contacts > 0).then(|| format!("{contacts} support tips touch the model"))
        }
        "unsupported" => {
            let bare =
                !models.is_empty() && models.iter().all(|model| model.get("supports").is_none());
            bare.then(|| "no supports: the model stands on the plate".to_owned())
        }
        "no-room" => {
            let places = summed("/supports/no_room_for");
            (places > 0).then(|| format!("{places} places had no room for a support"))
        }
        "open-mesh" => {
            let open = models
                .iter()
                .any(|model| model["repair"]["closed"].as_bool() == Some(false));
            open.then(|| "the model's surface is not closed".to_owned())
        }
        _ => bail!("the map of print failures names no sign {name}"),
    })
}

/// How many of the check's risks are of `kinds`, and where the first is.
fn risk_at(cured: &Value, kinds: &[&str], what: &str) -> Option<String> {
    let risks = cured["risks"].as_array()?;
    let of_kind: Vec<&Value> = risks
        .iter()
        .filter(|risk| {
            risk["kind"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .collect();
    let first = of_kind.first()?;
    let at: Vec<String> = first["at_mm"]
        .as_array()
        .map(|at| {
            at.iter()
                .filter_map(Value::as_f64)
                .map(|mm| format!("{mm:.1}"))
                .collect()
        })
        .unwrap_or_default();
    Some(format!(
        "{} {what}, the first at layer {}, at {} mm",
        of_kind.len(),
        first["layer"],
        at.join(", ")
    ))
}
