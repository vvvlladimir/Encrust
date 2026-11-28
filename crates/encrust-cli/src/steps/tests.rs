use super::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("encrust-steps-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the temp dir is writable");
    dir
}

#[test]
fn a_key_chained_through_the_same_steps_is_the_same_key() {
    let start = Key::of(&[b"models", b"cube.stl"]);
    let one = start.then(b"hollow 2").then(b"drain auto");
    let again = start.then(b"hollow 2").then(b"drain auto");
    assert_eq!(one, again);
    assert_ne!(one, start.then(b"hollow 2.5").then(b"drain auto"));
}

#[test]
fn parts_are_told_apart_by_their_lengths() {
    assert_ne!(Key::of(&[b"ab", b"c"]), Key::of(&[b"a", b"bc"]));
}

#[test]
fn a_file_is_keyed_by_its_bytes_and_rekeyed_once_rewritten() {
    let path = scratch("file").join("model.stl");
    std::fs::write(&path, b"solid one").expect("the temp dir is writable");
    let steps = Steps::default();
    let first = steps.file_key(&path).expect("the file reads");
    assert_eq!(steps.file_key(&path).expect("the file reads"), first);
    std::fs::write(&path, b"solid two, longer").expect("the temp dir is writable");
    assert_ne!(steps.file_key(&path).expect("the file reads"), first);
}

#[test]
fn a_file_is_loaded_once_while_it_stands_as_it_was() {
    let path = scratch("loaded").join("model.stl");
    std::fs::write(&path, b"solid one").expect("the temp dir is writable");
    let steps = Steps::default();
    let mut loads = 0;
    for _ in 0..3 {
        let loaded: Result<_, ()> = steps.loaded(&path, || {
            loads += 1;
            Ok(Loaded::default())
        });
        loaded.expect("the load is asked to succeed");
    }
    assert_eq!(loads, 1, "the second and third read what the first loaded");
}

#[test]
fn a_project_written_is_known_again_after_it_is_moved() {
    let dir = scratch("written");
    let path = dir.join("1.encrust");
    std::fs::write(&path, vec![7u8; 200_000]).expect("the temp dir is writable");
    let steps = Steps::default();
    let key = Key::of(&[b"plate"]);
    let document = Document::of_models(core_engine::project::Manifest::new(None, None), Vec::new());
    steps.wrote(&path, key, document);
    let moved = dir.join("2.encrust");
    std::fs::rename(&path, &moved).expect("a rename in one folder succeeds");
    let (found, _) = steps
        .written(&moved)
        .expect("the moved file is the one written");
    assert_eq!(found, key);
    std::fs::write(&moved, vec![8u8; 200_000]).expect("the temp dir is writable");
    assert!(
        steps.written(&moved).is_none(),
        "a file rewritten is not the one written"
    );
}

#[test]
fn an_edit_that_reads_a_file_is_keyed_by_its_bytes_too() {
    let font = scratch("font").join("face.ttf");
    std::fs::write(&font, b"one face").expect("the temp dir is writable");
    let edit = |font: &std::path::Path| Edit::Label {
        object: ObjectId(1),
        text: "A".to_owned(),
        at_mm: core_geometry::Vec3::ZERO,
        height_mm: None,
        depth_mm: None,
        raised: true,
        follow_surface: false,
        angle_deg: 0.0,
        font: Some(font.to_path_buf()),
    };
    let before = edit_bytes(&edit(&font));
    std::fs::write(&font, b"another face").expect("the temp dir is writable");
    assert_ne!(edit_bytes(&edit(&font)), before);
}

#[test]
fn an_edit_asked_again_of_a_plate_kept_is_answered_from_it_without_being_carried_out() {
    use crate::exit::Stop;
    use crate::stage::Staging;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.encrust");
    let opened = || crate::project::open(&path).expect("the fixture opens");
    let steps = Arc::new(Steps::default());
    let seed = || Some((Arc::clone(&steps), Key::of(&[b"project", b"cube"])));
    let mut first = Staging::of_project(opened(), 0, Vec::new(), seed());
    let cube = first.ids()[0];
    let delete = Edit::Delete { object: cube };
    first
        .apply(&delete, cube, &Stop::default())
        .expect("the cube is there to delete");

    let mut bare = opened();
    bare.manifest.objects.clear();
    bare.models.clear();
    let mut again = Staging::of_project(bare, 0, Vec::new(), seed());
    again
        .apply(&delete, cube, &Stop::default())
        .expect("a plate with no cube deletes it only from the plate kept");
    assert!(again.ids().is_empty(), "the plate kept has no object left");
}
