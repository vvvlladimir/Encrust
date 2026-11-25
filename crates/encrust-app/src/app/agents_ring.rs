//! The agents' ring and the list it opens, as the window draws them.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::states::{SETTLE_FRAMES, app, drawn};
use crate::agents::{Agent, Job, State};

/// BUG: the list closed on any click inside it, so *Follow agents* could not be flipped
/// without the list going away.
#[test]
fn flipping_follow_agents_leaves_the_list_open() {
    let mut app = app(false);
    app.machine.agents.list = vec![Agent {
        client: Some("claude-code".into()),
        started_s: 0,
        state: State::Idle,
        job: Some(Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 2,
            clean: Some(true),
        }),
    }];
    let mut harness = drawn(Harness::builder().with_size(egui::vec2(1280.0, 800.0)), app);
    harness.get_by_label("Agents").click();
    harness.run_steps(SETTLE_FRAMES);
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
