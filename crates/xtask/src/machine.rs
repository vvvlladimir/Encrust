//! One source profile turned into a `PrinterProfile`, plus the few facts the source does
//! not carry. See `docs/design/profiles.md`.

use anyhow::{Result, anyhow};
use printer_profiles::{BuildVolume, Connection, Display, OutputFormat, PrinterProfile};

use crate::containers::{self, Container};
use crate::ini::Ini;

/// Every brand the source names a machine after, spelled the way we write it.
const BRANDS: [&str; 20] = [
    "Anet",
    "Anycubic",
    "Concepts3D",
    "Creality",
    "Elegoo",
    "Emake3D",
    "EPAX",
    "FlashForge",
    "Kelant",
    "Longer",
    "Nova3D",
    "Peopoly",
    "Phrozen",
    "Prusa",
    "QIDI",
    "UniFormation",
    "Uniz",
    "Voxelab",
    "Wanhao",
    "Zortrax",
];

/// Where a model name has to be written differently from the source's file name.
const MODEL_NAMES: [(&str, &str); 1] = [("prusa-sl1s-speed", "SL1S Speed")];

/// The container for a machine whose source profile names none; both of these state an
/// archive format instead of a `FILEFORMAT_` keyword.
const OUTPUTS: [(&str, OutputFormat); 2] = [
    ("prusa-sl1", OutputFormat::Sl1),
    ("prusa-sl1s-speed", OutputFormat::Sl1s),
];

/// Where a source profile's travel is wrong and the published figure is not. Each line is
/// a judgement made once, with what settles it.
const TRAVEL_MM: [(&str, f32, &str); 1] = [(
    "anycubic-photon-mono-x2",
    260.0,
    "the vendor and the machine list both state 260 mm; the source profile carries 200",
)];

/// Which machines answer on the network, and on what. The source says nothing about it,
/// so these are the generations named in `docs/formats/sdcp.md` and
/// `docs/formats/prusalink.md` and no others.
const CONNECTIONS: [(&str, Connection); 4] = [
    ("elegoo-mars-5", Connection::Sdcp),
    ("elegoo-saturn-3-ultra", Connection::Sdcp),
    ("prusa-sl1", Connection::PrusaLink),
    ("prusa-sl1s-speed", Connection::PrusaLink),
];

