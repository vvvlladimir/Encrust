//! Which folder a program keeps searches in: one a build, so that the window and the command
//! line built together share it and a rebuilt search never reads an older one's (ADR 0466).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::KEPT_POSES;

/// Names the folder searches are kept in, or `off` for none.
pub const KEPT_POSES_ENV: &str = "ENCRUST_POSE_CACHE";
/// Folders of other builds kept beside this one's, the most recently made.
const BUILDS_KEPT: usize = 4;

/// The folder this program keeps searches in: the one `ENCRUST_POSE_CACHE` names, else this
/// build's under `cache`, the program's cache folder; none in a debug build or with `off`.
pub fn kept_poses_folder(cache: Option<&Path>) -> Option<PathBuf> {
    match std::env::var_os(KEPT_POSES_ENV) {
        Some(named) if named == "off" => None,
        Some(named) => Some(PathBuf::from(named)),
        None if cfg!(debug_assertions) => None,
        None => Some(this_build(&cache?.join(KEPT_POSES), &build_stamp()?)),
    }
}

/// The version and the build: `ENCRUST_BUILD_ID` where the build named one, as the release and
/// `cargo xtask` do, else when this program was made, a folder no other program shares.
fn build_stamp() -> Option<String> {
    let build = if let Some(id) = option_env!("ENCRUST_BUILD_ID") {
        id.to_owned()
    } else {
        let made = fs::metadata(std::env::current_exe().ok()?)
            .ok()?
            .modified()
            .ok()?;
        format!("made-{}", made.duration_since(UNIX_EPOCH).ok()?.as_secs())
    };
    Some(format!("{}-{build}", env!("CARGO_PKG_VERSION")))
}

/// `root`'s folder `stamp`, the folders of all but the `BUILDS_KEPT` latest other builds taken
/// away.
fn this_build(root: &Path, stamp: &str) -> PathBuf {
    if let Ok(listed) = fs::read_dir(root) {
        let mut others: Vec<_> = (listed.filter_map(Result::ok))
            .filter(|entry| entry.file_name() != stamp)
            .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
            .collect();
        others.sort_unstable_by_key(|(made, _)| std::cmp::Reverse(*made));
        for (_, old) in others.iter().skip(BUILDS_KEPT) {
            _ = fs::remove_dir_all(old);
        }
    }
    root.join(stamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_but_the_latest_other_builds_are_taken_away() {
        let root = std::env::temp_dir().join(format!("encrust-builds-{}", std::process::id()));
        _ = fs::remove_dir_all(&root);
        for build in 0..=BUILDS_KEPT + 2 {
            fs::create_dir_all(root.join(format!("old-{build}")))
                .expect("the temp dir is writable");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(this_build(&root, "ours"), root.join("ours"));
        let left = fs::read_dir(&root).map_or(0, Iterator::count);
        assert_eq!(left, BUILDS_KEPT, "the latest other builds stay");
        assert!(
            root.join(format!("old-{}", BUILDS_KEPT + 2)).is_dir(),
            "the newest stays"
        );
        assert!(!root.join("old-0").exists(), "the oldest goes");
    }
}
