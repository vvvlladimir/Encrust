use printer_profiles::SupportProfile;

use super::{Refused, carried, eased, followed};
use crate::group::Profiles;
use crate::pick::Part;
use crate::placed::Placed;
use crate::tree::SupportTree;

/// What a carry moves on one model: each tree with its index, and why some may not stand.
#[derive(Debug, Clone, Default)]
pub struct Carried {
    pub moved: Vec<(usize, SupportTree)>,
    pub refused: Option<Refused>,
}

/// The trees of `standing` (model space) held, each its index with its parts, carried by `step`
/// (plate mm) and eased, those on them seated on them as eased; `None` where a column has
/// nothing to stand on. `last` is what the previous step moved, which a foot keeps to
/// (ADR 0440, 0441).
pub fn carried_held(
    standing: &[SupportTree],
    last: &[(usize, SupportTree)],
    held: &[(usize, Vec<Part>)],
    step: core_geometry::Vec3,
    placed: &Placed,
    profiles: Profiles,
) -> Option<Carried> {
    let mut out = Carried::default();
    for (index, parts) in held {
        let was = standing.get(*index)?;
        let profile = profiles.of(was.group());
        let stood = last
            .iter()
            .find(|(taken, _)| taken == index)
            .map_or_else(|| was.landing(), |(_, tree)| tree.landing());
        let now = carried(was, stood, parts, step, placed, profile)?;
        let now = out.ease(was, now, placed, profile);
        out.moved.push((*index, now));
    }
    let hosts = out.moved.clone();
    for (host, now) in &hosts {
        // A rider is seated on the host as it stands once eased, or it hangs where it was.
        for (rider, seated) in followed(standing, *host, now, placed, profiles)?
            .into_iter()
            .skip(1)
        {
            if out.moved.iter().all(|(taken, _)| *taken != rider) {
                let was = &standing[rider];
                let seated = out.ease(was, seated, placed, profiles.of(was.group()));
                out.moved.push((rider, seated));
            }
        }
    }
    Some(out)
}

impl Carried {
    /// `now`, carried from `was`, eased where it may be; else as it is, its refusal kept.
    fn ease(
        &mut self,
        was: &SupportTree,
        now: SupportTree,
        placed: &Placed,
        profile: &SupportProfile,
    ) -> SupportTree {
        match eased(was, &now, placed, profile) {
            Ok(eased) => eased,
            Err(why) => {
                self.refused.get_or_insert(why);
                now
            }
        }
    }
}

#[cfg(test)]
mod tests;
