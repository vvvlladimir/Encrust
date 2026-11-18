//! Cutting the user's own names out of a message before the report states it: paths and
//! file names down to their extension, and every host on the user's network.

use std::net::IpAddr;
use std::path::Path;

/// Characters cut off the end of a word before it is read as a path, so that
/// `cannot open /home/ada/boat.stl:` is scrubbed along with the plain name.
const TRAILING: [char; 7] = ['.', ',', ':', ';', ')', '"', '\''];

/// The hosts a failure may name in full: the project's own, which stand in a message only
/// because the window put them there. Every other host in a message is a machine on the
/// user's own network, and the report states none of them.
const OURS: [&str; 3] = ["encrust.app", "github.com", "objects.githubusercontent.com"];

/// A message as the report states it, on one line: a path or a file name in it is cut to
/// its extension, because the name is the user's and the extension is what the bug needs,
/// and an address or host name is cut away altogether, because it is the user's network.
pub(super) fn scrubbed(message: &str, hosts: &[String]) -> String {
    message
        .split_whitespace()
        .map(|word| scrubbed_word(word, hosts))
        .collect::<Vec<_>>()
        .join(" ")
}

fn scrubbed_word(word: &str, hosts: &[String]) -> String {
    let kept = word.trim_end_matches(TRAILING);
    let tail = &word[kept.len()..];
    // A URL keeps only its scheme unless it points at the project: the authority is a
    // printer on the user's network more often than not, and the path can carry a name.
    if let Some((scheme, rest)) = kept.split_once("://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        return if ours(authority) {
            word.to_owned()
        } else {
            format!("{scheme}://...{tail}")
        };
    }
    if is_host(kept, hosts) {
        return format!("...{tail}");
    }
    let extension = Path::new(kept)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let path = kept.contains('/') || kept.contains('\\');
    let cut = match (path, extension) {
        (false, None) => return word.to_owned(),
        (false, Some(extension)) if !is_ours(&extension) => return word.to_owned(),
        (false, Some(extension)) => format!("*.{extension}"),
        (true, Some(extension)) => format!(".../*.{extension}"),
        (true, None) => ".../*".to_owned(),
    };
    format!("{cut}{tail}")
}

/// Whether an authority belongs to the project itself, credentials and port aside.
fn ours(authority: &str) -> bool {
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let name = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    OURS.contains(&name.as_str())
}

/// Whether a bare word names a machine rather than says something: an address of any
/// shape, an mDNS name, or one of the addresses the window knows a printer by. All three
/// are the user's own network, and a name such as `ada-macbook.local` is the user. A
/// four-part version number is cut along with the addresses it cannot be told apart from;
/// the version that matters is stated on its own line above.
fn is_host(word: &str, hosts: &[String]) -> bool {
    let bare = word.trim_start_matches('[');
    let bare = bare.split_once(']').map_or(bare, |(inside, _)| inside);
    if bare.parse::<IpAddr>().is_ok() {
        return true;
    }
    let named = bare.split(':').next().unwrap_or(bare);
    if named.parse::<IpAddr>().is_ok() || named.to_ascii_lowercase().ends_with(".local") {
        return true;
    }
    hosts.iter().any(|host| {
        let host = host.trim();
        !host.is_empty() && (host.eq_ignore_ascii_case(word) || host.eq_ignore_ascii_case(named))
    })
}

/// Whether an extension is one of the files the window opens or writes, which is what
/// makes a bare word a file name rather than a version or a number.
fn is_ours(extension: &str) -> bool {
    let mut known = crate::files::MESHES
        .iter()
        .chain(&crate::files::PROJECTS)
        .chain(&crate::sliced::EXTENSIONS);
    extension == "toml" || known.any(|known| *known == extension)
}
