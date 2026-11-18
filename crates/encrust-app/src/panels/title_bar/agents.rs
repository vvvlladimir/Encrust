//! The agents' part of the print chip and the list it opens: each chat client's agent with
//! its latest jobs, a press showing one on the plate (ADR 0465, `ui-screens.md`, Top bar).

use crate::agents::{Agent, Agents, Asked, Job, State};
use crate::files::Handed;
use crate::i18n::tr;
use crate::panels::Window;
use crate::ui::{
    Aside, ChipPart, ChooserRow, Lead, chooser_group, chooser_row, hairline, hint, icon, switch,
    theme,
};

/// The most agents that have ended the list keeps.
const MOST_ENDED: usize = 3;

/// The agents' part: the robot, lit while one works, and how many there are now. None in a
/// browser, where no agent can reach the window.
pub(super) fn part(ui: &egui::Ui, agents: &Agents) -> Option<ChipPart> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    let colors = theme::colors();
    let (count, working) = agents.at_work();
    let text = if count > 0 {
        count.to_string()
    } else {
        String::new()
    };
    let part = ChipPart::new(
        ui,
        icon::AGENTS,
        &text,
        colors.text_high,
        icon::CARET_DOWN,
        f32::INFINITY,
    );
    Some(if working {
        part.glyph_tint(colors.accent)
    } else {
        part
    })
}

/// The part's name for a screen reader.
pub(super) fn name(agents: &Agents) -> String {
    tr!("chip-agents", count = agents.at_work().0)
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
    let ended = agents
        .list
        .iter()
        .filter(|agent| agent.state == State::Ended);
    let shown = agents.list.len() - ended.count().saturating_sub(MOST_ENDED);
    for (agent, client) in agents.list.iter().zip(names).take(shown) {
        let group = match agent.state {
            State::Working { .. } => tr!("agents-working", client = client),
            State::Idle => tr!("agents-idle", client = client),
            State::Ended => tr!("agents-ended", client = client),
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
}
