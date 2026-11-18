//! The agents' ring beside the print chip and the list it opens: each chat client's agent with
//! its latest jobs, a press showing one on the plate (ADR 0465, `ui-screens.md`, Top bar).

use crate::agents::{Agent, Agents, Asked, Job, State};
use crate::files::Handed;
use crate::i18n::tr;
use crate::panels::Window;
use egui::{Sense, vec2};

use crate::ui::theme::layout::printer_card_ring::{GAP, SIZE};
use crate::ui::{
    Aside, ChooserRow, Lead, RingLook, chooser_frame, chooser_group, chooser_row, duration,
    focus_ring, hairline, hint, icon, paint_ring, ring_badge, switch, theme,
};

/// The most agents that have ended the list keeps.
const MOST_ENDED: usize = 3;

/// The agents' ring, beside the printers': there while an agent works or waits for its chat,
/// spinning while one works, the count at its corner past one; a press opens the list.
/// Answers the width it took; none in a browser, which no agent reaches.
pub(super) fn ring(ui: &mut egui::Ui, window: &mut Window) -> f32 {
    if cfg!(target_arch = "wasm32") {
        return 0.0;
    }
    let (count, working) = window.machine.agents.at_work();
    if count == 0 {
        return 0.0;
    }
    let colors = theme::colors();
    let look = RingLook {
        glyph: icon::AGENTS,
        glyph_tint: colors.text_mid,
        track: colors.line,
        arc: colors.accent,
        share: None,
        spinning: working,
    };
    let (rect, response) = ui.allocate_exact_size(vec2(SIZE, SIZE), Sense::click());
    paint_ring(
        ui,
        rect.center(),
        SIZE / 2.0 - 2.0,
        &look,
        response.hovered(),
    );
    if count > 1 {
        ring_badge(ui, rect, count);
    }
    focus_ring(ui, &response, theme::R_CONTROL);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tr!("agents-title"))
    });
    let response = response.on_hover_text(hover_lines(&window.machine.agents));
    chooser(&response).show(|ui| menu(ui, window));
    SIZE + GAP
}

/// What the ring says on hover: each agent there now, what it does and for how long.
fn hover_lines(agents: &Agents) -> String {
    let now = crate::agents::now_s();
    let names = client_names(&agents.list);
    let lines: Vec<String> = agents
        .list
        .iter()
        .zip(names)
        .filter(|(agent, _)| agent.state != State::Ended)
        .map(|(agent, client)| match &agent.state {
            State::Working { tool, job, since_s } => {
                let agent_name = tr!("agents-working", client = client);
                let time =
                    duration(u32::try_from(now.saturating_sub(*since_s)).unwrap_or(u32::MAX));
                let on = job.as_ref().and_then(|id| agents_job(agent, id));
                match on {
                    Some(job) => tr!(
                        "agents-ring-line",
                        agent = agent_name,
                        model = job.name(),
                        detail = detail(job, Some(tool)),
                        time = time
                    ),
                    None => tr!("agents-ring-busy", agent = agent_name, time = time),
                }
            }
            _ => tr!("agents-idle", client = client),
        })
        .collect();
    lines.join("\n")
}

fn agents_job<'a>(agent: &'a Agent, id: &str) -> Option<&'a Job> {
    agent.jobs.iter().find(|job| job.id == id)
}

/// The list the ring opens, hanging under it.
fn chooser(ring: &egui::Response) -> egui::Popup<'_> {
    crate::ui::popup_menu(ring)
        .frame(chooser_frame())
        .width(theme::CHOOSER_W)
        .gap(theme::CHOOSER_GAP)
}

/// The list: each agent under its name with its jobs, then whether the window follows them.
pub(super) fn menu(ui: &mut egui::Ui, window: &mut Window) {
    let agents = &window.machine.agents;
    let mut picked: Option<Job> = None;
    if agents.list.is_empty() {
        chooser_group(ui, &tr!("agents-title"));
        hint(ui, &tr!("agents-none"));
    }
    let names = client_names(&agents.list);
    let mut ended = 0;
    for (agent, client) in agents.list.iter().zip(names) {
        let group = match agent.state {
            _ if agent.unclaimed => tr!("agents-earlier"),
            State::Working { .. } => tr!("agents-working", client = client),
            State::Idle => tr!("agents-idle", client = client),
            State::Ended => {
                ended += 1;
                if ended > MOST_ENDED {
                    continue;
                }
                tr!("agents-ended", client = client)
            }
        };
        chooser_group(ui, &group);
        if agent.jobs.is_empty() {
            hint(ui, &tr!("agents-no-job"));
        }
        for job in &agent.jobs {
            if job_row(ui, agent, job, agents.on_plate.contains(&job.id)) {
                picked = Some(job.clone());
            }
        }
    }
    hairline(ui);
    let general = &mut window.machine.general;
    switch(ui, &mut general.follow_agents, &tr!("agents-follow"))
        .on_hover_text(tr!("agents-follow-tooltip"));
    if let Some(job) = picked {
        ui.close();
        if job.is_project() {
            crate::app::open_by_what_it_is(window, Handed::Path(job.model));
        } else {
            window.machine.agents.asked.push(Asked {
                job: job.id,
                revision: job.revision,
            });
        }
    }
}

