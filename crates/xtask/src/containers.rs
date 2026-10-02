//! Which sliced-file container a source profile's `FILEFORMAT_` keyword names, and
//! whether this workspace writes it yet.

use printer_profiles::{AnycubicExtension as Ext, OutputFormat, PhotonRevision as Rev};

/// Every Photon Workshop target `format-anycubic` writes: the keyword a source profile
/// names, the revision beside it, and the extension and revision we write for it. One
/// extension appears twice, read at two revisions by two machines.
const PHOTON_WORKSHOP: [(&str, u32, Ext, Rev); 18] = [
    ("PWX", 1, Ext::Pwx, Rev::V1),
    ("PW0", 1, Ext::Pw0, Rev::V1),
    ("PWMX", 516, Ext::Pwmx, Rev::V516),
    ("PWMO", 516, Ext::Pwmo, Rev::V516),
    ("PWMS", 516, Ext::Pwms, Rev::V516),
    ("PMSQ", 516, Ext::Pmsq, Rev::V516),
    ("DLP", 516, Ext::Dlp, Rev::V516),
    ("PWMA", 516, Ext::Pwma, Rev::V516),
    ("PM3", 516, Ext::Pm3, Rev::V516),
    ("PM3M", 516, Ext::Pm3m, Rev::V516),
    ("PWMB", 516, Ext::Pwmb, Rev::V516),
    ("PWMB", 517, Ext::Pwmb, Rev::V517),
    ("DL2P", 517, Ext::Dl2p, Rev::V517),
    ("PX6S", 517, Ext::Px6s, Rev::V517),
    ("PMX2", 517, Ext::Pmx2, Rev::V517),
    ("PM3N", 517, Ext::Pm3n, Rev::V517),
    ("PM3R", 517, Ext::Pm3r, Rev::V517),
    ("PM5", 517, Ext::Pm5, Rev::V517),
];

/// The Photon Workshop target a keyword and revision name, where we write that pairing.
fn photon_workshop(keyword: &str, version: Option<u32>) -> Option<OutputFormat> {
    let version = version?;
    PHOTON_WORKSHOP
        .iter()
        .find(|(name, revision, _, _)| *name == keyword && *revision == version)
        .map(|&(_, _, extension, revision)| OutputFormat::Anycubic {
            extension,
            revision,
        })
}

/// What becomes of a machine whose firmware reads a given container.
pub enum Container {
    /// One the pipeline already writes. `older` marks a source profile that asks for a
    /// revision below the oldest we write, which the file it produces has to say.
    Write { output: OutputFormat, older: bool },
    /// One whose codec a later step brings, with that step named.
    Waiting(&'static str),
}

/// The container a keyword names, at the revision beside it and under the firmware class
/// the notes state, where they state one.
pub fn resolve(keyword: &str, version: Option<u32>, class: Option<&str>) -> Container {
    let write = |output| Container::Write {
        output,
        older: false,
    };
    // One extension, two firmwares: a named class is a container of its own, whose program
    // is not the one we write even where the archive around it is the same.
    if let Some(class) = class {
        return Container::Waiting(waiting_on_class(class));
    }
    if let Some(output) = photon_workshop(keyword, version) {
        return write(output);
    }
    match (keyword, version) {
        ("GOO", _) => write(OutputFormat::Goo),
        ("CTB" | "GKTWO.CTB", Some(4)) => write(OutputFormat::Ctb4),
        ("CTB" | "GKTWO.CTB", Some(5)) => write(OutputFormat::Ctb5),
        // Version 4 is the oldest `.ctb` we write, and a board of this generation needs
        // firmware from 2021 or later to read it; see docs/formats/chitu.md.
        ("CTB", None | Some(..=3)) => Container::Write {
            output: OutputFormat::Ctb4,
            older: true,
        },
        ("PHOTON", Some(2)) => write(OutputFormat::Photon),
        // The keyword names no revision: the archive has none to name.
        ("ZIP", _) => write(OutputFormat::Zip),
        ("CXDLP", _) => write(OutputFormat::Cxdlp3),
        ("CXDLPV4", _) => write(OutputFormat::Cxdlp4),
        ("SVGX", _) => write(OutputFormat::Svgx),
        ("CWS", _) => write(OutputFormat::Cws),
        _ => Container::Waiting(waiting_on(keyword, version)),
    }
}

/// The step that brings the container a named firmware class reads.
fn waiting_on_class(class: &str) -> &'static str {
    match class {
        "KLIPPER" => "step 21g, the Klipper archive",
        _ => "step 21g, the long tail",
    }
}

