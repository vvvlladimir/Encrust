use core_geometry::{Vec3, cuboid};

use super::*;

fn folder(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("encrust-kept-poses-{name}-{}", std::process::id()));
    _ = fs::remove_dir_all(&folder);
    folder
}

fn kept_in(folder: &Path, mesh: &Mesh, settings: &OrientSettings) -> Vec<Choice> {
    let printed = Printed::solid(mesh);
    orient_for_print_kept(Some(folder), &printed, settings, None, Watch::never())
        .expect("a block has poses")
}

fn files_in(folder: &Path) -> Vec<PathBuf> {
    let listed = fs::read_dir(folder).expect("the folder was made");
    listed
        .filter_map(|entry| Some(entry.ok()?.path()))
        .collect()
}

#[test]
fn a_search_asked_again_is_read_from_the_folder() {
    let folder = folder("again");
    let block = cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0));
    let settings = OrientSettings::default();
    let searched = kept_in(&folder, &block, &settings);
    let files = files_in(&folder);
    assert_eq!(files.len(), 1, "one search kept: {files:?}");
    assert_eq!(
        kept_in(&folder, &block, &settings),
        searched,
        "read back exactly"
    );
    // Keep one choice alone, so an answer of one shows it was read rather than searched.
    let one = Kept {
        engine: env!("CARGO_PKG_VERSION").to_owned(),
        choices: searched[..1].to_vec(),
    };
    fs::write(&files[0], serde_json::to_vec(&one).expect("writes")).expect("writable");
    assert_eq!(kept_in(&folder, &block, &settings), searched[..1]);
}

#[test]
fn another_model_or_another_setting_is_searched_anew() {
    let folder = folder("anew");
    let block = cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0));
    let settings = OrientSettings::default();
    kept_in(&folder, &block, &settings);
    kept_in(
        &folder,
        &cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 31.0)),
        &settings,
    );
    let tilted = OrientSettings {
        max_tilt_deg: Some(10.0),
        ..settings
    };
    kept_in(&folder, &block, &tilted);
    assert_eq!(files_in(&folder).len(), 3, "three searches, three keys");
}

#[test]
fn choices_kept_by_another_engine_are_not_read() {
    let folder = folder("engine");
    let block = cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0));
    let settings = OrientSettings::default();
    let searched = kept_in(&folder, &block, &settings);
    let path = &files_in(&folder)[0];
    let older = Kept {
        engine: "0.0.0".to_owned(),
        choices: Vec::new(),
    };
    fs::write(path, serde_json::to_vec(&older).expect("writes")).expect("writable");
    assert_eq!(kept_in(&folder, &block, &settings), searched);
}

#[test]
fn a_folder_that_cannot_be_written_is_a_search_like_any_other() {
    let file = folder("not-a-folder");
    fs::write(&file, b"").expect("the temp dir is writable");
    let block = cuboid(Vec3::ZERO, Vec3::new(10.0, 20.0, 30.0));
    let settings = OrientSettings::default();
    assert!(!kept_in(&file, &block, &settings).is_empty());
}
