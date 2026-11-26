//! The search's best ways to stand a model laid side by side, each with what it prints and
//! drawn, for a client to choose by what the model is (ADR 0458).

use anyhow::{Context as _, Result, bail};
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

use super::{Context, Server};
use crate::jobs::Revision;
use crate::params::{ComparePoses, View};
use crate::recipe::{Goal, Pose, Recipe, Supports};

/// How many ways are compared when the client names no count.
const COUNT: u16 = 4;
/// Each picture's width and height when the client names none: small, for one a choice.
const SIZE_PX: u32 = 320;

/// What the client is told of the points a new pose took away, which were on the old one.
const DRAINS_CLEARED: &str =
    "the drain points were cleared: they were on the old pose; automatic drains stay";
const SUPPORT_POINTS_CLEARED: &str = "the support points, shown points and merges given by hand \
    were cleared: they were on the old pose; the supports stand again for the new one";
const BOTH_CLEARED: &str = "the drain and support points given by hand were cleared: they were \
    on the old pose; automatic drains stay and the supports stand again for the new one";

/// Stands `recipe`'s model at `pose`, taking away the points given on the old one; answers
/// what the client is told of them.
pub(super) fn turned(recipe: &mut Recipe, pose: Option<Pose>) -> Option<&'static str> {
    let cleared: Vec<bool> = (recipe.drains_mut())
        .map(|drains| !std::mem::take(&mut drains.at_mm).is_empty())
        .collect();
    let had_drains = cleared.contains(&true);
    let asked = recipe.supports.iter_mut().chain(&mut recipe.more_supports);
    let dropped: Vec<bool> = asked.map(Supports::drop_plate_points).collect();
    let had_points = dropped.contains(&true);
    recipe.pose = pose;
    match (had_drains, had_points) {
        (false, false) => None,
        (true, false) => Some(DRAINS_CLEARED),
        (false, true) => Some(SUPPORT_POINTS_CLEARED),
        (true, true) => Some(BOTH_CLEARED),
    }
}

impl Server {
    /// The ways the latest revision's model could stand, each with its numbers and pictures.
    pub(super) async fn poses(
        &self,
        compare: ComparePoses,
        ctx: &Context,
    ) -> Result<CallToolResult> {
        let latest = self.jobs.open(&compare.job, None)?;
        let pose = match compare.pose {
            Some(pose @ Pose::Search { .. }) => pose,
            Some(Pose::Rotate { .. }) => bail!("compare_poses compares the ways a search finds"),
            None => match &latest.recipe.pose {
                Some(pose @ Pose::Search { .. }) => pose.clone(),
                _ => Pose::search(Goal::Auto, None),
            },
        };
        let mut pose = pose.unstood();
        if let Pose::Search { pick, .. } = &mut pose {
            *pick = None;
        }
        self.seat_keep_clear(&mut pose, &latest)?;
        let mut recipe = latest.recipe.clone();
        recipe.pose = Some(pose.clone());
        for drains in recipe.drains_mut() {
            drains.at_mm.clear();
        }
        let count = compare.count.unwrap_or(COUNT).clamp(2, 8);
        // The search runs in the first run, before any step naming some objects (ADR 0479).
        let flags = recipe.runs()?.swap_remove(0);
        let mut args = vec!["estimate".to_owned()];
        args.extend(flags.iter().cloned());
        args.push(format!("--orient-choices={count}"));
        let outcome = self.watched(&args, ctx).await?;
        let choices = outcome.report["models"][0]["oriented"]["choices"].clone();
        let Some(listed) = choices.as_array().filter(|listed| !listed.is_empty()) else {
            bail!(
                "the search offered no way to stand the model: {}",
                outcome.report
            );
        };
        let mut content = vec![ContentBlock::text(
            json!({
                "job": latest.job,
                "seen_on_revision": latest.number,
                "pose": pose,
                "choices": choices,
                "take_one": "set_pose with this pose and `pick` set to the choice's number",
                "pictures": "each choice's views in turn, choices in order",
            })
            .to_string(),
        )];
        if compare.pictures.unwrap_or(true) {
            let views = compare.views.unwrap_or_else(|| vec![View::Iso]);
            for choice in listed {
                let pick = choice["pick"].as_u64().context("a choice has its number")?;
                let drawn = self
                    .drawn_pick(&flags, (&latest, pick), &views, compare.size_px, ctx)
                    .await?;
                content.extend(drawn);
            }
        }
        Ok(CallToolResult::success(content))
    }

