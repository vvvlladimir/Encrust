//! The one rule an orientation is chosen by, whether the readings are predicted off the
//! mesh or measured by printing it, and whatever the goal: what fails the print first,
//! then what is left hanging, then the goal's own reading within a band, then a
//! tie-break, and the model stays as it stands unless a turn is worth it. The fewest
//! supports weighs only what fails and its own reading, with no band. See
//! `docs/design/orientation.md`.

use crate::score::OrientGoal;

/// What one way to stand reads on everything the choice weighs; lower is better in each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Readings {
    /// What would lose the print outright whatever the goal, weighted by how surely: a
    /// pose past the build volume, or balanced on the plate on a point.
    pub fatal: f64,
    /// Suction cups and pulls past what the film lets go of, weighted the same way: fatal
    /// to every goal but the fewest supports, which leaves them to a drain hole.
    pub suction: f64,
    /// Islands left with nothing under them, where the stage measured them; the fewest
    /// supports does not weigh them.
    pub unheld: Option<f64>,
    /// Contacts placement wanted and found no room for a column under: left hanging for
    /// every goal but the fewest supports, which takes them as placement leaves them.
    pub roomless: f64,
    /// The goal's own reading.
    pub primary: f64,
    /// What decides between candidates the goal reads alike, compared first to last.
    pub tiebreak: [f64; 2],
    /// Whether this is the way the model stands now, which a turn has to be worth. Where
    /// several are, set on the plate and on supports, the best ranked of them is.
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

/// How candidates are ranked for one goal.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Policy {
    /// Whether suction counts as failing the print, islands and contacts with no room as
    /// hanging, and the goal's reading within a band; without, the least reading wins.
    strict: bool,
    unheld: Band,
    primary: Band,
    /// The share of its own reading a turn has to win over standing still.
    stay_gain: f64,
}

impl Policy {
    fn fatal(&self, r: &Readings) -> f64 {
        r.fatal + if self.strict { r.suction } else { 0.0 }
    }

    /// What is left hanging, where the stage measured it and the goal weighs it.
    fn left(&self, r: &Readings) -> Option<f64> {
        r.unheld
            .filter(|_| self.strict)
            .map(|islands| islands + r.roomless)
    }

    /// The goal's own reading where it ranks ahead of what is left hanging and the
    /// tie-break, which is where it has no band.
    fn least(&self, r: &Readings) -> f64 {
        if self.strict { 0.0 } else { r.primary }
    }

    fn of(goal: OrientGoal, primary_slack: f64) -> Self {
        if goal == OrientGoal::FewestSupports {
            // The least support volume, whatever else it reads (ADR 0279).
            let exact = Band {
                share: 0.0,
                slack: 0.0,
            };
            return Self {
                strict: false,
                unheld: exact,
                primary: exact,
                stay_gain: 0.0,
            };
        }
        Self {
            strict: true,
            unheld: UNHELD,
            primary: Band {
                share: PRIMARY_SHARE,
                slack: primary_slack,
            },
            stay_gain: STAY_GAIN,
        }
    }
}

/// Islands and hanging contacts within a quarter and one of the fewest are the grain of
/// the support grid; one and not two, as a coarse stack undercounts islands (ADR 0279).
const UNHELD: Band = Band {
    share: 0.25,
    slack: 1.0,
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
        let fatal = readings
            .iter()
            .map(|r| policy.fatal(r))
            .min_by(f64::total_cmp)?;
        let unheld = readings
            .iter()
            .filter(|r| policy.fatal(r).total_cmp(&fatal).is_eq())
            .filter_map(|r| policy.left(r))
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
        match (policy.left(r), self.unheld) {
            (Some(own), Some(best)) => own <= policy.unheld.limit(best),
            _ => true,
        }
    }

    fn first_tier(&self, r: &Readings, policy: &Policy) -> bool {
        policy.fatal(r).total_cmp(&self.fatal).is_eq() && self.held(r, policy)
    }

    fn in_band(&self, r: &Readings, policy: &Policy) -> bool {
        self.first_tier(r, policy) && r.primary <= policy.primary.limit(self.primary)
    }

    /// The order key: fatal, then held, then in band, then the fewest unheld, then the
    /// least reading where the goal has no band, then the tie-break.
    fn key(&self, r: &Readings, policy: &Policy) -> Key {
        (
            policy.fatal(r),
            !self.held(r, policy),
            !self.in_band(r, policy),
            policy.left(r).unwrap_or(0.0),
            policy.least(r),
            r.tiebreak,
        )
    }
}

