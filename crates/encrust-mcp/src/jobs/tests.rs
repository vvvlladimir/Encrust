use std::path::PathBuf;

use serde_json::json;

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
        prepare: false,
        repair: None,
        pose: None,
        hollow: None,
        drains: None,
        supports: None,
        mend: None,
        modelling: None,
        relief: None,
        slicing: None,
    }
}

#[test]
fn a_job_is_named_after_its_model_and_starts_at_revision_one() {
    let jobs = scratch("start");
    let job = Jobs::name_for(&recipe());
    assert!(job.starts_with("my-bust--final-"), "named {job}");
    let first = jobs
        .commit(&job, recipe(), &Value::Null)
        .expect("the root is writable");
    assert_eq!(first.number, 1);
}

#[test]
fn each_commit_is_a_new_revision_and_the_old_one_stays_readable() {
    let jobs = scratch("revisions");
    let job = Jobs::name_for(&recipe());
    jobs.commit(&job, recipe(), &Value::Null)
        .expect("the root is writable");
    let mut edited = recipe();
    edited.supports = Some(Supports {
        profile: "auto".into(),
        cavity: false,
        ..Default::default()
    });
    let second = jobs
        .commit(&job, edited.clone(), &Value::Null)
        .expect("the job exists");
    assert_eq!(second.number, 2);
    assert_eq!(jobs.open(&job, None).expect("latest").recipe, edited);
    assert_eq!(jobs.open(&job, Some(1)).expect("first").recipe, recipe());
}

#[test]
fn each_revision_keeps_the_report_of_its_check_beside_its_recipe() {
    let jobs = scratch("report");
    let job = Jobs::name_for(&recipe());
    let report = json!({ "status": "ok" });
    let first = jobs
        .commit(&job, recipe(), &report)
        .expect("the root is writable");
    let kept = fs::read_to_string(jobs.root.join(&job).join(format!("{}.json", first.number)))
        .expect("the report is written beside the recipe");
    assert_eq!(serde_json::from_str::<Value>(&kept).expect("JSON"), report);
}

#[test]
fn a_job_name_cannot_reach_outside_the_root() {
    let jobs = scratch("escape");
    let outside = std::env::temp_dir().join("encrust-mcp-outside");
    let absolute = outside.display().to_string();
    for name in ["../etc", "a/b", "", ".", absolute.as_str()] {
        assert!(jobs.open(name, None).is_err(), "{name:?} was opened");
        assert!(
            jobs.open(name, Some(1)).is_err(),
            "{name:?} revision 1 was opened"
        );
        assert!(
            jobs.commit(name, recipe(), &Value::Null).is_err(),
            "{name:?} was written"
        );
    }
    assert!(!outside.exists());
}

#[test]
fn a_job_that_was_never_started_has_no_latest_revision() {
    let jobs = scratch("never");
    let refused = jobs
        .open("no-such-job", None)
        .expect_err("nothing was started");
    assert!(
        refused.to_string().contains("no-such-job"),
        "said {refused}"
    );
}
