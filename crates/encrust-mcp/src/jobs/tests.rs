use std::path::PathBuf;

use super::*;
use crate::recipe::Supports;

fn scratch(name: &str) -> Jobs {
    let root = std::env::temp_dir().join(format!("encrust-mcp-jobs-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    Jobs::new(root)
}

fn recipe() -> Recipe {
    Recipe {
        model: PathBuf::from("/models/My Bust (final).stl"),
        printer: "elegoo-mars-4-ultra".into(),
        resin: "my-grey".into(),
        repair: None,
        pose: None,
        hollow: None,
        drains: None,
        supports: None,
    }
}

#[test]
fn a_job_starts_at_revision_one_named_after_its_model() {
    let jobs = scratch("start");
    let first = jobs.start(recipe()).expect("the root is writable");
    assert_eq!(first.number, 1);
    assert!(
        first.job.starts_with("my-bust--final-"),
        "named {}",
        first.job
    );
    assert!(is_job_name(&first.job));
}

#[test]
fn each_edit_is_a_new_revision_and_the_old_one_stays_readable() {
    let jobs = scratch("revisions");
    let first = jobs.start(recipe()).expect("the root is writable");
    let mut edited = first.recipe.clone();
    edited.supports = Some(Supports {
        profile: "auto".into(),
        cavity: false,
    });
    let second = jobs
        .next(&first.job, edited.clone())
        .expect("the job exists");
    assert_eq!(second.number, 2);
    assert_eq!(jobs.open(&first.job, None).expect("latest").recipe, edited);
    assert_eq!(
        jobs.open(&first.job, Some(1)).expect("first").recipe,
        recipe()
    );
}

#[test]
fn a_report_is_kept_beside_its_revision() {
    let jobs = scratch("report");
    let first = jobs.start(recipe()).expect("the root is writable");
    let report = serde_json::json!({"status": "ok"});
    jobs.keep_report(&first, &report).expect("the job exists");
    let kept = fs::read_to_string(jobs.dir(&first.job).join("1.json")).expect("it was kept");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&kept).expect("JSON"),
        report
    );
}

#[test]
fn a_job_name_cannot_reach_outside_the_root() {
    let jobs = scratch("escape");
    for name in ["../etc", "a/b", "", "."] {
        let refused = jobs.open(name, None);
        assert!(refused.is_err(), "{name:?} was opened");
    }
}
