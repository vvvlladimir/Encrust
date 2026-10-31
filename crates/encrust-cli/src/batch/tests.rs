use super::*;

#[test]
fn only_models_are_picked_up_and_they_come_out_sorted() {
    let dir = std::env::temp_dir().join("encrust-batch-listing");
    std::fs::create_dir_all(&dir).expect("the temporary directory is writable");
    for name in [
        "b.stl",
        "a.STL",
        "c.obj",
        "d.3mf",
        "notes.txt",
        "batch.json",
    ] {
        std::fs::write(dir.join(name), b"").expect("the temporary directory is writable");
    }

    let models = models_in(&dir).expect("the directory reads");
    let names: Vec<&str> = models
        .iter()
        .filter_map(|path| path.file_name()?.to_str())
        .collect();
    assert_eq!(
        names,
        ["a.STL", "b.stl", "c.obj", "d.3mf"],
        "case is not what makes a model"
    );

    std::fs::remove_dir_all(&dir).expect("the directory was just made");
}

fn named(name: &str) -> crate::commands::BatchCommand {
    let cli = <crate::Cli as clap::Parser>::parse_from([
        "encrust", "batch", "models", "-o", "out", "--name", name,
    ]);
    let crate::Command::Batch(command) = cli.command else {
        unreachable!("the batch subcommand was parsed");
    };
    command
}

fn grey() -> Chosen {
    Chosen {
        printer: None,
        material: printer_profiles::MaterialProfile {
            name: "Grey".to_owned(),
            layer_height_mm: 0.05,
            ..printer_profiles::MaterialProfile::default()
        },
        invented_exposure: false,
    }
}

#[test]
fn each_file_is_named_by_the_template() {
    let command = named("{model}_{resin}_{layer}um");
    let models = [
        PathBuf::from("models/part.v2.stl"),
        PathBuf::from("models/b.stl"),
    ];
    let outputs = outputs_for(&models, &command.batch(), &grey()).expect("names differ");
    assert_eq!(
        outputs,
        [
            PathBuf::from("out/part.v2_Grey_50um"),
            PathBuf::from("out/b_Grey_50um")
        ],
        "a dot in the model's name is kept"
    );
}

#[test]
fn a_template_giving_two_models_one_file_is_refused() {
    let command = named("{resin}");
    let models = [PathBuf::from("models/a.stl"), PathBuf::from("models/b.stl")];
    let error = outputs_for(&models, &command.batch(), &grey()).expect_err("one file twice");
    assert!(error.to_string().contains("two models"), "got {error:#}");
}

#[test]
fn names_differing_only_in_case_are_one_file() {
    let command = named("{model}");
    let models = [
        PathBuf::from("models/Part.stl"),
        PathBuf::from("models/part.obj"),
    ];
    let error = outputs_for(&models, &command.batch(), &grey()).expect_err("Part and part");
    assert!(error.to_string().contains("two models"), "got {error:#}");
}

#[test]
fn a_directory_with_nothing_in_it_is_an_error_rather_than_an_empty_run() {
    let dir = std::env::temp_dir().join("encrust-batch-empty");
    std::fs::create_dir_all(&dir).expect("the temporary directory is writable");

    let cli = <crate::Cli as clap::Parser>::parse_from([
        "encrust",
        "batch",
        dir.to_str().expect("ascii path"),
    ]);
    let crate::Command::Batch(command) = &cli.command else {
        unreachable!("the batch subcommand was parsed");
    };
    let chosen = crate::profiles::Chosen {
        printer: None,
        material: printer_profiles::MaterialProfile::default(),
        invented_exposure: false,
    };
    let watch = Watch {
        talk: false,
        progress: false,
        stop: &Stop::default(),
    };
    let error = run(&command.batch(), &chosen, &watch).expect_err("there is nothing to slice");
    assert!(error.to_string().contains("no models"));

    std::fs::remove_dir_all(&dir).expect("the directory was just made");
}

#[test]
fn a_batch_stopped_by_ctrl_c_starts_no_model_and_reports_none() {
    let dir = std::env::temp_dir().join("encrust-batch-stopped");
    let out = dir.join("out");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary directory is writable");
    for name in ["a.stl", "b.stl"] {
        std::fs::write(dir.join(name), b"").expect("the temporary directory is writable");
    }

    let cli = <crate::Cli as clap::Parser>::parse_from([
        "encrust",
        "batch",
        dir.to_str().expect("ascii path"),
        "-o",
        out.to_str().expect("ascii path"),
    ]);
    let crate::Command::Batch(command) = &cli.command else {
        unreachable!("the batch subcommand was parsed");
    };
    let chosen = crate::profiles::Chosen {
        printer: None,
        material: printer_profiles::MaterialProfile::default(),
        invented_exposure: false,
    };
    let stop = Stop::default();
    stop.request();
    let watch = Watch {
        talk: false,
        progress: false,
        stop: &stop,
    };
    let error = run(&command.batch(), &chosen, &watch).expect_err("the batch was stopped");
    assert!(is_cancelled(&error), "got {error:#}");
    assert!(
        !out.join("a.json").exists() && !out.join("b.json").exists(),
        "a model Ctrl-C kept from starting is not reported as failed"
    );

    std::fs::remove_dir_all(&dir).expect("the directory was just made");
}