/// A machine ready to be written out, or the reason it is not.
pub enum Transcribed {
    Ready(Machine),
    /// Its container waits on a later step, which this names.
    Waiting(&'static str),
}

/// One machine of ours, with what the generator had to decide about it.
pub struct Machine {
    pub id: String,
    pub profile: PrinterProfile,
    /// Whether the source asked for a container revision older than the one we write.
    pub older_revision: bool,
}

/// Reads one source profile. `stem` is its file name without the extension, which is
/// where the brand and the model come from.
pub fn transcribe(stem: &str, ini: &Ini) -> Result<Transcribed> {
    let (manufacturer, model) = split_brand(stem)?;
    let id = identifier(manufacturer, &model);
    let model = MODEL_NAMES
        .iter()
        .find(|(known, _)| *known == id)
        .map_or(model, |(_, name)| (*name).to_owned());

    let Some(container) = container(&id, ini) else {
        return Err(anyhow!("the source profile names no container"));
    };
    let (output, older_revision) = match container {
        Container::Write { output, older } => (output, older),
        Container::Waiting(step) => return Ok(Transcribed::Waiting(step)),
    };

    let display = display(ini)?;
    let profile = PrinterProfile {
        name: model,
        manufacturer: manufacturer.to_owned(),
        // Only a file the machine's own slicer wrote settles this, and the source is not
        // one; see ADR 0140.
        machine_name: None,
        build_volume: BuildVolume {
            x: display.width_mm,
            y: display.height_mm,
            z: travel_mm(&id, ini)?,
        },
        display,
        mirror_x: ini.flag("display_mirror_x")?,
        mirror_y: ini.flag("display_mirror_y")?,
        output,
        connection: connection(&id),
        firmware: printer_profiles::Firmware {
            // A board old enough to be given a `.ctb` below version 4 predates the
            // firmware that reads the per-layer tables.
            per_layer_settings: !older_revision,
            variable_layer_height: false,
        },
    };
    Ok(Transcribed::Ready(Machine {
        id,
        profile,
        older_revision,
    }))
}

/// How far the plate travels, which one source profile states wrongly.
fn travel_mm(id: &str, ini: &Ini) -> Result<f32> {
    match TRAVEL_MM.iter().find(|(known, _, _)| *known == id) {
        Some(&(_, travel, _)) => Ok(travel),
        None => ini.f32("max_print_height"),
    }
}

fn display(ini: &Ini) -> Result<Display> {
    Ok(Display {
        width_px: ini.u32("display_pixels_x")?,
        height_px: ini.u32("display_pixels_y")?,
        width_mm: ini.f32("display_width")?,
        height_mm: ini.f32("display_height")?,
        // Not in the source, and a panel's dimmest usable grey is measured rather than
        // published; see ADR 0139.
        grey_floor: 0,
    })
}

/// The container the machine's firmware reads, from the keyword or from the one table
/// above for the machines whose source profile carries none.
fn container(id: &str, ini: &Ini) -> Option<Container> {
    if let Some((keyword, version)) = ini.container_keyword() {
        return Some(containers::resolve(
            &keyword,
            version,
            ini.file_class().as_deref(),
        ));
    }
    OUTPUTS
        .iter()
        .find(|(known, _)| *known == id)
        .map(|&(_, output)| Container::Write {
            output,
            older: false,
        })
}

fn connection(id: &str) -> Connection {
    CONNECTIONS
        .iter()
        .find(|(known, _)| *known == id)
        .map_or(Connection::None, |&(_, connection)| connection)
}

/// Splits a file name into the brand and the model. The brand is the first word that
/// names one, because a file name may carry a prefix in front of it.
fn split_brand(stem: &str) -> Result<(&'static str, String)> {
    for (index, word) in stem.split_whitespace().enumerate() {
        if let Some(brand) = BRANDS.iter().find(|known| known.eq_ignore_ascii_case(word)) {
            let model: Vec<&str> = stem.split_whitespace().skip(index + 1).collect();
            if model.is_empty() {
                return Err(anyhow!("`{stem}` names a brand and no model"));
            }
            return Ok((brand, model.join(" ")));
        }
    }
    Err(anyhow!("`{stem}` names no brand this generator knows"))
}

/// The catalogue id: the brand and the model, lowercased, with every run of anything else
/// as a single hyphen.
fn identifier(manufacturer: &str, model: &str) -> String {
    let mut id = String::with_capacity(manufacturer.len() + model.len() + 1);
    for character in format!("{manufacturer} {model}").chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character.to_ascii_lowercase());
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brand_is_the_first_word_that_names_one() {
        let (brand, model) = split_brand("Elegoo Mars 4 Ultra").expect("a known brand");
        assert_eq!((brand, model.as_str()), ("Elegoo", "Mars 4 Ultra"));

        let (brand, model) = split_brand("Some Tool Prusa SL1S SPEED").expect("a known brand");
        assert_eq!(
            (brand, model.as_str()),
            ("Prusa", "SL1S SPEED"),
            "a prefix in front of the brand is not part of the model"
        );
    }

    #[test]
    fn a_brand_is_matched_whatever_its_case_and_written_our_way() {
        let (brand, _) = split_brand("uniformation GKtwo").expect("a known brand");
        assert_eq!(brand, "UniFormation");
    }

    #[test]
    fn a_file_naming_no_brand_is_refused() {
        assert!(split_brand("Mystery 9000").is_err());
        assert!(split_brand("Elegoo").is_err());
    }

    #[test]
    fn an_identifier_is_the_brand_and_model_in_kebab_case() {
        assert_eq!(identifier("Elegoo", "Mars 4 Ultra"), "elegoo-mars-4-ultra");
        assert_eq!(identifier("UniFormation", "GKtwo"), "uniformation-gktwo");
        assert_eq!(identifier("QIDI", "Shadow6.0 Pro"), "qidi-shadow6-0-pro");
        assert_eq!(identifier("EPAX", "X156 4K Color"), "epax-x156-4k-color");
    }

    #[test]
    fn only_the_machines_in_the_table_are_reachable() {
        assert_eq!(connection("elegoo-mars-5"), Connection::Sdcp);
        assert_eq!(connection("prusa-sl1"), Connection::PrusaLink);
        assert_eq!(connection("elegoo-saturn-3"), Connection::None);
    }
}