/// One job's row: the model, its revision and what is said of it, the job on the plate
/// ticked. Answers whether it was pressed.
fn job_row(ui: &mut egui::Ui, agent: &Agent, job: &Job, on_plate: bool) -> bool {
    let working = match &agent.state {
        State::Working {
            tool,
            job: Some(on),
            ..
        } if *on == job.id => Some(tool.as_str()),
        _ => None,
    };
    let detail = detail(job, working);
    let name = job.name();
    let row = ChooserRow {
        lead: Lead::Glyph(icon::MODEL),
        name: &name,
        detail: &detail,
        aside: working.map(|_| Aside::Dot(theme::colors().accent)),
        chosen: on_plate,
    };
    let tooltip = if job.is_project() {
        tr!("agents-open-project-tooltip")
    } else {
        tr!("agents-show-tooltip")
    };
    chooser_row(ui, &row).on_hover_text(tooltip).clicked()
}

/// What a job's row says under its name: what its agent does to it now, else the check's
/// verdict on its latest revision.
fn detail(job: &Job, working: Option<&str>) -> String {
    let revision = job.revision;
    match working {
        Some("start_job") => tr!("agents-starting", revision = revision),
        Some("check") => tr!("agents-checking", revision = revision),
        Some("render" | "compare_poses") => tr!("agents-drawing", revision = revision),
        Some("write_file") => tr!("agents-writing", revision = revision),
        Some("send_to_printer") => tr!("agents-sending", revision = revision),
        Some("report_print") => tr!("agents-reading-print", revision = revision),
        Some(_) => tr!("agents-changing", revision = revision),
        None if job.is_project() => tr!("agents-revision-project", revision = revision),
        None => match job.clean {
            Some(true) => tr!("agents-revision-clean", revision = revision),
            Some(false) => tr!("agents-revision-problems", revision = revision),
            None => tr!("agents-revision", revision = revision),
        },
    }
}

/// Each agent's client by the name its maker gives it, a second of one client numbered.
fn client_names(list: &[Agent]) -> Vec<String> {
    let names: Vec<String> = list
        .iter()
        .map(|agent| client_name(agent.client.as_deref()))
        .collect();
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let before = names[..index].iter().filter(|other| *other == name).count();
            if before == 0 {
                name.clone()
            } else {
                tr!(
                    "agents-numbered",
                    client = name.clone(),
                    number = before + 1
                )
            }
        })
        .collect()
}

/// A client's own name for itself as its maker writes it; one it did not give is an agent.
fn client_name(client: Option<&str>) -> String {
    match client {
        Some("claude-code") => "Claude Code".to_owned(),
        Some("claude-ai") => "Claude Desktop".to_owned(),
        Some(name) if name.starts_with("codex") => "Codex".to_owned(),
        Some(name) if name.to_ascii_lowercase().contains("cursor") => "Cursor".to_owned(),
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => tr!("agents-unnamed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(client: Option<&str>) -> Agent {
        Agent {
            client: client.map(str::to_owned),
            started_s: 0,
            state: State::Idle,
            jobs: Vec::new(),
            unclaimed: false,
        }
    }

    #[test]
    fn a_client_is_named_as_its_maker_names_it_and_a_second_one_numbered() {
        let list = [
            agent(Some("claude-code")),
            agent(Some("claude-ai")),
            agent(Some("claude-code")),
            agent(Some("my-bench")),
        ];
        assert_eq!(
            client_names(&list),
            ["Claude Code", "Claude Desktop", "Claude Code 2", "my-bench"]
        );
    }

    #[test]
    fn a_job_says_what_its_agent_does_to_it_else_what_its_check_found() {
        let job = Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 3,
            clean: Some(false),
        };
        assert_eq!(detail(&job, Some("check")), "Revision 3 · checking");
        assert_eq!(
            detail(&job, Some("set_hollow")),
            "Revision 3 · changing the model"
        );
        assert_eq!(detail(&job, None), "Revision 3 · the check found problems");
    }

    #[test]
    fn the_ring_says_what_each_agent_there_does_and_leaves_out_the_ended() {
        let mut working = agent(Some("claude-code"));
        working.state = State::Working {
            tool: "check".into(),
            job: Some("cube-1".into()),
            since_s: crate::agents::now_s(),
        };
        working.jobs = vec![Job {
            id: "cube-1".into(),
            model: "/m/cube.stl".into(),
            revision: 3,
            clean: None,
        }];
        let mut ended = agent(Some("claude-ai"));
        ended.state = State::Ended;
        let mut agents = Agents::default();
        agents.list = vec![working, agent(None), ended];
        let said = hover_lines(&agents);
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), 2, "{said}");
        assert!(
            lines[0].starts_with("Claude Code · working: cube.stl, Revision 3 · checking, "),
            "{said}"
        );
        assert_eq!(lines[1], "Agent · waiting");
    }
}
