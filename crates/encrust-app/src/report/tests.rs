use super::*;

fn facts<'a>(settings: &'a ToolSettings, resin: &'a MaterialProfile) -> Facts<'a> {
    Facts {
        printer: None,
        printer_id: None,
        resin,
        resin_id: Some("grey"),
        format: "goo",
        message: None,
        hosts: &[],
        plate: Plate {
            models: 2,
            triangles: 43_834,
            vertices: 22_579,
        },
        settings,
    }
}

fn written(report: &Report) -> String {
    let settings = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
    let resin = MaterialProfile::default();
    report.markdown(&facts(&settings, &resin))
}

#[test]
fn the_report_states_the_build_and_what_the_user_typed() {
    let report = Report {
        what: "the mask comes out mirrored".to_owned(),
        steps: "slice the cube, open the file".to_owned(),
        ..Report::default()
    };
    let text = written(&report);
    assert!(text.contains("## What happened, and what was expected"));
    assert!(text.contains("the mask comes out mirrored"));
    assert!(text.contains("slice the cube, open the file"));
    assert!(text.contains(env!("CARGO_PKG_VERSION")));
    assert!(text.contains(PLATFORM));
    assert!(text.contains("Output format:** .goo"));
}

#[test]
fn nothing_typed_still_makes_a_report() {
    let text = written(&Report::default());
    assert!(text.contains("## How to reproduce it\n\n—"));
}

#[test]
fn a_switch_turned_off_keeps_its_section_out() {
    let full = written(&Report::default());
    assert!(full.contains("## Resin profile"));
    assert!(full.contains("## Tool settings"));
    assert!(full.contains("## What stands on the plate"));

    let bare = written(&Report {
        include: Include {
            profiles: false,
            settings: false,
            plate: false,
        },
        ..Report::default()
    });
    assert!(!bare.contains("## Resin profile"));
    assert!(!bare.contains("## Tool settings"));
    assert!(!bare.contains("## What stands on the plate"));
    assert!(bare.contains("## Encrust"), "the build is always stated");
}

#[test]
fn a_path_in_a_failure_is_cut_to_its_extension() {
    assert_eq!(
        scrubbed("cannot load /home/ada/patient.stl: no such file", &[]),
        "cannot load .../*.stl: no such file"
    );
    assert_eq!(
        scrubbed(r"cannot write C:\Users\Ada\plate.goo", &[]),
        "cannot write .../*.goo"
    );
    assert_eq!(
        scrubbed("cannot load patient.3MF", &[]),
        "cannot load *.3mf"
    );
    assert_eq!(scrubbed("cannot read ../out", &[]), "cannot read .../*");
}

#[test]
fn a_failure_keeps_the_words_that_are_not_names() {
    assert_eq!(
        scrubbed("version 0.1.0 refused the file", &[]),
        "version 0.1.0 refused the file"
    );
    assert_eq!(
        scrubbed("see https://encrust.app/guides/ for this", &[]),
        "see https://encrust.app/guides/ for this"
    );
}

#[test]
fn a_printer_a_failure_names_is_not_in_the_report() {
    let hosts = vec!["printer-in-the-hall".to_owned(), "192.168.1.42".to_owned()];
    assert_eq!(
        scrubbed("192.168.1.42 rejected the credentials", &hosts),
        "... rejected the credentials"
    );
    assert_eq!(
        scrubbed("cannot reach http://192.168.1.42/api/version", &hosts),
        "cannot reach http://..."
    );
    assert_eq!(
        scrubbed("printer-in-the-hall did not answer", &hosts),
        "... did not answer"
    );
    assert_eq!(
        scrubbed("ada-macbook.local did not answer", &[]),
        "... did not answer"
    );
    assert_eq!(
        scrubbed("[fe80::1]:8080 refused the file", &[]),
        "... refused the file"
    );
    assert_eq!(
        scrubbed("the printer at 10.0.0.7:3000 is silent", &[]),
        "the printer at ... is silent"
    );
}

#[test]
fn the_prefilled_issue_names_the_form_and_stays_under_the_budget() {
    let report = Report {
        what: "x".repeat(20_000),
        steps: "open the window".to_owned(),
        ..Report::default()
    };
    let settings = ToolSettings::of(&crate::state::Tools::default(), &Default::default());
    let resin = MaterialProfile::default();
    let mut facts = facts(&settings, &resin);
    facts.message = Some("cannot load /home/ada/patient.stl".to_owned());
    let url = report.issue_url(&facts);

    assert!(url.len() <= MAX_URL, "a URL GitHub refuses is no help");
    assert!(url.starts_with(NEW_ISSUE));
    assert!(url.contains("template=bug.yml"));
    assert!(url.contains(&format!("version={}", env!("CARGO_PKG_VERSION"))));
    assert!(url.contains("os=") && url.contains(PLATFORM));
    assert!(
        url.contains("log=cannot%20load%20...%2F%2A.stl"),
        "the path is scrubbed in the form too, not only in the report"
    );
    assert!(
        url.contains("steps=open%20the%20window"),
        "a short field after a long one still fits"
    );
    assert!(
        !url.contains("what="),
        "the long one is left to the clipboard"
    );
}
