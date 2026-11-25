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
fn an_agent_at_work_is_listed_first_with_the_job_its_tool_names() {
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
            "jobs": ["bust-2", "cube-1"],
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
        agents[0].job,
        Some(Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 3,
            clean: Some(true)
        }),
        "the job the tool runs on, not the latest touched"
    );
    assert_eq!(agents[1].state, State::Idle);
    assert_eq!(
        agents[1].job.as_ref().map(|job| job.clean),
        Some(Some(false))
    );
}

#[test]
fn an_agent_ended_silent_or_with_no_model_in_hand_is_not_listed() {
    let dir = scratch("unlisted");
    job(&dir, "cube-1", "/m/cube.stl", 1, "ok");
    let said = |seen: u64, extra: serde_json::Value| {
        let mut said = json!({"started_s": 1, "seen_s": seen, "jobs": ["cube-1"]});
        said.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        said
    };
    agent(&dir, "1-1.json", &said(900, json!({})));
    agent(&dir, "2-2.json", &said(990, json!({"ended_s": 990})));
    agent(&dir, "3-3.json", &said(990, json!({"jobs": ["gone-3"]})));
    agent(
        &dir,
        "4-4.json",
        &said(
            990,
            json!({"jobs": [], "working": {"tool": "list_profiles", "since_s": 1}}),
        ),
    );
    agent(
        &dir,
        "5-5.json",
        &said(
            990,
            json!({"jobs": [], "working": {"tool": "start_job", "since_s": 1}}),
        ),
    );
    fs::write(dir.join("agents").join("6-6.json.part"), "{").expect("writable");
    let agents = listed(&dir, 1000);
    assert_eq!(agents.len(), 1, "{agents:?}");
    assert!(agents[0].opening(), "one opening a model is listed");
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
