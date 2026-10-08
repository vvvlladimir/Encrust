//! The bug report the window writes: what the user typed, the build it ran, the profiles
//! in hand and the values the tools are set to, as one markdown file.
//!
//! Nothing here reaches the network. The report is shown in full and then copied, saved or
//! carried into a prefilled issue by the user; see `docs/decisions/0212`.

use std::fmt::Write as _;
use std::path::Path;

use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::tool_settings::ToolSettings;

/// The issue form a report is carried into, and the field ids it prefills, are those of
/// `.github/ISSUE_TEMPLATE/bug.yml`.
const NEW_ISSUE: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new");
const TEMPLATE: &str = "bug.yml";

/// How long the issue URL may grow. GitHub answers a longer one with `414 URI Too Long`
/// around eight kilobytes, so the report never travels in the URL: it goes to the
/// clipboard, and the form gets the fields that fit.
const MAX_URL: usize = 6_000;

/// The file a saved report is offered under.
pub const FILE_NAME: &str = "encrust-report.md";

/// What the window ran on, worded as the issue form's dropdown lists it.
const PLATFORM: &str = if cfg!(target_arch = "wasm32") {
    "Browser"
} else if cfg!(target_os = "windows") {
    "Windows"
} else if cfg!(target_os = "macos") {
    "macOS"
} else {
    "Linux"
};

/// Characters cut off the end of a word before it is read as a path, so that
/// `cannot open /home/ada/boat.stl:` is scrubbed along with the plain name.
const TRAILING: [char; 7] = ['.', ',', ':', ';', ')', '"', '\''];

/// Which parts of the window the report carries. Each is the user's to leave out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Include {
    /// The printer and resin profiles, as TOML.
    pub profiles: bool,
    /// Every value the tool panels are set to.
    pub settings: bool,
    /// How much stands on the plate.
    pub plate: bool,
}

impl Default for Include {
    fn default() -> Self {
        Self {
            profiles: true,
            settings: true,
            plate: true,
        }
    }
}

/// What stands on the plate, counted. Names and files are left out on purpose: a model's
/// name is the user's, and a bug is reproduced from its size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plate {
    pub models: usize,
    pub triangles: usize,
    pub vertices: usize,
}

/// What the window knows about the run, for the report to state.
pub struct Facts<'a> {
    pub printer: Option<&'a PrinterProfile>,
    /// The catalogue id of that printer, when it came from the catalogue.
    pub printer_id: Option<&'a str>,
    pub resin: &'a MaterialProfile,
    pub resin_id: Option<&'a str>,
    /// The extension the next file would be written under.
    pub format: &'a str,
    /// The failure on the status strip, when it is showing one.
    pub message: Option<String>,
    pub plate: Plate,
    pub settings: &'a ToolSettings,
}

/// What the user is writing, and what of the window goes with it.
#[derive(Debug, Default)]
pub struct Report {
    pub open: bool,
    /// What happened, and what was expected instead.
    pub what: String,
    /// The clicks or the command that bring it back.
    pub steps: String,
    pub include: Include,
}

impl Report {
    /// The whole report, which is what the sheet shows and what every button hands over.
    pub fn markdown(&self, facts: &Facts<'_>) -> String {
        let mut text = String::new();
        told(
            &mut text,
            "What happened, and what was expected",
            &self.what,
        );
        told(&mut text, "How to reproduce it", &self.steps);

        text.push_str("## Encrust\n\n");
        for (key, value) in environment(facts) {
            let _ = writeln!(text, "- **{key}:** {value}");
        }
        text.push('\n');

        if self.include.plate {
            let plate = facts.plate;
            text.push_str("## What stands on the plate\n\n");
            let _ = writeln!(
                text,
                "{} models, {} triangles, {} vertices. Model names and files are not in \
                 this report.\n",
                plate.models, plate.triangles, plate.vertices
            );
        }
        if self.include.profiles {
            if let Some(printer) = facts.printer {
                fenced(&mut text, "Printer profile", "toml", &printer_toml(printer));
            }
            fenced(&mut text, "Resin profile", "toml", &resin_toml(facts.resin));
        }
        if self.include.settings {
            fenced(
                &mut text,
                "Tool settings",
                "json",
                &settings_json(facts.settings),
            );
        }

        text.push_str(FOOTER);
        text
    }

