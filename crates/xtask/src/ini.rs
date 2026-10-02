//! The `key = value` dialect the source printer profiles are written in.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

/// One source profile, read as a flat table. Comments and blank lines are dropped and a
/// repeated key keeps its last value, which is what the writers of these files assume.
pub struct Ini(HashMap<String, String>);

impl Ini {
    pub fn load(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Ok(Self::parse(&source))
    }

    fn parse(source: &str) -> Self {
        let entries = source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
            .collect();
        Self(entries)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    fn required(&self, key: &str) -> Result<&str> {
        self.get(key)
            .ok_or_else(|| anyhow!("the source profile has no `{key}`"))
    }

    pub fn u32(&self, key: &str) -> Result<u32> {
        let value = self.required(key)?;
        value
            .parse()
            .with_context(|| format!("`{key}` is not a whole number: {value}"))
    }

    pub fn f32(&self, key: &str) -> Result<f32> {
        let value = self.required(key)?;
        value
            .parse()
            .with_context(|| format!("`{key}` is not a number: {value}"))
    }

    /// A flag the source writes as `0` or `1`.
    pub fn flag(&self, key: &str) -> Result<bool> {
        Ok(self.u32(key)? != 0)
    }

    /// The `FILEFORMAT_` keyword in the notes block, with the `FILEVERSION_` beside it.
    ///
    /// Both are keywords inside one free-text field, so they are looked for rather than
    /// read off a key of their own.
    pub fn container_keyword(&self) -> Option<(String, Option<u32>)> {
        let notes = self.get("printer_notes")?;
        let keyword = keyword_after(notes, "FILEFORMAT_")?;
        let version = keyword_after(notes, "FILEVERSION_").and_then(|v| v.parse().ok());
        Some((keyword, version))
    }

    /// The `FILECLASS_` keyword, where the notes carry one: the firmware that reads the
    /// container, for an extension more than one firmware claims.
    pub fn file_class(&self) -> Option<String> {
        keyword_after(self.get("printer_notes")?, "FILECLASS_")
    }
}

/// The token following `prefix`, up to whatever ends a keyword in the notes block.
fn keyword_after(notes: &str, prefix: &str) -> Option<String> {
    let rest = notes.split_once(prefix)?.1;
    let token: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
        .collect();
    (!token.is_empty()).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# a comment\ndisplay_width = 143.43\n\ndisplay_mirror_x = 1\n\
        printer_notes = PRINTER_VENDOR_X\\nFILEFORMAT_CTB\\nFILEVERSION_4\\n\\nSTART_CUSTOM_VALUES\n\
        print_host = \n";

    #[test]
    fn a_comment_and_a_blank_line_are_not_keys() {
        let ini = Ini::parse(SAMPLE);
        assert_eq!(ini.get("# a comment"), None);
        let width = ini.f32("display_width").expect("a number");
        assert!(
            (width - 143.43).abs() < 1e-4,
            "{width} is not the source value"
        );
        assert!(ini.flag("display_mirror_x").expect("a flag"));
    }

    #[test]
    fn an_empty_value_counts_as_absent() {
        let ini = Ini::parse(SAMPLE);
        assert_eq!(ini.get("print_host"), None);
        assert!(ini.u32("print_host").is_err());
    }

    #[test]
    fn a_missing_key_names_itself() {
        let ini = Ini::parse(SAMPLE);
        let message = ini.u32("display_pixels_x").unwrap_err().to_string();
        assert!(message.contains("display_pixels_x"), "{message}");
    }

    #[test]
    fn the_container_keyword_comes_out_of_the_notes_block() {
        let ini = Ini::parse(SAMPLE);
        let (keyword, version) = ini.container_keyword().expect("a keyword");
        assert_eq!(keyword, "CTB");
        assert_eq!(version, Some(4));
    }

    #[test]
    fn a_firmware_class_is_read_beside_the_container() {
        assert_eq!(Ini::parse(SAMPLE).file_class(), None);
        let ini = Ini::parse("printer_notes = x\\nFILEFORMAT_ZIP\\nFILECLASS_KLIPPER\\ny");
        assert_eq!(ini.file_class().as_deref(), Some("KLIPPER"));
    }

    #[test]
    fn notes_without_a_format_keyword_name_no_container() {
        let ini = Ini::parse("printer_notes = PRINTER_VENDOR_PRUSA3D\\nPRINTER_MODEL_SL1\\n");
        assert!(ini.container_keyword().is_none());
    }

    #[test]
    fn a_keyword_with_a_dot_in_it_keeps_the_dot() {
        let ini = Ini::parse("printer_notes = x\\nFILEFORMAT_ENCRYPTED.CTB\\ny");
        let (keyword, version) = ini.container_keyword().expect("a keyword");
        assert_eq!(keyword, "ENCRYPTED.CTB");
        assert_eq!(version, None);
    }
}
