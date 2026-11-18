//! The bug report the window writes: what the user typed, the build it ran, the profiles
//! in hand and the values the tools are set to, as one markdown file.
//!
//! Nothing here reaches the network. The report is shown in full and then copied, saved or
//! carried into a prefilled issue by the user; see `docs/decisions/0212`.

use std::fmt::Write as _;
use std::path::Path;

use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::tool_settings::ToolSettings;
use scrub::scrubbed;

/// The issue form a report is carried into, and the field ids it prefills, are those of
/// `.github/ISSUE_TEMPLATE/bug.yml`.
const NEW_ISSUE: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new");
const TEMPLATE: &str = "bug.yml";

/// What the form's report field says when the report is too long for the URL.
const PASTE_HERE: &str = "Paste the report from Encrust here: it is on your clipboard.";

/// How long the issue URL may grow. GitHub answers a longer one with `414 URI Too Long`
/// around eight kilobytes, so a report that does not fit is left to the clipboard.
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
    /// Whether that printer is exactly as this build ships it, so its id alone says it.
    pub printer_shipped: bool,
    pub resin: &'a MaterialProfile,
    pub resin_id: Option<&'a str>,
    /// The extension the next file would be written under.
    pub format: &'a str,
    /// The failure over the stage, when it is showing one.
    pub message: Option<String>,
    /// Every address and name the window knows a printer by, so that a failure quoting one
    /// can be scrubbed of it; see `Network::hosts`.
    pub hosts: &'a [String],
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
    /// The panic the last run closed on, offered once at this start and dropped with the
    /// sheet.
    pub crash: Option<String>,
    /// Whether the report went to the issue form since the sheet opened.
    pub handed: bool,
}

impl Report {
    /// Opens the sheet on the panic the last run closed on, if it closed on one.
    pub fn offer_crash(&mut self) {
        self.crash = crash::take();
        self.open |= self.crash.is_some();
    }

    /// The whole report, which is what the sheet shows and what every button hands over.
    pub fn markdown(&self, facts: &Facts<'_>) -> String {
        let mut text = String::new();
        told(
            &mut text,
            "What happened, and what was expected",
            &self.what,
        );
        told(&mut text, "How to reproduce it", &self.steps);
        text + &self.attached(facts)
    }

    /// What the window adds to what the user typed: the issue form's report field, whose
    /// own fields already carry the typed answers.
    fn attached(&self, facts: &Facts<'_>) -> String {
        let mut text = String::new();
        if let Some(crash) = &self.crash {
            fenced(
                &mut text,
                "The crash it closed on",
                "text",
                &trimmed::trimmed_crash(crash),
            );
        }
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
            printer_part(&mut text, facts);
            fenced(&mut text, "Resin profile", "toml", &resin_toml(facts.resin));
        }
        if self.include.settings {
            fenced(
                &mut text,
                "Tool settings changed from a fresh window",
                "json",
                &trimmed::changed_settings(facts.settings),
            );
        }
        text.push_str(FOOTER);
        text
    }

    /// The issue form, prefilled with the fields that fit in a URL; whatever opens it puts
    /// the report on the clipboard as well, for a report too long to travel in it.
    pub fn issue_url(&self, facts: &Facts<'_>) -> String {
        let fields = [
            ("version", env!("CARGO_PKG_VERSION").to_owned()),
            ("os", PLATFORM.to_owned()),
            ("printer", facts.printer.map(machine).unwrap_or_default()),
            (
                "log",
                self.crash.as_deref().map_or_else(
                    || {
                        facts
                            .message
                            .as_deref()
                            .map(|message| scrubbed(message, facts.hosts))
                            .unwrap_or_default()
                    },
                    crash_head,
                ),
            ),
            ("what", self.what.clone()),
            ("steps", self.steps.clone()),
        ];
        let paste_here = format!("&report={}", encoded(PASTE_HERE));
        let budget = MAX_URL - paste_here.len();
        let mut url = format!("{NEW_ISSUE}?template={TEMPLATE}");
        for (key, value) in fields {
            if value.is_empty() {
                continue;
            }
            let pair = format!("&{key}={}", encoded(&value));
            // Skipped rather than cut off: a field whose text is too long is in the
            // clipboard whole, and the short fields after it still fit.
            if url.len() + pair.len() <= budget {
                url.push_str(&pair);
            }
        }
        // Last, so the short fields are in first: the whole of it if it fits, else the
        // place to paste it.
        let whole = format!("&report={}", encoded(&self.attached(facts)));
        if url.len() + whole.len() <= MAX_URL {
            url.push_str(&whole);
        } else {
            url.push_str(&paste_here);
        }
        url
    }
}

/// A crash as the issue form's log field takes it: the panic without its backtrace, which
/// is in the report on the clipboard.
fn crash_head(crash: &str) -> String {
    crash.split("\n\n").next().unwrap_or(crash).to_owned()
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
        lines.push(("Last failure", scrubbed(message, facts.hosts)));
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

/// The printer as the report carries it: by its id when it is the shipped profile, which
/// a reader can open, else whole.
fn printer_part(text: &mut String, facts: &Facts<'_>) {
    let Some(printer) = facts.printer else {
        return;
    };
    match facts.printer_id {
        Some(id) if facts.printer_shipped => {
            let _ = writeln!(
                text,
                "## Printer profile\n\nThe shipped `{id}`, unchanged.\n"
            );
        }
        _ => fenced(text, "Printer profile", "toml", &printer_toml(printer)),
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

pub mod crash;
mod scrub;
#[cfg(test)]
mod tests;
mod trimmed;