type Key = (f64, bool, bool, f64, f64, [f64; 2]);

fn compare(a: Key, b: Key) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then(a.1.cmp(&b.1))
        .then(a.2.cmp(&b.2))
        .then(a.3.total_cmp(&b.3))
        .then(a.4.total_cmp(&b.4))
        .then(a.5[0].total_cmp(&b.5[0]))
        .then(a.5[1].total_cmp(&b.5[1]))
}

/// The indices of `readings`, best first, with the way the model stands moved to the front
/// when no turn ahead of it is worth making. Ties keep their order. `primary_slack` is what
/// of the primary's own unit is noise: a stage reads it in contacts, millilitres, newtons
/// or a dimensionless share, and only it knows which.
pub fn rank(readings: &[Readings], goal: OrientGoal, primary_slack: f64) -> Vec<usize> {
    ranking(readings, goal, primary_slack).map_or_else(Vec::new, |ranking| ranking.order)
}

/// Why each of `readings` ended where [`rank`] put it, in the order given.
pub fn rulings(readings: &[Readings], goal: OrientGoal, primary_slack: f64) -> Vec<Ruling> {
    let Some(ranking) = ranking(readings, goal, primary_slack) else {
        return Vec::new();
    };
    let mut rulings = vec![Ruling::TieBreak; readings.len()];
    for (place, at) in ranking.order.iter().enumerate() {
        rulings[*at] = ranking.ruling(&readings[*at], place);
    }
    rulings
}

/// What decided a candidate's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ruling {
    /// Ranked first.
    Chosen,
    /// Ranked first as the way the model stands, no turn ahead of it being worth making.
    Stayed,
    /// Ranked first and turned away from, but not worth the turn.
    NotWorthTurning,
    /// Would fail the print more surely than the best.
    Fatal,
    /// Leaves more hanging than the band over the fewest allows.
    Unheld,
    /// Its goal's own reading is past the band over the best.
    OutOfBand,
    /// As good as the best, and behind it on the tie-break.
    TieBreak,
}

/// The policy, the bests it measured, the order, and whether standing still was moved ahead.
struct Ranking {
    policy: Policy,
    bests: Bests,
    order: Vec<usize>,
    stayed: bool,
}

impl Ranking {
    fn ruling(&self, r: &Readings, place: usize) -> Ruling {
        let (policy, bests) = (&self.policy, &self.bests);
        match (place, self.stayed) {
            (0, true) => Ruling::Stayed,
            (0, false) => Ruling::Chosen,
            (1, true) => Ruling::NotWorthTurning,
            _ if policy.fatal(r).total_cmp(&bests.fatal).is_gt() => Ruling::Fatal,
            _ if !bests.held(r, policy) => Ruling::Unheld,
            _ if !bests.in_band(r, policy) => Ruling::OutOfBand,
            _ => Ruling::TieBreak,
        }
    }
}

fn ranking(readings: &[Readings], goal: OrientGoal, primary_slack: f64) -> Option<Ranking> {
    let policy = Policy::of(goal, primary_slack);
    let bests = Bests::of(readings, &policy)?;
    let key = |at: &usize| bests.key(&readings[*at], &policy);
    let mut order: Vec<usize> = (0..readings.len()).collect();
    order.sort_by(|a, b| compare(key(a), key(b)));

    let standing = order.iter().position(|at| readings[*at].standing);
    let stayed = standing
        .filter(|position| *position > 0)
        .is_some_and(|position| {
            !worth_turning(
                &bests,
                &policy,
                &readings[order[0]],
                &readings[order[position]],
            )
        });
    if let (true, Some(position)) = (stayed, standing) {
        let now = order.remove(position);
        order.insert(0, now);
    }
    Some(Ranking {
        policy,
        bests,
        order,
        stayed,
    })
}

