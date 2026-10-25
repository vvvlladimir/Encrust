//! What the window's printer card does to a machine besides sending it a file: pause,
//! resume and stop a print, list the last prints, the camera and the plate's picture.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use printer_link::{Command, Ending, Task, Wire};
use serde::Serialize;

use super::{Machine, MachineArgs};
use crate::commands::GlobalArgs;
use crate::estimate::hours_minutes;
use crate::json;
use crate::sliced_file::now_unix_s;

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Printer")]
pub struct HistoryArgs {
    #[command(flatten)]
    pub machine: MachineArgs,

    /// How many of the most recent prints to list.
    #[arg(long, value_name = "N", default_value_t = 10)]
    pub count: usize,
}

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Printer")]
pub struct CameraArgs {
    #[command(flatten)]
    pub machine: MachineArgs,

    /// Turn the camera on, answering where it streams, or off.
    #[arg(value_enum, help_heading = "Arguments")]
    pub switch: Switch,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    On,
    Off,
}

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Printer")]
pub struct PictureArgs {
    #[command(flatten)]
    pub machine: MachineArgs,

    /// Where the picture goes, in the format the printer serves it.
    #[arg(short, long, value_name = "PATH")]
    pub output: PathBuf,
}

#[derive(Serialize)]
struct ControlReport {
    #[serde(flatten)]
    machine: Machine,
    command: String,
}

/// Pauses, resumes or stops the print under way.
pub fn control(args: &MachineArgs, command: Command, global: &GlobalArgs) -> Result<()> {
    let wire = args.wire()?;
    let word = command.label().to_lowercase();
    printer_link::control(&wire, command)
        .with_context(|| format!("cannot {word} the print on {}", wire.name()))?;
    if global.json {
        return json::print(&ControlReport {
            machine: Machine::of(&wire),
            command: word,
        });
    }
    if global.talks() {
        println!("{}: {}", wire.name(), done(command));
    }
    Ok(())
}

fn done(command: Command) -> &'static str {
    match command {
        Command::Pause => "paused",
        Command::Resume => "resumed",
        Command::Stop => "stopped",
    }
}

/// One print the printer remembers, as `history --json` lists it.
#[derive(Serialize)]
struct Remembered {
    file: String,
    ending: &'static str,
    begin_unix_s: u64,
    end_unix_s: u64,
    layers_printed: u32,
    layer_count: Option<u32>,
}

impl Remembered {
    fn of(task: &Task) -> Self {
        Self {
            file: task.file_name().to_owned(),
            ending: ending_name(task.ending()),
            begin_unix_s: task.begin_s,
            end_unix_s: task.end_s,
            layers_printed: task.already_print_layer,
            layer_count: task
                .slice_information
                .as_ref()
                .map(|slice| slice.total_layer_numbers)
                .filter(|total| *total > 0),
        }
    }

    /// One line: the file, how it ended, how far it got, how long it ran and how long ago.
    fn line(&self, now_unix_s: u64) -> String {
        let layers = match self.layer_count {
            Some(count) => format!("{}/{count} layers", self.layers_printed),
            None => format!("{} layers", self.layers_printed),
        };
        let ran = seconds_between(self.begin_unix_s, self.end_unix_s);
        let ago = seconds_between(self.end_unix_s.max(self.begin_unix_s), now_unix_s);
        format!(
            "{}  {}  {layers}  {}, {} ago",
            self.file,
            self.ending,
            hours_minutes(ran),
            hours_minutes(ago)
        )
    }
}

fn seconds_between(from_s: u64, to_s: u64) -> u32 {
    u32::try_from(to_s.saturating_sub(from_s)).unwrap_or(u32::MAX)
}

fn ending_name(ending: Ending) -> &'static str {
    match ending {
        Ending::Finished => "finished",
        Ending::Failed => "failed",
        Ending::Stopped => "stopped",
        Ending::Other => "other",
    }
}

#[derive(Serialize)]
struct HistoryReport {
    #[serde(flatten)]
    machine: Machine,
    prints: Vec<Remembered>,
}

