use std::path::{Path, PathBuf};

/// One plate of a whole-project run: its index, its name, and the stem the name template
/// gives its file, if the template was kept rather than a name typed over it.
pub struct PlateFile<'a> {
    pub plate: u32,
    pub name: &'a str,
    pub stem: Option<String>,
}

/// Where each plate of a whole-project run is written, beside `output`: its own stem, or the
/// typed stem of `output` with the plate's name on the end, as is a stem two plates share;
/// its number goes on where an earlier plate took that too (ADR 0387).
pub fn plate_outputs(output: &Path, plates: &[PlateFile<'_>]) -> Vec<(u32, PathBuf)> {
    let typed = output.file_stem().unwrap_or_default().to_string_lossy();
    let stem_of = |file: &PlateFile<'_>| file.stem.clone().unwrap_or_else(|| typed.to_string());
    let mut outputs: Vec<(u32, PathBuf)> = Vec::with_capacity(plates.len());
    for file in plates {
        let stem = stem_of(file);
        let shared = plates.iter().filter(|other| stem_of(other) == stem).count() > 1;
        let mut path = if file.stem.is_none() || shared {
            named(output, &format!("{stem}-{}", slug(file.name)))
        } else {
            named(output, &stem)
        };
        if outputs.iter().any(|(_, taken)| *taken == path) {
            path = named(output, &format!("{stem}-plate-{}", file.plate + 1));
        }
        outputs.push((file.plate, path));
    }
    outputs
}

/// A plate's name as it stands in a file name.
fn slug(name: &str) -> String {
    name.replace(' ', "-").to_lowercase()
}

/// `output` renamed to `stem`, keeping its extension; a dot inside the stem stays its own.
fn named(output: &Path, stem: &str) -> PathBuf {
    match output.extension() {
        Some(extension) => output.with_file_name(format!("{stem}.{}", extension.to_string_lossy())),
        None => output.with_file_name(stem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed<'a>(plates: &[(u32, &'a str)]) -> Vec<PlateFile<'a>> {
        plates
            .iter()
            .map(|&(plate, name)| PlateFile {
                plate,
                name,
                stem: None,
            })
            .collect()
    }

    #[test]
    fn each_plate_is_written_beside_the_name_that_was_typed() {
        let outputs = plate_outputs(
            Path::new("/tmp/batch.goo"),
            &typed(&[(0, "Plate 1"), (1, "Small parts")]),
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
        let outputs = plate_outputs(
            Path::new("/tmp/batch.goo"),
            &typed(&[(0, "Body"), (1, "Body")]),
        );
        assert_eq!(outputs[0].1, PathBuf::from("/tmp/batch-body.goo"));
        assert_eq!(outputs[1].1, PathBuf::from("/tmp/batch-plate-2.goo"));
    }

    #[test]
    fn a_dot_in_the_typed_name_keeps_the_plates_apart() {
        let outputs = plate_outputs(Path::new("/tmp/my.part.goo"), &typed(&[(0, "A"), (1, "B")]));
        assert_eq!(outputs[0].1, PathBuf::from("/tmp/my.part-a.goo"));
        assert_eq!(outputs[1].1, PathBuf::from("/tmp/my.part-b.goo"));
    }

    #[test]
    fn a_png_stack_takes_the_plate_on_its_directory_name() {
        let outputs = plate_outputs(Path::new("out"), &typed(&[(2, "Third")]));
        assert_eq!(outputs, [(2, PathBuf::from("out-third"))]);
    }

    /// BUG: a dot in the typed name was taken for the extension, so every plate of
    /// `my.part.goo` was written to `my.goo`.
    #[test]
    fn a_dot_in_the_name_stays_in_every_plate_s_file() {
        let outputs = plate_outputs(
            Path::new("/tmp/my.part.goo"),
            &typed(&[(0, "Plate 1"), (1, "Plate 2")]),
        );
        assert_eq!(outputs[0].1, PathBuf::from("/tmp/my.part-plate-1.goo"));
        assert_eq!(outputs[1].1, PathBuf::from("/tmp/my.part-plate-2.goo"));
    }

    #[test]
    fn a_template_filled_per_plate_names_each_plate_by_itself() {
        let plates = [(0, "Plate 1", "bracket_p1"), (1, "Plate 2", "bracket_p2")].map(
            |(plate, name, stem)| PlateFile {
                plate,
                name,
                stem: Some(stem.to_owned()),
            },
        );
        let outputs = plate_outputs(Path::new("/tmp/bracket_p2.goo"), &plates);
        assert_eq!(
            outputs,
            [
                (0, PathBuf::from("/tmp/bracket_p1.goo")),
                (1, PathBuf::from("/tmp/bracket_p2.goo")),
            ],
            "{{plate}} is each plate's own number, not the plate in hand's"
        );
    }
}