/// Whether `turned`, ranked ahead of `now`, is worth leaving the way the model stands for:
/// it fails less, standing is out of the band, or it wins `stay_gain` of the goal's own
/// reading and more than the noise of its unit.
fn worth_turning(bests: &Bests, policy: &Policy, turned: &Readings, now: &Readings) -> bool {
    let key = |r: &Readings| bests.key(r, policy);
    if compare(key(turned), key(now)).is_ge() {
        return false;
    }
    let fails_less = (policy.fatal(turned), !bests.held(turned, policy))
        .partial_cmp(&(policy.fatal(now), !bests.held(now, policy)))
        .is_some_and(std::cmp::Ordering::is_lt);
    let gain = now.primary - turned.primary;
    let worth = (policy.stay_gain * now.primary.abs()).max(policy.primary.slack);
    fails_less || !bests.in_band(now, policy) || gain > worth
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLACK: f64 = 0.0;
    /// A goal ranked with its bands; the fewest supports has none.
    const GOAL: OrientGoal = OrientGoal::LeastPeel;
    const FEWEST: OrientGoal = OrientGoal::FewestSupports;

    fn reading(fatal: f64, unheld: Option<f64>, primary: f64, tiebreak: f64) -> Readings {
        Readings {
            fatal,
            unheld,
            primary,
            suction: 0.0,
            roomless: 0.0,
            tiebreak: [tiebreak, 0.0],
            standing: false,
        }
    }

    #[test]
    fn what_fails_the_print_loses_whatever_it_reads() {
        let readings = [reading(1.0, None, 1.0, 0.0), reading(0.0, None, 100.0, 1.0)];
        assert_eq!(rank(&readings, GOAL, SLACK)[0], 1);
        assert_eq!(rank(&readings, FEWEST, SLACK)[0], 1);
    }

    #[test]
    fn what_is_left_hanging_past_the_band_loses_to_what_is_held() {
        let readings = [
            reading(0.0, Some(12.0), 10.0, 0.0),
            reading(0.0, Some(3.0), 50.0, 1.0),
        ];
        assert_eq!(
            rank(&readings, GOAL, SLACK)[0],
            1,
            "3 * 1.25 + 1 is under 12"
        );
    }

    #[test]
    fn within_the_band_the_tiebreak_decides() {
        let readings = [
            reading(0.0, None, 100.0, 5.0),
            reading(0.0, None, 105.0, 1.0),
            reading(0.0, None, 120.0, 0.0),
        ];
        assert_eq!(rank(&readings, GOAL, SLACK), [1, 0, 2]);
    }

    #[test]
    fn within_the_band_the_fewest_left_hanging_come_first() {
        let readings = [
            reading(0.0, Some(3.0), 100.0, 0.0),
            reading(0.0, Some(1.0), 105.0, 1.0),
        ];
        assert_eq!(rank(&readings, GOAL, SLACK)[0], 1);
    }

    fn sucking(primary: f64) -> Readings {
        Readings {
            suction: 1.0,
            ..reading(0.0, None, primary, 0.0)
        }
    }

    #[test]
    fn the_fewest_supports_take_the_least_support_whatever_it_cups() {
        let sound = reading(0.0, None, 90.0, 1.0);
        assert_eq!(rank(&[sucking(60.0), sound], FEWEST, SLACK)[0], 0);
    }

    #[test]
    fn contacts_with_no_room_hang_for_every_goal_but_the_fewest_supports() {
        let roomless = Readings {
            roomless: 40.0,
            ..reading(0.0, Some(0.0), 60.0, 0.0)
        };
        let roomy = reading(0.0, Some(0.0), 90.0, 1.0);
        assert_eq!(rank(&[roomless, roomy], FEWEST, SLACK)[0], 0);
        assert_eq!(rank(&[roomless, roomy], OrientGoal::LeastPeel, SLACK)[0], 1);
    }

    #[test]
    fn other_goals_refuse_a_cup_whatever_it_saves() {
        let sound = reading(0.0, None, 90.0, 1.0);
        let peel = OrientGoal::LeastPeel;
        assert_eq!(rank(&[sucking(60.0), sound], peel, SLACK)[0], 1);
        assert_eq!(
            rulings(&[sucking(60.0), sound], peel, SLACK),
            [Ruling::Fatal, Ruling::Chosen]
        );
    }

    #[test]
    fn the_fewest_supports_take_the_least_support_whatever_is_left_hanging() {
        let loose = reading(0.0, Some(12.0), 10.0, 1.0);
        let held = reading(0.0, Some(0.0), 11.0, 0.0);
        assert_eq!(rank(&[loose, held], FEWEST, SLACK)[0], 0);
    }

    #[test]
    fn the_fewest_supports_have_no_band_over_the_least() {
        let readings = [
            reading(0.0, None, 105.0, 0.0),
            reading(0.0, None, 100.0, 1.0),
            reading(0.0, None, 101.0, 2.0),
        ];
        assert_eq!(rank(&readings, FEWEST, 0.5), [1, 2, 0]);
    }

    #[test]
    fn the_fewest_supports_turn_for_any_less_support() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 99.0, 0.0);
        assert_eq!(
            rank(&[slightly, now], FEWEST, 0.5)[0],
            0,
            "1 % less is less"
        );
        let same = reading(0.0, None, 100.0, 0.0);
        assert_eq!(rank(&[same, now], FEWEST, 0.5)[0], 1, "as little stays");
    }

    #[test]
    fn a_turn_has_to_win_more_than_the_noise_of_its_unit() {
        let mut now = reading(0.0, None, 0.5, 1.0);
        now.standing = true;
        let turned = reading(0.0, None, 0.0, 0.0);
        assert_eq!(
            rank(&[turned, now], GOAL, 0.5)[0],
            1,
            "half a newton is the noise of a newton reading"
        );
    }

    #[test]
    fn standing_still_wins_over_a_turn_that_gains_too_little() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 98.0, 0.0);
        assert_eq!(
            rank(&[slightly, now], GOAL, SLACK)[0],
            1,
            "2 % is not worth a turn"
        );
        let clearly = reading(0.0, None, 80.0, 0.0);
        assert_eq!(rank(&[clearly, now], GOAL, SLACK)[0], 0, "20 % is");
    }

    #[test]
    fn a_turn_that_fails_less_is_always_worth_it() {
        let mut now = reading(1.0, None, 10.0, 1.0);
        now.standing = true;
        let sound = reading(0.0, None, 10.0, 0.0);
        assert_eq!(rank(&[now, sound], GOAL, SLACK)[0], 1);
    }

    #[test]
    fn of_two_ways_of_standing_still_the_better_is_what_a_turn_is_weighed_against() {
        let mut supported = reading(0.0, None, 46.0, 0.0);
        supported.standing = true;
        let turned = reading(0.0, None, 0.0, 1.0);
        let mut bare = reading(0.0, None, 0.0, 2.0);
        bare.standing = true;
        assert_eq!(
            rank(&[supported, turned, bare], GOAL, SLACK)[0],
            2,
            "set bare as it stands, nothing is left for a turn to win"
        );
    }

    #[test]
    fn nothing_to_rank_ranks_nothing() {
        assert!(rank(&[], GOAL, SLACK).is_empty());
        assert!(rulings(&[], GOAL, SLACK).is_empty());
    }

    #[test]
    fn each_place_is_ruled_by_the_first_thing_it_lost_on() {
        let readings = [
            reading(1.0, Some(0.0), 10.0, 0.0),
            reading(0.0, Some(20.0), 10.0, 1.0),
            reading(0.0, Some(0.0), 50.0, 2.0),
            reading(0.0, Some(0.0), 10.0, 4.0),
            reading(0.0, Some(0.0), 10.0, 3.0),
        ];
        assert_eq!(
            rulings(&readings, GOAL, SLACK),
            [
                Ruling::Fatal,
                Ruling::Unheld,
                Ruling::OutOfBand,
                Ruling::TieBreak,
                Ruling::Chosen
            ]
        );
    }

    #[test]
    fn standing_still_is_ruled_stayed_and_the_turn_it_beat_not_worth_it() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 98.0, 0.0);
        assert_eq!(
            rulings(&[slightly, now], GOAL, SLACK),
            [Ruling::NotWorthTurning, Ruling::Stayed]
        );
    }
}
