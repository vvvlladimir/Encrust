use serde::{Deserialize, Serialize};
use toml::{Table, Value};

/// How a machine's panel takes the plate: the maker's own three-way setting. See ADR 0366.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mirror {
    /// The panel shows the plate as seen from above.
    #[default]
    Normal,
    /// An LCD panel that takes its masks mirrored along X.
    LcdMirror,
    /// A DLP projector; laid out as `Normal` until a known file says otherwise.
    DlpNormal,
}

impl Mirror {
    /// The three, in the order the form offers them.
    pub const ALL: [Self; 3] = [Self::Normal, Self::LcdMirror, Self::DlpNormal];

    /// Whether a mask for this panel is mirrored along X.
    pub fn mirrors_x(self) -> bool {
        self == Self::LcdMirror
    }
}

/// Rewrites the `mirror_x` and `mirror_y` flags of a profile from before ADR 0366 as `mirror`.
/// An old flag wins over a `mirror` beside it, which is where an edit lands over a shipped printer.
pub(crate) fn read_old_flags(table: &mut Table) {
    table.remove("mirror_y");
    if let Some(Value::Boolean(mirror_x)) = table.remove("mirror_x") {
        let mirror = if mirror_x { "lcd-mirror" } else { "normal" };
        table.insert("mirror".to_owned(), Value::String(mirror.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Row {
        #[serde(default)]
        mirror: Mirror,
    }

    fn mirror_of(text: &str) -> Mirror {
        let mut table: Table = toml::from_str(text).expect("valid TOML");
        read_old_flags(&mut table);
        let row: Row = table.try_into().expect("a known mirror");
        row.mirror
    }

    #[test]
    fn each_mirror_reads_from_its_kebab_case_name() {
        for (text, mirror) in [
            ("normal", Mirror::Normal),
            ("lcd-mirror", Mirror::LcdMirror),
            ("dlp-normal", Mirror::DlpNormal),
        ] {
            assert_eq!(mirror_of(&format!("mirror = \"{text}\"")), mirror);
        }
    }

    #[test]
    fn an_old_x_flag_reads_as_the_lcd_mirror() {
        assert_eq!(
            mirror_of("mirror_x = true\nmirror_y = false"),
            Mirror::LcdMirror
        );
        assert_eq!(
            mirror_of("mirror_x = false\nmirror_y = true"),
            Mirror::Normal
        );
    }

    #[test]
    fn an_old_flag_wins_over_the_field_beside_it() {
        assert_eq!(
            mirror_of("mirror = \"lcd-mirror\"\nmirror_x = false"),
            Mirror::Normal
        );
    }

    #[test]
    fn a_profile_stating_nothing_is_normal() {
        assert_eq!(mirror_of(""), Mirror::Normal);
    }
}
