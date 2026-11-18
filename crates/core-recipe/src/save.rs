//! Where a save step writes: its `folder`, `~` the user's home.

use std::path::{Path, PathBuf};

/// The folder `asked` names, a leading `~` read as `home`; as written where there is no home.
pub fn save_folder(asked: &str, home: Option<&Path>) -> PathBuf {
    match (asked.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            home.join(rest.trim_start_matches(['/', '\\']))
        }
        _ => PathBuf::from(asked),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tilde_is_the_home_and_only_at_the_start_of_a_folder() {
        let home = Some(Path::new("/home/me"));
        assert_eq!(save_folder("~", home), PathBuf::from("/home/me"));
        assert_eq!(
            save_folder("~/prints", home),
            PathBuf::from("/home/me/prints")
        );
        assert_eq!(
            save_folder("~\\prints", home),
            PathBuf::from("/home/me/prints")
        );
        assert_eq!(
            save_folder("~bob/x", home),
            PathBuf::from("~bob/x"),
            "another's home"
        );
        assert_eq!(
            save_folder("/srv/prints", home),
            PathBuf::from("/srv/prints")
        );
        assert_eq!(save_folder("~/prints", None), PathBuf::from("~/prints"));
    }
}
