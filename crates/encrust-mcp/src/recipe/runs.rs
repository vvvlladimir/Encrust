//! A recipe whose steps name some of the job's objects, staged as several runs of the command
//! line: the model first, then each group of steps on the plate the run before wrote, by
//! `--only` (ADR 0458).

use anyhow::{Result, bail};

use super::Recipe;

/// Stands for the plate the run before wrote, which the server puts in its place.
pub const PREVIOUS: &str = "{previous plate}";

impl Recipe {
    /// Whether it runs as several runs: a step names some of the job's objects rather than
    /// every one, or Check's answers follow a modelling that makes pieces, which no recipe takes.
    pub fn scoped(&self) -> bool {
        let named = |objects: &[u64]| !objects.is_empty();
        let answered = self.fixes.as_ref().is_some_and(super::Fixes::asked);
        self.modelling.as_ref().is_some_and(|m| named(&m.objects))
            || self.hollow.as_ref().is_some_and(|h| named(&h.objects))
            || self.drains.as_ref().is_some_and(|d| named(&d.objects))
            || self.supports.as_ref().is_some_and(|s| named(&s.objects))
            || !self.more_modelling.is_empty()
            || !self.more_hollow.is_empty()
            || !self.more_drains.is_empty()
            || !self.more_supports.is_empty()
            || self.fixes.as_ref().is_some_and(|f| named(&f.objects))
            || answered && self.makes_pieces()
    }

    /// The runs that stage this recipe, each its flags after the subcommand: one where every
    /// step shapes every object, else the model with the steps that do, then a run for each
    /// group of steps naming the same objects, in the command line's order.
    pub fn runs(&self) -> Result<Vec<Vec<String>>> {
        if !self.scoped() {
            return Ok(vec![self.flags()]);
        }
        if self.prepare || self.steps_by.is_some() {
            bail!(
                "the autopilot and a recipe prepare every object, and a step here names some \
                 (its `objects`, or points handed to the object they lie at): take those back \
                 first"
            );
        }
        let mut groups: Vec<(Vec<u64>, Vec<String>)> = Vec::new();
        for (objects, flags) in self.steps() {
            match groups.last_mut() {
                Some((named, together)) if *named == objects => together.extend(flags),
                _ => groups.push((objects, flags)),
            }
        }
        let base = Recipe {
            pose: self.pose.clone().filter(|_| !self.models_some()),
            modelling: None,
            more_modelling: Vec::new(),
            hollow: None,
            more_hollow: Vec::new(),
            drains: None,
            more_drains: Vec::new(),
            supports: None,
            more_supports: Vec::new(),
            fixes: None,
            ..self.clone()
        };
        let mut runs = vec![base.flags()];
        let mut groups = groups.into_iter().peekable();
        if let Some((_, flags)) = groups.next_if(|(objects, _)| objects.is_empty()) {
            runs[0].extend(flags);
        }
        for (objects, flags) in groups {
            let mut run = vec![PREVIOUS.to_owned()];
            run.extend(self.machine_flags());
            if !objects.is_empty() {
                let ids: Vec<String> = objects.iter().map(u64::to_string).collect();
                run.push(format!("--only={}", ids.join(",")));
            }
            run.extend(flags);
            runs.push(run);
        }
        if let Some(fixes) = self.fixes.as_ref().filter(|fixes| fixes.asked()) {
            runs.push(self.answers_run(fixes));
        }
        let arranged = self.modellings().any(|m| m.arrange);
        if arranged
            && let Some(last) = runs.last_mut()
            && !last.iter().any(|flag| flag == "--arrange")
        {
            last.push("--arrange".into());
        }
        Ok(runs)
    }

    /// Check's answers on the plate the run before wrote, to the objects they name or every one
    /// (ADR 0465).
    fn answers_run(&self, fixes: &super::Fixes) -> Vec<String> {
        let mut run = vec![PREVIOUS.to_owned()];
        run.extend(self.machine_flags());
        if !fixes.objects.is_empty() {
            let ids: Vec<String> = fixes.objects.iter().map(u64::to_string).collect();
            run.push(format!("--only={}", ids.join(",")));
        }
        fixes.push_flags(&mut run);
        run
    }

    /// Whether the modelling names some objects, so the pose runs on its pieces after it
    /// (ADR 0458).
    fn models_some(&self) -> bool {
        self.modellings().any(|m| !m.objects.is_empty())
    }

    /// Each step's objects and flags, in the command line's order.
    fn steps(&self) -> Vec<(Vec<u64>, Vec<String>)> {
        let mut steps = Vec::new();
        for modelling in self.modellings() {
            let mut flags = Vec::new();
            modelling.push_flags(&mut flags);
            steps.push((modelling.objects.clone(), flags));
        }
        if let Some(pose) = self.pose.as_ref().filter(|_| self.models_some()) {
            let mut flags = Vec::new();
            pose.push_flags(&mut flags);
            steps.push((Vec::new(), flags));
        }
        for hollow in self.hollow.iter().chain(&self.more_hollow) {
            let mut flags = Vec::new();
            hollow.push_flags(&mut flags);
            steps.push((hollow.objects.clone(), flags));
        }
        for drains in self.drains.iter().chain(&self.more_drains) {
            let mut flags = Vec::new();
            drains.push_flags(&mut flags);
            steps.push((drains.objects.clone(), flags));
        }
        for supports in self.supports.iter().chain(&self.more_supports) {
            let mut flags = Vec::new();
            supports.push_flags(&mut flags);
            steps.push((supports.objects.clone(), flags));
        }
        steps.retain(|(_, flags)| !flags.is_empty());
        steps
    }
}
