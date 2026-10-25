use std::path::{Path, PathBuf};

/// Where each plate of a whole-project run is written: `output` with the plate's own name on
/// the end, or its number where an earlier plate took that name. `plates` pairs index and name.
pub fn plate_outputs(output: &Path, plates: &[(u32, &str)]) -> Vec<(u32, PathBuf)> {
    let mut outputs: Vec<(u32, PathBuf)> = Vec::with_capacity(plates.len());
    for &(plate, name) in plates {
        let mut path = beside(output, &name.replace(' ', "-").to_lowercase());
        if outputs.iter().any(|(_, taken)| *taken == path) {
            path = beside(output, &format!("plate-{}", plate + 1));
        }
        outputs.push((plate, path));
    }
    outputs
}

fn beside(output: &Path, suffix: &str) -> PathBuf {
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let mut path = output.with_file_name(format!("{stem}-{suffix}"));
    if let Some(extension) = output.extension() {
        path.set_extension(extension);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_plate_is_written_beside_the_name_that_was_typed() {
        let outputs = plate_outputs(
            Path::new("/tmp/batch.goo"),
            &[(0, "Plate 1"), (1, "Small parts")],
        );
        assert_eq!(
            outputs,
            [
                (0, PathBuf::from("/tmp/batch-plate-1.goo")),
                (1, PathBuf::from("/tmp/batch-small-parts.goo")),
            ],
            "a renamed plate is named in the file, not numbered"
        );
    }

    #[test]
    fn two_plates_of_one_name_do_not_write_to_one_file() {
        let outputs = plate_outputs(Path::new("/tmp/batch.goo"), &[(0, "Body"), (1, "Body")]);
        assert_eq!(outputs[0].1, PathBuf::from("/tmp/batch-body.goo"));
        assert_eq!(outputs[1].1, PathBuf::from("/tmp/batch-plate-2.goo"));
    }

    #[test]
    fn a_png_stack_takes_the_plate_on_its_directory_name() {
        let outputs = plate_outputs(Path::new("out"), &[(2, "Third")]);
        assert_eq!(outputs, [(2, PathBuf::from("out-third"))]);
    }
}