    /// The issue form, prefilled with the fields that fit in a URL. The report itself does
    /// not fit, so whatever opens this puts it on the clipboard as well.
    pub fn issue_url(&self, facts: &Facts<'_>) -> String {
        let fields = [
            ("version", env!("CARGO_PKG_VERSION").to_owned()),
            ("os", PLATFORM.to_owned()),
            ("printer", facts.printer.map(machine).unwrap_or_default()),
            (
                "log",
                facts.message.as_deref().map(scrubbed).unwrap_or_default(),
            ),
            ("what", self.what.clone()),
            ("steps", self.steps.clone()),
        ];
        let mut url = format!("{NEW_ISSUE}?template={TEMPLATE}");
        for (key, value) in fields {
            if value.is_empty() {
                continue;
            }
            let pair = format!("&{key}={}", encoded(&value));
            // Skipped rather than cut off: a field whose text is too long is in the
            // clipboard whole, and the short fields after it still fit.
            if url.len() + pair.len() <= MAX_URL {
                url.push_str(&pair);
            }
        }
        url
    }
}

/// The line every report ends on, so a reader of the issue knows what it cannot contain.
const FOOTER: &str = concat!(
    "---\n\nWritten by Encrust ",
    env!("CARGO_PKG_VERSION"),
    ". It carries no model, no file name and no printer address.\n"
);

/// One of the two things the user typed, under its heading. An empty answer still gets its
/// heading: the issue form asks for both, and a gap is easier to fill in than to notice.
fn told(text: &mut String, heading: &str, said: &str) {
    let said = said.trim();
    let body = if said.is_empty() { "—" } else { said };
    let _ = writeln!(text, "## {heading}\n\n{body}\n");
}

fn fenced(text: &mut String, heading: &str, language: &str, body: &str) {
    let _ = writeln!(
        text,
        "## {heading}\n\n```{language}\n{}\n```\n",
        body.trim_end()
    );
}

/// The build, the machine it is set up for, and the failure on the strip.
fn environment(facts: &Facts<'_>) -> Vec<(&'static str, String)> {
    let named = |name: String, id: Option<&str>| match id {
        Some(id) => format!("{name} (`{id}`)"),
        None => format!("{name} (a profile from a file)"),
    };
    let mut lines = vec![
        ("Version", env!("CARGO_PKG_VERSION").to_owned()),
        (
            "Where it ran",
            format!("{PLATFORM}, {}", std::env::consts::ARCH),
        ),
        (
            "Printer",
            facts.printer.map_or_else(
                || "none chosen".to_owned(),
                |printer| named(machine(printer), facts.printer_id),
            ),
        ),
        ("Resin", named(facts.resin.name.clone(), facts.resin_id)),
        ("Output format", format!(".{}", facts.format)),
    ];
    if let Some(message) = &facts.message {
        lines.push(("Last failure", scrubbed(message)));
    }
    lines
}

/// A printer as the report names it: the profile's name, and the machine name the
/// firmware is matched against where that differs.
fn machine(printer: &PrinterProfile) -> String {
    match &printer.machine_name {
        Some(machine) if *machine != printer.name => format!("{} / {machine}", printer.name),
        _ => printer.name.clone(),
    }
}

/// A profile that cannot be written out says so inside its own fence: the reason is itself
/// worth reporting, and a report with a gap in it is worse than one with a comment.
fn printer_toml(printer: &PrinterProfile) -> String {
    printer
        .to_toml_string(Path::new("printer.toml"))
        .unwrap_or_else(|error| format!("# this profile could not be written out: {error}"))
}

fn resin_toml(resin: &MaterialProfile) -> String {
    resin
        .to_toml_string(Path::new("resin.toml"))
        .unwrap_or_else(|error| format!("# this profile could not be written out: {error}"))
}

fn settings_json(settings: &ToolSettings) -> String {
    serde_json::to_string_pretty(settings)
        .unwrap_or_else(|error| format!("\"these settings could not be written out: {error}\""))
}

