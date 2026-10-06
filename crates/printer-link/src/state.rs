use serde::Serialize;

/// What a printer said it is doing, the same shape whichever protocol said it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct State {
    /// One word for the machine's state, in its own protocol's terms.
    pub state: String,
    /// What the print is doing at this moment, when the protocol says.
    pub stage: Option<String>,
    /// The file being printed, when the protocol names it.
    pub file: Option<String>,
    /// Share of the print done, 0 to 1.
    pub progress: Option<f32>,
    pub remaining_s: Option<u64>,
    /// Why the last print stopped, when the printer says.
    pub error: Option<String>,
}

impl State {
    pub(crate) fn of_board(status: &net_sdcp::Status) -> Self {
        let info = &status.print_info;
        let stage = info.stage();
        Self {
            state: status.machine().label().to_owned(),
            stage: (stage != net_sdcp::Stage::Idle).then(|| stage.label().to_owned()),
            file: (!info.filename.is_empty()).then(|| info.filename.clone()),
            progress: info.fraction(),
            remaining_s: None,
            error: info.error().map(str::to_owned),
        }
    }

    pub(crate) fn of_prusa(status: net_prusalink::Status) -> Self {
        Self {
            state: status.printer.state.to_lowercase(),
            stage: None,
            file: None,
            progress: status.job.as_ref().map(|job| job.progress / 100.0),
            remaining_s: status.job.and_then(|job| job.time_remaining),
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_printing_names_its_file_and_layer_share() {
        let status: net_sdcp::Status = serde_json::from_str(
            r#"{"CurrentStatus":[1],"PrintInfo":{"Status":3,"CurrentLayer":50,
                "TotalLayer":200,"Filename":"cube.goo","ErrorNumber":0}}"#,
        )
        .expect("a version 3 status report");
        let state = State::of_board(&status);
        assert_eq!(state.state, "printing");
        assert_eq!(
            state.stage.as_deref(),
            Some("exposing"),
            "PrintInfo.Status 3"
        );
        assert_eq!(state.file.as_deref(), Some("cube.goo"));
        assert_eq!(state.progress, Some(0.25), "50 of 200 layers");
        assert_eq!(state.error, None);
    }

    #[test]
    fn a_prusa_machine_reports_percent_which_becomes_a_share() {
        let status: net_prusalink::Status = serde_json::from_str(
            r#"{"printer":{"state":"PRINTING"},
                "job":{"id":1,"progress":42.0,"time_remaining":520,"time_printing":5}}"#,
        )
        .expect("the specification's example status");
        let state = State::of_prusa(status);
        assert_eq!(state.state, "printing");
        assert_eq!(state.progress, Some(0.42), "42 percent");
        assert_eq!(state.remaining_s, Some(520));
    }
}