/// The step that brings the codec for a container we do not write.
fn waiting_on(keyword: &str, version: Option<u32>) -> &'static str {
    if is_photon_workshop(keyword) {
        return match version {
            Some(515..=518) => "step 21c, Photon Workshop at version 5",
            _ => "step 21c, the rest of the Anycubic line",
        };
    }
    match keyword {
        "ENCRYPTED.CTB" => "step 21b, the encrypted .ctb",
        "RGB.CWS" | "XML.CWS" => "step 21g, the rest of the CWS family",
        _ => "step 21g, the long tail",
    }
}

/// Whether a keyword names one of the Photon Workshop extensions, every one of which is
/// the same table container at some revision; see `docs/formats/anycubic.md`.
fn is_photon_workshop(keyword: &str) -> bool {
    const PREFIXES: [&str; 6] = ["PWM", "PWS", "PM", "PP1", "DL", "PX"];
    matches!(keyword, "M5SP") || PREFIXES.iter().any(|p| keyword.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(keyword: &str, version: Option<u32>) -> OutputFormat {
        match resolve(keyword, version, None) {
            Container::Write { output, .. } => output,
            Container::Waiting(step) => panic!("{keyword} should be written, not {step}"),
        }
    }

    fn step(keyword: &str, version: Option<u32>) -> &'static str {
        match resolve(keyword, version, None) {
            Container::Write { output, .. } => {
                panic!("{keyword} should wait, not write {output:?}")
            }
            Container::Waiting(step) => step,
        }
    }

    #[test]
    fn a_goo_machine_is_written_whatever_revision_it_names() {
        assert_eq!(output("GOO", None), OutputFormat::Goo);
        assert_eq!(output("GOO", Some(51)), OutputFormat::Goo);
    }

    #[test]
    fn a_ctb_below_version_four_is_written_as_version_four_and_says_so() {
        let Container::Write { output, older } = resolve("CTB", Some(3), None) else {
            panic!("version 3 is written as version 4");
        };
        assert_eq!(output, OutputFormat::Ctb4);
        assert!(older, "the file has to carry the revision it was asked for");

        let Container::Write { older, .. } = resolve("CTB", Some(4), None) else {
            panic!("version 4 is written as itself");
        };
        assert!(!older);
    }

    #[test]
    fn the_gktwo_variant_is_a_ctb_of_its_stated_revision() {
        assert_eq!(output("GKTWO.CTB", Some(4)), OutputFormat::Ctb4);
    }

    #[test]
    fn a_photon_workshop_keyword_carries_the_revision_it_names() {
        assert_eq!(
            output("PWX", Some(1)),
            OutputFormat::Anycubic {
                extension: Ext::Pwx,
                revision: Rev::V1
            }
        );
        assert_eq!(
            output("PM5", Some(517)),
            OutputFormat::Anycubic {
                extension: Ext::Pm5,
                revision: Rev::V517
            }
        );
        assert_eq!(output("PHOTON", Some(2)), OutputFormat::Photon);
    }

    #[test]
    fn one_extension_is_read_at_two_revisions_by_two_machines() {
        for revision in [Rev::V516, Rev::V517] {
            let named = match revision {
                Rev::V516 => 516,
                _ => 517,
            };
            assert_eq!(
                output("PWMB", Some(named)),
                OutputFormat::Anycubic {
                    extension: Ext::Pwmb,
                    revision
                }
            );
        }
    }

    #[test]
    fn the_archive_keyword_is_written_whatever_revision_it_names() {
        assert_eq!(output("ZIP", None), OutputFormat::Zip);
        assert_eq!(output("ZIP", Some(1)), OutputFormat::Zip);
    }

    #[test]
    fn an_archive_another_firmware_reads_waits_for_that_firmwares_program() {
        let Container::Waiting(step) = resolve("ZIP", None, Some("KLIPPER")) else {
            panic!("the archive is the same zip, the program in it is not ours");
        };
        assert!(step.contains("Klipper"), "{step}");
    }

    #[test]
    fn a_photon_workshop_revision_we_do_not_write_waits() {
        assert!(step("PM5S", Some(518)).contains("21c"));
        assert!(
            step("PM7", None).contains("21c"),
            "an extension the source names no revision for cannot be guessed at"
        );
    }

    #[test]
    fn every_container_we_do_not_write_names_the_step_that_brings_it() {
        let expected = [
            ("ENCRYPTED.CTB", None, "21b"),
            ("PP1", None, "21c"),
            ("M5SP", Some(518), "21c"),
            ("PWS", Some(1), "21c"),
            ("PM5S", Some(518), "21c"),
            ("RGB.CWS", None, "21g"),
            ("XML.CWS", None, "21g"),
            ("LGS4K", None, "21g"),
            ("N4", None, "21g"),
        ];
        for (keyword, version, step_name) in expected {
            let waiting = step(keyword, version);
            assert!(
                waiting.contains(step_name),
                "{keyword} waits on {waiting}, not {step_name}"
            );
        }
    }
}
