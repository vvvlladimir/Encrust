use std::fs;

use serde_json::json;

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("encrust-app-agents-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("agents")).expect("writable");
    fs::create_dir_all(dir.join("mcp-jobs")).expect("writable");
    dir
}

fn agent(dir: &Path, name: &str, said: &serde_json::Value) {
    fs::write(dir.join("agents").join(name), said.to_string()).expect("writable");
}

fn job(dir: &Path, id: &str, model: &str, revisions: u32, status: &str) {
    let job = dir.join("mcp-jobs").join(id);
    fs::create_dir_all(&job).expect("writable");
    for revision in 1..=revisions {
        fs::write(job.join(format!("{revision}.toml")), "").expect("writable");
        let flags = json!([model, "--printer=p", "--resin=r"]).to_string();
        fs::write(job.join(format!("{revision}.args.json")), flags).expect("writable");
        fs::write(
            job.join(format!("{revision}.recipe.toml")),
            "name = \"x\"\n",
        )
        .expect("writable");
    }
    let report = json!({"status": status}).to_string();
    fs::write(job.join(format!("{revisions}.json")), report).expect("writable");
}

fn listed(dir: &Path, now: u64) -> Vec<Agent> {
    scan(&dir.join("agents"), &dir.join("mcp-jobs"), now)
}

#[test]
fn an_agent_at_work_is_listed_first_with_its_jobs_latest_revision_and_verdict() {
    let dir = scratch("working");
    job(&dir, "cube-1", "/m/cube.stl", 3, "ok");
    job(&dir, "bust-2", "/m/bust.stl", 1, "unclean");
    agent(
        &dir,
        "1-100.json",
        &json!({
            "client": "claude-ai", "started_s": 100, "seen_s": 990, "jobs": ["bust-2"],
        }),
    );
    agent(
        &dir,
        "2-200.json",
        &json!({
            "client": "claude-code", "started_s": 200, "seen_s": 980,
            "working": {"tool": "check", "job": "cube-1", "since_s": 970},
            "jobs": ["cube-1", "gone-3"],
        }),
    );
    let agents = listed(&dir, 1000);
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0].client.as_deref(), Some("claude-code"));
    assert_eq!(
        agents[0].state,
        State::Working {
            tool: "check".into(),
            job: Some("cube-1".into()),
            since_s: 970
        }
    );
    assert_eq!(
        agents[0].jobs,
        [Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 3,
            clean: Some(true)
        }],
        "a job whose folder is gone is not listed"
    );
    assert_eq!(agents[1].state, State::Idle);
    assert_eq!(agents[1].jobs[0].clean, Some(false));
}

#[test]
fn an_agent_silent_a_minute_has_ended_and_one_ended_with_no_job_is_not_listed() {
    let dir = scratch("ended");
    job(&dir, "cube-1", "/m/cube.stl", 1, "ok");
    agent(
        &dir,
        "1-1.json",
        &json!({
            "started_s": 1, "seen_s": 900, "working": {"tool": "check", "since_s": 1}, "jobs": ["cube-1"],
        }),
    );
    agent(
        &dir,
        "2-2.json",
        &json!({"started_s": 2, "seen_s": 990, "ended_s": 990, "jobs": []}),
    );
    fs::write(dir.join("agents").join("3-3.json.part"), "{").expect("writable");
    let agents = listed(&dir, 1000);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].state, State::Ended);
    assert_eq!(agents[0].client, None);
}

#[test]
fn a_project_job_is_told_apart_and_no_folder_lists_nothing() {
    let project = Job {
        id: "p".into(),
        model: "/m/plate.encrust".into(),
        revision: 1,
        clean: None,
    };
    assert!(project.is_project());
    assert_eq!(project.name(), "plate.encrust");
    assert!(scan(Path::new("/no/such/agents"), Path::new("/no/jobs"), 0).is_empty());
}

#[test]
fn the_jobs_of_the_last_day_no_agent_names_are_listed_as_one_entry_after_the_agents() {
    let dir = scratch("unclaimed");
    job(&dir, "cube-1", "/m/cube.stl", 2, "ok");
    job(&dir, "bust-2", "/m/bust.stl", 1, "ok");
    agent(
        &dir,
        "1-1.json",
        &json!({"started_s": 1, "seen_s": now_s(), "jobs": ["bust-2"]}),
    );
    let agents = listed(&dir, now_s());
    assert_eq!(agents.len(), 2);
    assert!(!agents[0].unclaimed);
    assert!(agents[1].unclaimed && agents[1].state == State::Ended);
    let ids: Vec<&str> = agents[1].jobs.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(
        ids,
        ["cube-1"],
        "a job an agent names is listed under it alone"
    );

    let fs_gone = std::fs::remove_dir_all(dir.join("agents"));
    assert!(fs_gone.is_ok());
    assert_eq!(
        listed(&dir, now_s()).len(),
        1,
        "no agents' folder at all still lists them"
    );
    let two_days = now_s() + 2 * 24 * 60 * 60;
    assert!(
        listed(&dir, two_days).is_empty(),
        "a job a day old is no longer listed"
    );
}
