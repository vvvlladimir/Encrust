use super::*;

fn asked(args: &[&str]) -> Answer {
    let args: Vec<String> = args.iter().map(|word| (*word).to_owned()).collect();
    answered(7, &args, &Stop::default(), Arc::new(Steps::default()))
}

#[test]
fn a_run_is_answered_with_its_id_its_code_and_the_document_it_printed() {
    let answer = asked(&["profiles", "list"]);
    assert_eq!(answer.id, 7);
    assert_eq!(answer.exit_code, 0, "{}", answer.report);
    assert_eq!(answer.report["schema"], json::SCHEMA, "{}", answer.report);
}

#[test]
fn words_the_command_line_refuses_are_answered_as_an_error_document() {
    let answer = asked(&["no-such-command"]);
    assert_eq!(answer.exit_code, 1);
    assert!(
        answer.report["error"]["message"].is_string(),
        "{}",
        answer.report
    );
}

#[test]
fn serve_refuses_to_run_itself() {
    let answer = asked(&["serve"]);
    assert_eq!(answer.exit_code, 1);
    assert!(answer.report["error"].is_object(), "{}", answer.report);
}

#[test]
fn a_line_asks_a_run_or_a_stop() {
    let run: Asked = serde_json::from_str(r#"{"id":3,"args":["inspect","a.stl"]}"#)
        .expect("a run is a line of JSON");
    assert!(matches!(run, Asked::Run { id: 3, ref args } if args.len() == 2));
    let stop: Asked = serde_json::from_str(r#"{"cancel":3}"#).expect("a stop is one too");
    assert!(matches!(stop, Asked::Cancel { cancel: 3 }));
}

#[test]
fn a_run_asked_under_an_id_still_going_is_answered_as_failed_and_leaves_that_run_be() {
    let going: Going = Arc::default();
    let first = Stop::default();
    lock(&going).insert(7, first.clone());
    let written = Arc::new(Mutex::new(Vec::<u8>::new()));
    let out: Out = Arc::clone(&written) as Out;
    let steps = Arc::new(Steps::default());
    let thread = started(7, vec!["profiles".into()], (&steps, &going, &out));
    assert!(thread.is_none(), "no second run starts");
    let line = String::from_utf8(lock(&written).clone()).expect("JSON is text");
    let answer: Value = serde_json::from_str(&line).expect("one line of JSON");
    assert_eq!(answer["exit_code"], 1, "{answer}");
    lock(&going).get(&7).expect("the first run stays").request();
    assert!(
        first.requested(),
        "the first run's stop is still the one kept"
    );
}
