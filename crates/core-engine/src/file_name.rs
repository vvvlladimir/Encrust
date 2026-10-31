//! A sliced file's name from a template the user writes, the same in the window and on the
//! command line (ADR 0387).

use jiff::civil::DateTime;

/// The template a name follows until the user writes another: the model's name alone.
pub const DEFAULT_TEMPLATE: &str = "{model}";

/// What the placeholders of a template stand for.
pub struct NameFields<'a> {
    /// The model's name without the extension of the file it came from.
    pub model: &'a str,
    /// That extension, `stl`, or empty for a model from no file.
    pub model_ext: &'a str,
    pub printer: &'a str,
    pub resin: &'a str,
    /// Written into the name in whole µm.
    pub layer_height_mm: f32,
    /// Counted from one.
    pub plate: u32,
    /// The local time the slice starts.
    pub when: DateTime,
}

/// What a name falls back to when the template leaves nothing a file system takes.
const FALLBACK: &str = "model";

/// The longest a name may be before its extension, bytes: file systems stop at 255, and a
/// plate's suffix and the extension still go on.
const MAX_STEM_BYTES: usize = 200;

/// What Windows, macOS or Linux refuse in a file name.
const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Names Windows keeps for devices, whatever follows the first dot.
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `template` filled in from `fields` and made safe for any file system, without its
/// extension. A placeholder nothing answers stays as written.
pub fn file_stem(template: &str, fields: &NameFields) -> String {
    let mut filled = String::with_capacity(template.len() + fields.model.len());
    for piece in pieces(template) {
        match piece {
            Piece::Text(text) => filled.push_str(text),
            Piece::Tag(tag) => filled.push_str(&value(tag, fields)),
        }
    }
    safe(&filled)
}

/// The placeholders in `template` that stand for nothing, so the window can say which.
pub fn unknown_tags(template: &str) -> Vec<&str> {
    pieces(template)
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Tag(tag) if !TAGS.contains(&tag) => Some(tag),
            _ => None,
        })
        .collect()
}

const TAGS: [&str; 8] = [
    "model", "ext", "printer", "resin", "layer", "plate", "date", "time",
];

enum Piece<'a> {
    Text(&'a str),
    Tag(&'a str),
}

/// `template` cut into text and `{tag}`s; a brace that closes nothing is text.
fn pieces(template: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        pieces.push(Piece::Text(&rest[..open]));
        let after = &rest[open + 1..];
        match after.find(['{', '}']) {
            Some(end) if after[end..].starts_with('}') => {
                pieces.push(Piece::Tag(&after[..end]));
                rest = &after[end + 1..];
            }
            _ => {
                pieces.push(Piece::Text("{"));
                rest = after;
            }
        }
    }
    pieces.push(Piece::Text(rest));
    pieces
}

fn value(tag: &str, fields: &NameFields) -> String {
    match tag {
        "model" => fields.model.to_owned(),
        "ext" => fields.model_ext.to_owned(),
        "printer" => fields.printer.to_owned(),
        "resin" => fields.resin.to_owned(),
        "layer" => format!("{:.0}", fields.layer_height_mm * 1000.0),
        "plate" => fields.plate.to_string(),
        "date" => fields.when.strftime("%Y-%m-%d").to_string(),
        // A colon is refused on Windows and shown as a slash on macOS.
        "time" => fields.when.strftime("%H-%M").to_string(),
        _ => format!("{{{tag}}}"),
    }
}

/// `name` with what a file system refuses replaced, not dropped, so two names stay apart.
fn safe(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if FORBIDDEN.contains(&c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut stem = shortened(replaced.trim_start(), MAX_STEM_BYTES)
        .trim_end_matches(['.', ' '])
        .to_owned();
    if stem.is_empty() {
        return FALLBACK.to_owned();
    }
    // Windows ignores spaces after a device name: `con .v2` is `con` too.
    let device = stem.split('.').next().unwrap_or_default().trim_end();
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(device))
    {
        stem.insert(device.len(), '_');
    }
    stem
}

/// The longest start of `text` within `bytes`, cut between characters.
fn shortened(text: &str, bytes: usize) -> &str {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> NameFields<'static> {
        NameFields {
            model: "bracket",
            model_ext: "stl",
            printer: "ELEGOO Saturn 4 Ultra",
            resin: "Grey",
            layer_height_mm: 0.05,
            plate: 2,
            when: jiff::civil::date(2026, 10, 31).at(9, 5, 0, 0),
        }
    }

    #[test]
    fn the_default_template_is_the_models_name() {
        assert_eq!(file_stem(DEFAULT_TEMPLATE, &fields()), "bracket");
    }

    #[test]
    fn every_placeholder_is_filled_in() {
        let stem = file_stem(
            "{model}-{ext}_{printer}_{resin}_{layer}um_p{plate}_{date}_{time}",
            &fields(),
        );
        assert_eq!(
            stem,
            "bracket-stl_ELEGOO Saturn 4 Ultra_Grey_50um_p2_2026-10-31_09-05"
        );
    }

    #[test]
    fn an_unknown_placeholder_stays_as_written_and_is_named() {
        assert_eq!(file_stem("{model}-{colour}", &fields()), "bracket-{colour}");
        assert_eq!(unknown_tags("{model}-{colour}{ {x"), ["colour"]);
    }

    #[test]
    fn a_brace_that_closes_nothing_is_text() {
        assert_eq!(file_stem("{{model}", &fields()), "{bracket");
        assert_eq!(file_stem("model}", &fields()), "model}");
    }

    #[test]
    fn what_a_file_system_refuses_is_replaced_not_dropped() {
        let model = NameFields {
            model: "a/b:c",
            ..fields()
        };
        assert_eq!(file_stem("{model}?", &model), "a_b_c_");
        assert_eq!(
            file_stem("sub/{model}", &fields()),
            "sub_bracket",
            "no folder is made"
        );
    }

    #[test]
    fn a_name_left_empty_falls_back() {
        assert_eq!(file_stem("", &fields()), FALLBACK);
        assert_eq!(file_stem(" ..", &fields()), FALLBACK);
    }

    #[test]
    fn a_windows_device_name_is_kept_off() {
        let model = NameFields {
            model: "con",
            ..fields()
        };
        assert_eq!(file_stem("{model}", &model), "con_");
        assert_eq!(file_stem("{model}.v2", &model), "con_.v2");
        assert_eq!(file_stem("{model} .v2", &model), "con_ .v2");
        assert_eq!(file_stem("{model}sole", &model), "console");
    }

    #[test]
    fn a_long_name_is_cut_between_characters() {
        let long = "ж".repeat(MAX_STEM_BYTES);
        let model = NameFields {
            model: &long,
            ..fields()
        };
        let stem = file_stem("{model}", &model);
        assert_eq!(stem.len(), MAX_STEM_BYTES, "{} bytes", stem.len());
    }
}
