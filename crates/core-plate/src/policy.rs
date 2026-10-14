//! The one rule an orientation is chosen by, whether the readings are predicted off the
//! mesh or measured by printing it, and whatever the goal: what fails the print first,
//! then what is left hanging, then the goal's own reading within a band, then a
//! tie-break, and the model stays as it stands unless a turn is worth it. See
//! `docs/design/orientation.md`.

/// What one way to stand reads on everything the choice weighs; lower is better in each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Readings {
    /// What would lose the print outright, weighted by how surely; compared exactly.
    pub fatal: f64,
    /// Islands and contacts left with nothing under them, where the stage measured them.
    pub unheld: Option<f64>,
    /// The goal's own reading.
    pub primary: f64,
    /// What decides between candidates the goal reads alike.
    pub tiebreak: f64,
    /// Whether this is the way the model stands now, which a turn has to be worth. Where
    /// several are, set on the plate and on supports, the first of them is.
    pub standing: bool,
}

/// How far over the best still counts as as good: a share of the best, and a slack
/// beyond it for a reading that is zero at best.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Band {
    share: f64,
    slack: f64,
}

impl Band {
    fn limit(self, best: f64) -> f64 {
        best + self.share * best.abs() + self.slack
    }
}

/// How candidates are ranked.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Policy {
    unheld: Band,
    primary: Band,
    /// The share of its own reading a turn has to win over standing still.
    stay_gain: f64,
}

/// Islands and hanging contacts within a quarter and two of the fewest are the grain of
/// the support grid, not a difference between poses.
const UNHELD: Band = Band {
    share: 0.25,
    slack: 2.0,
};
/// The goal's own reading within a tenth of the best is as good as the best.
const PRIMARY_SHARE: f64 = 0.10;
/// A turn has to win a twentieth of the goal's own reading over standing still.
const STAY_GAIN: f64 = 0.05;

/// The rankings' common ground: the fewest fatal readings, the fewest unheld among them,
/// and the best primary among those.
struct Bests {
    fatal: f64,
    unheld: Option<f64>,
    primary: f64,
}

impl Bests {
    fn of(readings: &[Readings], policy: &Policy) -> Option<Self> {
        let fatal = readings.iter().map(|r| r.fatal).min_by(f64::total_cmp)?;
        let unheld = readings
            .iter()
            .filter(|r| r.fatal.total_cmp(&fatal).is_eq())
            .filter_map(|r| r.unheld)
            .min_by(f64::total_cmp);
        let mut bests = Self {
            fatal,
            unheld,
            primary: f64::INFINITY,
        };
        bests.primary = readings
            .iter()
            .filter(|r| bests.first_tier(r, policy))
            .map(|r| r.primary)
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY);
        Some(bests)
    }

    fn held(&self, r: &Readings, policy: &Policy) -> bool {
        match (r.unheld, self.unheld) {
            (Some(own), Some(best)) => own <= policy.unheld.limit(best),
            _ => true,
        }
    }

    fn first_tier(&self, r: &Readings, policy: &Policy) -> bool {
        r.fatal.total_cmp(&self.fatal).is_eq() && self.held(r, policy)
    }

    fn in_band(&self, r: &Readings, policy: &Policy) -> bool {
        self.first_tier(r, policy) && r.primary <= policy.primary.limit(self.primary)
    }

    /// The order key: fatal, then held, then in band, then the tie-break.
    fn key(&self, r: &Readings, policy: &Policy) -> (f64, bool, bool, f64) {
        (
            r.fatal,
            !self.held(r, policy),
            !self.in_band(r, policy),
            r.tiebreak,
        )
    }
}

fn compare(a: (f64, bool, bool, f64), b: (f64, bool, bool, f64)) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then(a.1.cmp(&b.1))
        .then(a.2.cmp(&b.2))
        .then(a.3.total_cmp(&b.3))
}

