//! Check's automatic answers as flags: a `check` step run last, as Fix all runs it, so a run
//! with them goes through the recipe's steps (ADR 0459).

use anyhow::Result;
use core_recipe::{Condition, Retry, Setting, Step, StepKind};

/// How many times islands are supported and measured again, as Fix all does.
const ISLAND_PASSES: u8 = 3;

/// What the check of the plate answers once the rest is done.
#[derive(Debug, Clone, Default, clap::Args)]
#[command(next_help_heading = "Fixing what the check finds")]
pub struct FixArgs {
    /// Stand a support under every island the check finds, checking again up to three times.
    #[arg(long)]
    pub support_islands: bool,

    /// Drill a vent into every suction cup the check finds, through its thinnest wall.
    #[arg(long)]
    pub drill_cups: bool,

    /// Stand supports under every weak neck the check finds, thick enough to hold it.
    #[arg(long)]
    pub hold_necks: bool,
}

impl FixArgs {
    /// Whether any of Check's answers is asked for.
    pub fn asked(&self) -> bool {
        self.support_islands || self.drill_cups || self.hold_necks
    }

    /// The `check` step these flags ask for, `None` for none.
    pub fn step(&self) -> Result<Option<Step>> {
        if !self.asked() {
            return Ok(None);
        }
        let mut set = core_recipe::Settings::new();
        set.insert("drill_cups".into(), Setting::Bool(self.drill_cups));
        set.insert("hold_levers".into(), Setting::Bool(self.hold_necks));
        let retry = match self.support_islands {
            true => Some(Retry {
                while_: Condition::parse("islands > 0")?,
                max: ISLAND_PASSES,
            }),
            false => None,
        };
        Ok(Some(Step {
            kind: StepKind::Check,
            enabled: true,
            when: None,
            set,
            choose: Vec::new(),
            on_fail: Default::default(),
            message: None,
            stop_if: None,
            retry,
        }))
    }
}