/// A message as the report states it, on one line: a path or a file name in it is cut to
/// its extension, because the name is the user's and the extension is what the bug needs.
fn scrubbed(message: &str) -> String {
    message
        .split_whitespace()
        .map(scrubbed_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn scrubbed_word(word: &str) -> String {
    let kept = word.trim_end_matches(TRAILING);
    let tail = &word[kept.len()..];
    if kept.starts_with("http://") || kept.starts_with("https://") {
        return word.to_owned();
    }
    let extension = Path::new(kept)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let path = kept.contains('/') || kept.contains('\\');
    let cut = match (path, extension) {
        (false, None) => return word.to_owned(),
        (false, Some(extension)) if !is_ours(&extension) => return word.to_owned(),
        (false, Some(extension)) => format!("*.{extension}"),
        (true, Some(extension)) => format!(".../*.{extension}"),
        (true, None) => ".../*".to_owned(),
    };
    format!("{cut}{tail}")
}

/// Whether an extension is one of the files the window opens or writes, which is what
/// makes a bare word a file name rather than a version or a number.
fn is_ours(extension: &str) -> bool {
    let mut known = crate::files::MESHES
        .iter()
        .chain(&crate::files::PROJECTS)
        .chain(&crate::sliced::EXTENSIONS);
    extension == "toml" || known.any(|known| *known == extension)
}

/// Percent-encodes everything but the unreserved characters of RFC 3986, so that the
/// newlines and punctuation of a prefilled field survive the query string.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(settings: &'a ToolSettings, resin: &'a MaterialProfile) -> Facts<'a> {
        Facts {
            printer: None,
            printer_id: None,
            resin,
            resin_id: Some("grey"),
            format: "goo",
            message: None,
            plate: Plate {
                models: 2,
                triangles: 43_834,
                vertices: 22_579,
            },
            settings,
        }
    }

    fn written(report: &Report) -> String {
        let settings = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
        let resin = MaterialProfile::default();
        report.markdown(&facts(&settings, &resin))
    }

    #[test]
    fn the_report_states_the_build_and_what_the_user_typed() {
        let report = Report {
            what: "the mask comes out mirrored".to_owned(),
            steps: "slice the cube, open the file".to_owned(),
            ..Report::default()
        };
        let text = written(&report);
        assert!(text.contains("## What happened, and what was expected"));
        assert!(text.contains("the mask comes out mirrored"));
        assert!(text.contains("slice the cube, open the file"));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains(PLATFORM));
        assert!(text.contains("Output format:** .goo"));
    }

    #[test]
    fn nothing_typed_still_makes_a_report() {
        let text = written(&Report::default());
        assert!(text.contains("## How to reproduce it\n\n—"));
    }

    #[test]
    fn a_switch_turned_off_keeps_its_section_out() {
        let full = written(&Report::default());
        assert!(full.contains("## Resin profile"));
        assert!(full.contains("## Tool settings"));
        assert!(full.contains("## What stands on the plate"));

        let bare = written(&Report {
            include: Include {
                profiles: false,
                settings: false,
                plate: false,
            },
            ..Report::default()
        });
        assert!(!bare.contains("## Resin profile"));
        assert!(!bare.contains("## Tool settings"));
        assert!(!bare.contains("## What stands on the plate"));
        assert!(bare.contains("## Encrust"), "the build is always stated");
    }

    #[test]
    fn a_path_in_a_failure_is_cut_to_its_extension() {
        assert_eq!(
            scrubbed("cannot load /home/ada/patient.stl: no such file"),
            "cannot load .../*.stl: no such file"
        );
        assert_eq!(
            scrubbed(r"cannot write C:\Users\Ada\plate.goo"),
            "cannot write .../*.goo"
        );
        assert_eq!(scrubbed("cannot load patient.3MF"), "cannot load *.3mf");
        assert_eq!(scrubbed("cannot read ../out"), "cannot read .../*");
    }

    #[test]
    fn a_failure_keeps_the_words_that_are_not_names() {
        assert_eq!(
            scrubbed("version 0.1.0 refused the file"),
            "version 0.1.0 refused the file"
        );
        assert_eq!(
            scrubbed("see https://encrust.app/guides/ for this"),
            "see https://encrust.app/guides/ for this"
        );
    }

    #[test]
    fn the_prefilled_issue_names_the_form_and_stays_under_the_budget() {
        let report = Report {
            what: "x".repeat(20_000),
            steps: "open the window".to_owned(),
            ..Report::default()
        };
        let settings = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
        let resin = MaterialProfile::default();
        let mut facts = facts(&settings, &resin);
        facts.message = Some("cannot load /home/ada/patient.stl".to_owned());
        let url = report.issue_url(&facts);

        assert!(url.len() <= MAX_URL, "a URL GitHub refuses is no help");
        assert!(url.starts_with(NEW_ISSUE));
        assert!(url.contains("template=bug.yml"));
        assert!(url.contains(&format!("version={}", env!("CARGO_PKG_VERSION"))));
        assert!(url.contains("os=") && url.contains(PLATFORM));
        assert!(
            url.contains("log=cannot%20load%20...%2F%2A.stl"),
            "the path is scrubbed in the form too, not only in the report"
        );
        assert!(
            url.contains("steps=open%20the%20window"),
            "a short field after a long one still fits"
        );
        assert!(
            !url.contains("what="),
            "the long one is left to the clipboard"
        );
    }
}