/// The last prints a board remembers; a Prusa machine keeps none its API serves.
pub fn history(args: &HistoryArgs, global: &GlobalArgs) -> Result<()> {
    let wire = args.machine.wire()?;
    let tasks = printer_link::history(&wire, args.count)
        .with_context(|| format!("cannot ask {} for its last prints", wire.name()))?;
    let prints: Vec<Remembered> = tasks.iter().map(Remembered::of).collect();
    if global.json {
        return json::print(&HistoryReport {
            machine: Machine::of(&wire),
            prints,
        });
    }
    if prints.is_empty() && global.talks() {
        println!("{} remembers no prints", wire.name());
    }
    let now = now_unix_s();
    for print in &prints {
        println!("{}", print.line(now));
    }
    Ok(())
}

#[derive(Serialize)]
struct CameraReport {
    #[serde(flatten)]
    machine: Machine,
    on: bool,
    url: Option<String>,
}

/// Turns the camera on or off; on, prints the RTSP address it streams to.
pub fn camera(args: &CameraArgs, global: &GlobalArgs) -> Result<()> {
    let wire = args.machine.wire()?;
    let on = args.switch == Switch::On;
    if matches!(wire, Wire::Prusa(_)) {
        bail!("{} serves no camera over PrusaLink", wire.name());
    }
    let url = printer_link::camera(&wire, on)
        .with_context(|| format!("cannot switch the camera of {}", wire.name()))?;
    if on && url.is_none() {
        bail!("{} has no camera", wire.name());
    }
    if global.json {
        return json::print(&CameraReport {
            machine: Machine::of(&wire),
            on,
            url,
        });
    }
    match url {
        Some(url) => println!("{url}"),
        None if global.talks() => println!("{}: camera off", wire.name()),
        None => {}
    }
    Ok(())
}

#[derive(Serialize)]
struct PictureReport {
    #[serde(flatten)]
    machine: Machine,
    output: String,
}

/// Writes the picture of the plate the printer keeps for the print under way.
pub fn picture(args: &PictureArgs, global: &GlobalArgs) -> Result<()> {
    let wire = args.machine.wire()?;
    let state = printer_link::state(&wire)
        .with_context(|| format!("cannot ask {} what it is doing", wire.name()))?;
    let bytes = printer_link::picture(&wire, &state, None)
        .with_context(|| format!("cannot fetch the picture from {}", wire.name()))?
        .with_context(|| format!("{} keeps no picture of a print under way", wire.name()))?;
    std::fs::write(&args.output, bytes)
        .with_context(|| format!("cannot write {}", args.output.display()))?;
    if global.json {
        return json::print(&PictureReport {
            machine: Machine::of(&wire),
            output: args.output.display().to_string(),
        });
    }
    if global.talks() {
        println!("Wrote {}", args.output.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remembered_print_reads_as_one_line() {
        let print = Remembered {
            file: "cube.goo".to_owned(),
            ending: "finished",
            begin_unix_s: 1_000,
            end_unix_s: 1_000 + 3_600,
            layers_printed: 400,
            layer_count: Some(400),
        };
        assert_eq!(
            print.line(1_000 + 3_600 + 120),
            "cube.goo  finished  400/400 layers  1 h 00 min, 2 min ago"
        );
    }

    #[test]
    fn a_print_with_no_end_or_layer_count_reads_as_the_window_shows_it() {
        let print = Remembered {
            file: "cube.goo".to_owned(),
            ending: "other",
            begin_unix_s: 1_000,
            end_unix_s: 0,
            layers_printed: 12,
            layer_count: None,
        };
        assert_eq!(
            print.line(1_000 + 120),
            "cube.goo  other  12 layers  0 min, 2 min ago",
            "ago is counted from the start when the board kept no end"
        );
    }

    #[test]
    fn a_clock_set_back_reads_as_no_time_rather_than_wrapping() {
        assert_eq!(seconds_between(10, 5), 0);
    }
}