    /// The pictures of the `pick`th way, as `render` draws a revision.
    async fn drawn_pick(
        &self,
        flags: &[String],
        (latest, pick): (&Revision, u64),
        views: &[View],
        size_px: Option<u32>,
        ctx: &Context,
    ) -> Result<Vec<ContentBlock>> {
        let dir = self.jobs.poses_dir(latest, pick)?;
        let names: Vec<&str> = views.iter().map(|view| view.name()).collect();
        let mut args = vec!["render".to_owned()];
        args.extend(flags.iter().cloned());
        args.push(format!("--orient-pick={pick}"));
        args.push(format!("--view={}", names.join(",")));
        args.push(format!(
            "--size={}",
            size_px.unwrap_or(SIZE_PX).clamp(64, 1024)
        ));
        args.push(format!("--output={}", dir.display()));
        let outcome = self.watched(&args, ctx).await?;
        if outcome.exit_code != 0 {
            bail!("drawing choice {pick} failed: {}", outcome.report);
        }
        super::reply::images(&outcome.report).await
    }

    /// Keeps with new keep-clear points the turn `latest` stood in and takes its supports'
    /// lift off them, so a new pose finds them; points seated before keep theirs.
    pub(super) fn seat_keep_clear(&self, pose: &mut Pose, latest: &Revision) -> Result<()> {
        let Pose::Search {
            keep_clear_at_mm,
            keep_clear_seen_deg,
            ..
        } = pose
        else {
            return Ok(());
        };
        if keep_clear_at_mm.is_empty() || keep_clear_seen_deg.is_some() {
            return Ok(());
        }
        let report = self.jobs.report(latest)?;
        let (seen, lifted_mm) = seen_on(&report, latest);
        *keep_clear_seen_deg = Some(seen);
        for at in keep_clear_at_mm.iter_mut() {
            at[2] -= lifted_mm;
        }
        Ok(())
    }
}

/// The turn the job's model stands in on a revision and how far its supports lift it.
fn seen_on(report: &Value, revision: &Revision) -> ([f32; 3], f32) {
    let model = &report["models"][0];
    let reported = model["oriented"]["rotate_deg"].as_array().and_then(|turn| {
        let turn: Vec<f32> = turn
            .iter()
            .filter_map(|deg| Some(deg.as_f64()? as f32))
            .collect();
        <[f32; 3]>::try_from(turn).ok()
    });
    let given = match &revision.recipe.pose {
        Some(Pose::Rotate { degrees }) => *degrees,
        _ => [0.0; 3],
    };
    let lifted_mm = model["supports"]["lifted_mm"].as_f64().unwrap_or(0.0) as f32;
    (reported.unwrap_or(given), lifted_mm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::Recipe;

    fn revision(pose: Option<Pose>) -> Revision {
        Revision {
            job: "j".into(),
            number: 1,
            recipe: Recipe {
                pose,
                ..Recipe::bare("/m.stl".into(), "p".into(), "r".into())
            },
            steps: None,
        }
    }

    #[test]
    fn the_turn_seen_is_the_search_s_where_it_turned_the_model() {
        let report = json!({"models": [{"oriented": {"rotate_deg": [0.0, -90.0, 0.0]},
                                        "supports": {"lifted_mm": 5.0}}]});
        assert_eq!(seen_on(&report, &revision(None)), ([0.0, -90.0, 0.0], 5.0));
    }

    #[test]
    fn the_turn_seen_is_the_one_given_where_no_search_turned_it() {
        let turned = revision(Some(Pose::Rotate {
            degrees: [30.0, 0.0, 0.0],
        }));
        assert_eq!(
            seen_on(&json!({"models": [{}]}), &turned),
            ([30.0, 0.0, 0.0], 0.0)
        );
        assert_eq!(seen_on(&json!({}), &revision(None)), ([0.0; 3], 0.0));
    }
}
