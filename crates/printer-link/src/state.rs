use serde::Serialize;

/// What a printer said it is doing, the same shape whichever protocol said it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct State {
    /// One word for the machine's state, in its own protocol's terms.
    pub state: String,
    /// Whether the machine is in the middle of something and will not start a new print.
    pub busy: bool,
    /// What the print is doing at this moment, when the protocol says.
    pub stage: Option<String>,
    /// The file being printed, when the protocol names it.
    pub file: Option<String>,
    /// Share of the print done, 0 to 1.
    pub progress: Option<f32>,
    /// The layer being printed, beside how many the file has.
    pub layer: Option<u32>,
    pub layer_count: Option<u32>,
    pub elapsed_s: Option<u64>,
    pub remaining_s: Option<u64>,
    /// Temperature of the light source, in °C.
    pub uv_led_c: Option<f32>,
    /// Temperature inside the hood, in °C.
    pub chamber_c: Option<f32>,
    /// Why the last print stopped, when the printer says.
    pub error: Option<String>,
}

impl State {
    pub(crate) fn of_board(status: &net_sdcp::Status) -> Self {
        let info = &status.print_info;
        let stage = info.stage();
        let machine = status.machine();
        let counted = info.total_layer > 0;
        Self {
            state: machine.label().to_owned(),
            busy: machine != net_sdcp::Machine::Idle,
            stage: (stage != net_sdcp::Stage::Idle).then(|| stage.label().to_owned()),
            file: (!info.filename.is_empty()).then(|| info.filename.clone()),
            progress: info.fraction(),
            layer: counted.then_some(info.current_layer),
            layer_count: counted.then_some(info.total_layer),
            elapsed_s: info.elapsed_s(),
            remaining_s: info.remaining_s(),
            uv_led_c: status.uv_led_c,
            chamber_c: status.chamber_c,
            error: info.error().map(str::to_owned),
        }
    }

    pub(crate) fn of_prusa(status: net_prusalink::Status) -> Self {
        let busy = matches!(
            status.printer.state.as_str(),
            "PRINTING" | "PAUSED" | "BUSY" | "ATTENTION"
        );
        Self {
            state: status.printer.state.to_lowercase(),
            busy,
            stage: None,
            file: None,
            progress: status.job.as_ref().map(|job| job.progress / 100.0),
            layer: None,
            layer_count: None,
            elapsed_s: None,
            remaining_s: status.job.and_then(|job| job.time_remaining),
            uv_led_c: None,
            chamber_c: None,
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
            r#"{"CurrentStatus":[1],"TempOfUVLED":31.5,"PrintInfo":{"Status":3,
                "CurrentLayer":50,"TotalLayer":200,"CurrentTicks":600000,
                "TotalTicks":2400000,"Filename":"cube.goo","ErrorNumber":0}}"#,
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
        assert_eq!((state.layer, state.layer_count), (Some(50), Some(200)));
        assert_eq!(
            state.remaining_s,
            Some(1800),
            "40 minutes estimated, 10 printed"
        );
        assert_eq!(state.uv_led_c, Some(31.5));
        assert!(state.busy, "a printing board takes no second print");
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
        assert!(state.busy);
    }

    #[test]
    fn a_board_that_finished_names_its_last_file_and_is_free_again() {
        let status: net_sdcp::Status = serde_json::from_str(
            r#"{"CurrentStatus":[0],"PrintInfo":{"Status":9,"CurrentLayer":200,
                "TotalLayer":200,"Filename":"cube.goo"}}"#,
        )
        .expect("a version 3 status report");
        let state = State::of_board(&status);
        assert!(!state.busy);
        assert_eq!(state.stage.as_deref(), Some("complete"));
        assert_eq!(state.file.as_deref(), Some("cube.goo"));
    }
}