/// The indices of `readings`, best first, with the way the model stands moved to the front
/// when no turn ahead of it is worth making. Ties keep their order. `primary_slack` is what
/// of the primary's own unit is noise: a stage reads it in contacts, newtons or a
/// dimensionless share, and only it knows which.
pub fn rank(readings: &[Readings], primary_slack: f64) -> Vec<usize> {
    let policy = &Policy {
        unheld: UNHELD,
        primary: Band {
            share: PRIMARY_SHARE,
            slack: primary_slack,
        },
        stay_gain: STAY_GAIN,
    };
    let Some(bests) = Bests::of(readings, policy) else {
        return Vec::new();
    };
    let key = |at: &usize| bests.key(&readings[*at], policy);
    let mut order: Vec<usize> = (0..readings.len()).collect();
    order.sort_by(|a, b| compare(key(a), key(b)));

    let first_standing = readings.iter().position(|r| r.standing);
    let standing = order.iter().position(|at| Some(*at) == first_standing);
    if let Some(position) = standing.filter(|position| *position > 0)
        && !worth_turning(
            &bests,
            policy,
            &readings[order[0]],
            &readings[order[position]],
        )
    {
        let now = order.remove(position);
        order.insert(0, now);
    }
    order
}

/// Whether `turned`, ranked ahead of `now`, is worth leaving the way the model stands for:
/// it fails less, standing is out of the band, or it wins `stay_gain` of the goal's own
/// reading.
fn worth_turning(bests: &Bests, policy: &Policy, turned: &Readings, now: &Readings) -> bool {
    let key = |r: &Readings| bests.key(r, policy);
    if compare(key(turned), key(now)).is_ge() {
        return false;
    }
    let fails_less = (turned.fatal, !bests.held(turned, policy))
        .partial_cmp(&(now.fatal, !bests.held(now, policy)))
        .is_some_and(std::cmp::Ordering::is_lt);
    let gain = now.primary - turned.primary;
    fails_less || !bests.in_band(now, policy) || gain > policy.stay_gain * now.primary.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLACK: f64 = 0.0;

    fn reading(fatal: f64, unheld: Option<f64>, primary: f64, tiebreak: f64) -> Readings {
        Readings {
            fatal,
            unheld,
            primary,
            tiebreak,
            standing: false,
        }
    }

    #[test]
    fn what_fails_the_print_loses_whatever_it_reads() {
        let readings = [reading(1.0, None, 1.0, 0.0), reading(0.0, None, 100.0, 1.0)];
        assert_eq!(rank(&readings, SLACK)[0], 1);
    }

    #[test]
    fn what_is_left_hanging_past_the_band_loses_to_what_is_held() {
        let readings = [
            reading(0.0, Some(12.0), 10.0, 0.0),
            reading(0.0, Some(3.0), 50.0, 1.0),
        ];
        assert_eq!(rank(&readings, SLACK)[0], 1, "3 * 1.25 + 2 is under 12");
    }

    #[test]
    fn within_the_band_the_tiebreak_decides() {
        let readings = [
            reading(0.0, None, 100.0, 5.0),
            reading(0.0, None, 105.0, 1.0),
            reading(0.0, None, 120.0, 0.0),
        ];
        assert_eq!(rank(&readings, SLACK), [1, 0, 2]);
    }

    #[test]
    fn standing_still_wins_over_a_turn_that_gains_too_little() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 98.0, 0.0);
        assert_eq!(
            rank(&[slightly, now], SLACK)[0],
            1,
            "2 % is not worth a turn"
        );
        let clearly = reading(0.0, None, 80.0, 0.0);
        assert_eq!(rank(&[clearly, now], SLACK)[0], 0, "20 % is");
    }

    #[test]
    fn a_turn_that_fails_less_is_always_worth_it() {
        let mut now = reading(1.0, None, 10.0, 1.0);
        now.standing = true;
        let sound = reading(0.0, None, 10.0, 0.0);
        assert_eq!(rank(&[now, sound], SLACK)[0], 1);
    }

    #[test]
    fn of_two_ways_of_standing_still_the_first_is_what_a_turn_is_weighed_against() {
        let mut supported = reading(0.0, None, 46.0, 0.0);
        supported.standing = true;
        let turned = reading(0.0, None, 0.0, 1.0);
        let mut bare = reading(0.0, None, 0.0, 2.0);
        bare.standing = true;
        assert_eq!(
            rank(&[supported, turned, bare], SLACK)[0],
            1,
            "the turn wins on the first way of standing, whatever the second reads"
        );
    }

    #[test]
    fn nothing_to_rank_ranks_nothing() {
        assert!(rank(&[], SLACK).is_empty());
    }
}
