//! The agents' ring and the list it opens, as the window draws them.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::states::{SETTLE_FRAMES, app, drawn};
use crate::agents::{Agent, Job, State};

fn waiting_on_a_cube() -> Agent {
    Agent {
        client: Some("code-agent".into()),
        started_s: 0,
        state: State::Idle,
        job: Some(Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 2,
            clean: Some(true),
        }),
    }
}

/// The window with the agents' list open over one agent's cube.
fn listed(agent: Agent) -> Harness<'static, crate::app::SlicerApp> {
    let mut app = app(false);
    app.machine.agents.list = vec![agent];
    let mut harness = drawn(Harness::builder().with_size(egui::vec2(1280.0, 800.0)), app);
    harness.get_by_label("Agents").click();
    harness.run_steps(SETTLE_FRAMES);
    harness
}

/// BUG: the list closed on any click inside it, so *Follow agents* could not be flipped
/// without the list going away.
#[test]
fn flipping_follow_agents_leaves_the_list_open() {
    let mut harness = listed(waiting_on_a_cube());
    harness.get_by_label("Follow agents").click();
    harness.run_steps(SETTLE_FRAMES);
    assert!(harness.state().machine.general.follow_agents);
    assert!(
        harness.query_by_label("Follow agents").is_some(),
        "the list stays open after the switch is flipped"
    );
}

#[test]
fn no_agent_with_a_model_in_hand_draws_no_ring() {
    let harness = drawn(
        Harness::builder().with_size(egui::vec2(1280.0, 800.0)),
        app(false),
    );
    assert!(harness.query_by_label("Agents").is_none());
}

#[test]
fn a_hidden_job_leaves_the_list_and_takes_the_ring_with_it() {
    let mut harness = listed(waiting_on_a_cube());
    harness
        .get_by_label("Hide from the list until the agent takes another model")
        .click();
    harness.run_steps(SETTLE_FRAMES);
    assert!(harness.state().machine.agents.list.is_empty());
    assert!(harness.query_by_label("Agents").is_none());
}

#[test]
fn a_job_is_deleted_by_a_second_press_and_never_while_its_agent_works() {
    let delete = "Delete this job and its revisions from the disk";
    let mut harness = listed(waiting_on_a_cube());
    harness.get_by_label(delete).click();
    // A few frames only: the harness runs to the next repaint asked, and arming asks one
    // when it times out.
    harness.run_steps(2);
    assert_eq!(harness.state().machine.agents.list.len(), 1, "armed only");
    harness.get_by_label(delete).click();
    harness.run_steps(SETTLE_FRAMES);
    assert!(harness.state().machine.agents.list.is_empty());

    let mut working = waiting_on_a_cube();
    working.state = State::Working {
        tool: "check".into(),
        job: Some("cube-1".into()),
        since_s: crate::agents::now_s(),
    };
    let harness = listed(working);
    assert!(harness.query_by_label(delete).is_none());
    assert!(
        harness
            .query_by_label("The agent is working on this job")
            .is_some(),
        "the bin says why it waits"
    );
}
