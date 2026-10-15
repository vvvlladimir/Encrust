//! The one rule an orientation is chosen by, whatever the stage and the goal; each goal
//! picks its own [`Policy`]. See `docs/design/orientation.md`.

use core_analysis::PEEL_LIMIT_N;

use crate::score::OrientGoal;

/// What one way to stand reads on everything the choice weighs; lower is better in each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Readings {
    /// What would lose the print outright whatever the goal, weighted by how surely: a
    /// pose past the build volume, or balanced on the plate on a point.
    pub fatal: f64,
    /// Suction cups and pulls past what the film lets go of, weighted the same way: fatal
    /// to a goal whose policy weighs hazards.
    pub suction: f64,
    /// Islands left with nothing under them, where the stage measured them.
    pub unheld: Option<f64>,
    /// Contacts placement wanted and found no room for a column under, counted with the
    /// islands as left hanging.
    pub roomless: f64,
    /// Every goal's own reading, in the order of [`OrientGoal::ALL`]: what a policy takes
    /// its goal's reading and its fallback's from.
    pub goals: [f64; OrientGoal::ALL.len()],
    /// What decides between candidates the goal reads alike, compared first to last.
    pub tiebreak: [f64; 2],
    /// Whether this is the way the model stands now, which a turn has to be worth. Where
    /// several are, set on the plate and on supports, the best ranked of them is.
    pub standing: bool,
}

/// What of each goal's reading is noise, in the order of [`OrientGoal::ALL`] and the unit a
/// stage reads it in: contacts, millilitres, newtons or a dimensionless share.
pub type Noise = [f64; OrientGoal::ALL.len()];

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
    goal: OrientGoal,
    /// Whether suction counts as failing the print, islands and contacts with no room as
    /// hanging, and the goal's reading within a band; without, the least reading wins.
    weighs_hazards: bool,
    unheld: Band,
    primary: Band,
    /// The goal's reading at or under which every candidate reads alike, having nothing
    /// left to win.
    enough: f64,
    /// The goal whose reading decides between candidates that have enough, and what of that
    /// reading is noise.
    fallback: Option<OrientGoal>,
    fallback_noise: f64,
    /// The share of its own reading a turn has to win over standing still.
    stay_gain: f64,
}

impl Policy {
    /// The goal's own reading, no lower than enough.
    fn primary(&self, r: &Readings) -> f64 {
        r.goals[self.goal.index()].max(self.enough)
    }

    /// The fallback's reading where the goal has enough, ahead of anything that has not.
    fn fallback(&self, r: &Readings) -> f64 {
        match self.fallback {
            Some(goal) if self.primary(r) <= self.enough => r.goals[goal.index()],
            Some(_) => f64::INFINITY,
            None => 0.0,
        }
    }

    fn fatal(&self, r: &Readings) -> f64 {
        r.fatal + if self.weighs_hazards { r.suction } else { 0.0 }
    }

    /// What is left hanging, where the stage measured it and the goal weighs it.
    fn left(&self, r: &Readings) -> Option<f64> {
        r.unheld
            .filter(|_| self.weighs_hazards)
            .map(|islands| islands + r.roomless)
    }

    /// The goal's own reading where it ranks ahead of what is left hanging and the
    /// tie-break, which is where it has no band.
    fn least(&self, r: &Readings) -> f64 {
        if self.weighs_hazards {
            0.0
        } else {
            self.primary(r)
        }
    }

    /// Each goal's own policy: a goal is ranked by nothing another goal sets.
    fn of(goal: OrientGoal, noise: &Noise) -> Self {
        let primary_slack = noise[goal.index()];
        match goal {
            // The least support volume, whatever else it reads (ADR 0279).
            OrientGoal::FewestSupports => Self::strictly_least(goal),
            OrientGoal::LeastPeel => Self {
                enough: ENOUGH_PULL_N,
                fallback: Some(OrientGoal::FewestSupports),
                fallback_noise: noise[OrientGoal::FewestSupports.index()],
                ..Self::banded(goal, primary_slack)
            },
            OrientGoal::ShortestPrint | OrientGoal::CleanTop => Self::banded(goal, primary_slack),
        }
    }

    /// Strictly the least reading past what fails every goal, and any turn that lowers it.
    fn strictly_least(goal: OrientGoal) -> Self {
        let exact = Band {
            share: 0.0,
            slack: 0.0,
        };
        Self {
            goal,
            weighs_hazards: false,
            unheld: exact,
            primary: exact,
            enough: f64::NEG_INFINITY,
            fallback: None,
            fallback_noise: 0.0,
            stay_gain: 0.0,
        }
    }

    /// Nothing failing or hanging, the reading within a band, and a turn worth its gain.
    fn banded(goal: OrientGoal, primary_slack: f64) -> Self {
        Self {
            goal,
            weighs_hazards: true,
            unheld: UNHELD,
            primary: Band {
                share: PRIMARY_SHARE,
                slack: primary_slack,
            },
            enough: f64::NEG_INFINITY,
            fallback: None,
            fallback_noise: 0.0,
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
/// A pull under a third of the limit is no longer worth a turn or a support: the scale of
/// the pull is known to about a factor of three (`docs/design/analysis.md`).
const ENOUGH_PULL_N: f64 = PEEL_LIMIT_N / 3.0;

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
            .map(|r| policy.primary(r))
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
        self.first_tier(r, policy) && policy.primary(r) <= policy.primary.limit(self.primary)
    }

    /// The order key: fatal, then held, then in band, then the fewest unheld, then the
    /// least reading where the goal has no band, then the fallback, then the tie-break.
    fn key(&self, r: &Readings, policy: &Policy) -> Key {
        (
            policy.fatal(r),
            !self.held(r, policy),
            !self.in_band(r, policy),
            policy.left(r).unwrap_or(0.0),
            policy.least(r),
            policy.fallback(r),
            r.tiebreak,
        )
    }
}

type Key = (f64, bool, bool, f64, f64, f64, [f64; 2]);

fn compare(a: Key, b: Key) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then(a.1.cmp(&b.1))
        .then(a.2.cmp(&b.2))
        .then(a.3.total_cmp(&b.3))
        .then(a.4.total_cmp(&b.4))
        .then(a.5.total_cmp(&b.5))
        .then(a.6[0].total_cmp(&b.6[0]))
        .then(a.6[1].total_cmp(&b.6[1]))
}

/// The indices of `readings`, best first, with the way the model stands moved to the front
/// when no turn ahead of it is worth making. Ties keep their order. `noise` is the stage's
/// own: only it knows the unit it reads each goal in.
pub fn rank(readings: &[Readings], goal: OrientGoal, noise: &Noise) -> Vec<usize> {
    ranking(readings, goal, noise).map_or_else(Vec::new, |ranking| ranking.order)
}

/// Why each of `readings` ended where [`rank`] put it, in the order given.
pub fn rulings(readings: &[Readings], goal: OrientGoal, noise: &Noise) -> Vec<Ruling> {
    let Some(ranking) = ranking(readings, goal, noise) else {
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

fn ranking(readings: &[Readings], goal: OrientGoal, noise: &Noise) -> Option<Ranking> {
    let policy = Policy::of(goal, noise);
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
/// reading and more than the noise of its unit — or, both having enough, as much of the
/// fallback's.
fn worth_turning(bests: &Bests, policy: &Policy, turned: &Readings, now: &Readings) -> bool {
    let key = |r: &Readings| bests.key(r, policy);
    if compare(key(turned), key(now)).is_ge() {
        return false;
    }
    let fails_less = (policy.fatal(turned), !bests.held(turned, policy))
        .partial_cmp(&(policy.fatal(now), !bests.held(now, policy)))
        .is_some_and(std::cmp::Ordering::is_lt);
    let gain = policy.primary(now) - policy.primary(turned);
    let worth = (policy.stay_gain * policy.primary(now).abs()).max(policy.primary.slack);
    let both_enough = policy.primary(now).max(policy.primary(turned)) <= policy.enough;
    let spared = policy.fallback(now) - policy.fallback(turned);
    let spared_worth = both_enough
        && spared > (policy.stay_gain * policy.fallback(now).abs()).max(policy.fallback_noise);
    fails_less || !bests.in_band(now, policy) || gain > worth || spared_worth
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLACK: Noise = [0.0; OrientGoal::ALL.len()];
    /// Half a newton of pull and a twentieth of a millilitre of support, as a proof reads.
    const PROVEN: Noise = [0.05, 0.5, 5.0, 2.0];
    /// A goal ranked with its bands and no floor; the fewest supports has neither.
    const GOAL: OrientGoal = OrientGoal::ShortestPrint;
    const PEEL: OrientGoal = OrientGoal::LeastPeel;
    const FEWEST: OrientGoal = OrientGoal::FewestSupports;

    fn reading(fatal: f64, unheld: Option<f64>, primary: f64, tiebreak: f64) -> Readings {
        Readings {
            fatal,
            unheld,
            goals: [primary; OrientGoal::ALL.len()],
            suction: 0.0,
            roomless: 0.0,
            tiebreak: [tiebreak, 0.0],
            standing: false,
        }
    }

    #[test]
    fn what_fails_the_print_loses_whatever_it_reads() {
        let readings = [reading(1.0, None, 1.0, 0.0), reading(0.0, None, 100.0, 1.0)];
        assert_eq!(rank(&readings, GOAL, &SLACK)[0], 1);
        assert_eq!(rank(&readings, FEWEST, &SLACK)[0], 1);
    }

    #[test]
    fn what_is_left_hanging_past_the_band_loses_to_what_is_held() {
        let readings = [
            reading(0.0, Some(12.0), 10.0, 0.0),
            reading(0.0, Some(3.0), 50.0, 1.0),
        ];
        assert_eq!(
            rank(&readings, GOAL, &SLACK)[0],
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
        assert_eq!(rank(&readings, GOAL, &SLACK), [1, 0, 2]);
    }

    #[test]
    fn within_the_band_the_fewest_left_hanging_come_first() {
        let readings = [
            reading(0.0, Some(3.0), 100.0, 0.0),
            reading(0.0, Some(1.0), 105.0, 1.0),
        ];
        assert_eq!(rank(&readings, GOAL, &SLACK)[0], 1);
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
        assert_eq!(rank(&[sucking(60.0), sound], FEWEST, &SLACK)[0], 0);
    }

    #[test]
    fn contacts_with_no_room_hang_for_every_goal_but_the_fewest_supports() {
        let roomless = Readings {
            roomless: 40.0,
            ..reading(0.0, Some(0.0), 60.0, 0.0)
        };
        let roomy = reading(0.0, Some(0.0), 90.0, 1.0);
        assert_eq!(rank(&[roomless, roomy], FEWEST, &SLACK)[0], 0);
        assert_eq!(
            rank(&[roomless, roomy], OrientGoal::LeastPeel, &SLACK)[0],
            1
        );
    }

    #[test]
    fn other_goals_refuse_a_cup_whatever_it_saves() {
        let sound = reading(0.0, None, 90.0, 1.0);
        let peel = OrientGoal::LeastPeel;
        assert_eq!(rank(&[sucking(60.0), sound], peel, &SLACK)[0], 1);
        assert_eq!(
            rulings(&[sucking(60.0), sound], peel, &SLACK),
            [Ruling::Fatal, Ruling::Chosen]
        );
    }

    #[test]
    fn the_fewest_supports_take_the_least_support_whatever_is_left_hanging() {
        let loose = reading(0.0, Some(12.0), 10.0, 1.0);
        let held = reading(0.0, Some(0.0), 11.0, 0.0);
        assert_eq!(rank(&[loose, held], FEWEST, &SLACK)[0], 0);
    }

    #[test]
    fn the_fewest_supports_have_no_band_over_the_least() {
        let readings = [
            reading(0.0, None, 105.0, 0.0),
            reading(0.0, None, 100.0, 1.0),
            reading(0.0, None, 101.0, 2.0),
        ];
        assert_eq!(rank(&readings, FEWEST, &PROVEN), [1, 2, 0]);
    }

    #[test]
    fn the_fewest_supports_turn_for_any_less_support() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 99.0, 0.0);
        assert_eq!(
            rank(&[slightly, now], FEWEST, &PROVEN)[0],
            0,
            "1 % less is less"
        );
        let same = reading(0.0, None, 100.0, 0.0);
        assert_eq!(rank(&[same, now], FEWEST, &PROVEN)[0], 1, "as little stays");
    }

    #[test]
    fn a_turn_has_to_win_more_than_the_noise_of_its_unit() {
        let mut now = reading(0.0, None, 5.0, 1.0);
        now.standing = true;
        let turned = reading(0.0, None, 0.0, 0.0);
        assert_eq!(
            rank(&[turned, now], GOAL, &PROVEN)[0],
            1,
            "five layers is the noise of a layer count"
        );
    }

    #[test]
    fn standing_still_wins_over_a_turn_that_gains_too_little() {
        let mut now = reading(0.0, None, 100.0, 1.0);
        now.standing = true;
        let slightly = reading(0.0, None, 98.0, 0.0);
        assert_eq!(
            rank(&[slightly, now], GOAL, &SLACK)[0],
            1,
            "2 % is not worth a turn"
        );
        let clearly = reading(0.0, None, 80.0, 0.0);
        assert_eq!(rank(&[clearly, now], GOAL, &SLACK)[0], 0, "20 % is");
    }

    #[test]
    fn a_turn_that_fails_less_is_always_worth_it() {
        let mut now = reading(1.0, None, 10.0, 1.0);
        now.standing = true;
        let sound = reading(0.0, None, 10.0, 0.0);
        assert_eq!(rank(&[now, sound], GOAL, &SLACK)[0], 1);
    }

    #[test]
    fn of_two_ways_of_standing_still_the_better_is_what_a_turn_is_weighed_against() {
        let mut supported = reading(0.0, None, 46.0, 0.0);
        supported.standing = true;
        let turned = reading(0.0, None, 0.0, 1.0);
        let mut bare = reading(0.0, None, 0.0, 2.0);
        bare.standing = true;
        assert_eq!(
            rank(&[supported, turned, bare], GOAL, &SLACK)[0],
            2,
            "set bare as it stands, nothing is left for a turn to win"
        );
    }

    #[test]
    fn nothing_to_rank_ranks_nothing() {
        assert!(rank(&[], GOAL, &SLACK).is_empty());
        assert!(rulings(&[], GOAL, &SLACK).is_empty());
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
            rulings(&readings, GOAL, &SLACK),
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
            rulings(&[slightly, now], GOAL, &SLACK),
            [Ruling::NotWorthTurning, Ruling::Stayed]
        );
    }

    /// A candidate pulling `pull` newtons that the fewest supports reads as `supports`,
    /// tie-broken as a proof does, on the pull to its noise.
    fn pulling(pull: f64, supports: f64) -> Readings {
        let mut r = reading(0.0, Some(0.0), pull, (pull / 0.5).round());
        r.goals[FEWEST.index()] = supports;
        r
    }

    #[test]
    fn under_enough_pull_the_least_peel_takes_the_least_support() {
        let readings = [pulling(1.0, 9.0), pulling(6.0, 3.0), pulling(14.0, 1.0)];
        assert_eq!(
            rank(&readings, PEEL, &PROVEN),
            [1, 0, 2],
            "14 N is past 10 N and its band"
        );
    }

    #[test]
    fn past_enough_the_least_peel_takes_the_least_pull_out_of_its_band() {
        let readings = [pulling(25.0, 1.0), pulling(15.0, 9.0)];
        assert_eq!(rank(&readings, PEEL, &PROVEN)[0], 1);
    }

    #[test]
    fn a_model_pulling_little_enough_turns_only_to_spare_support() {
        let mut now = pulling(4.0, 10.0);
        now.standing = true;
        let lighter = pulling(1.0, 9.8);
        assert_eq!(
            rank(&[lighter, now], PEEL, &PROVEN)[0],
            1,
            "3 N less of a pull already under enough wins nothing"
        );
        let spare = pulling(1.0, 5.0);
        assert_eq!(
            rank(&[spare, now], PEEL, &PROVEN)[0],
            0,
            "half the support is worth it"
        );
    }

    #[test]
    fn a_turn_to_spare_support_has_to_spare_more_than_its_noise() {
        let mut now = pulling(2.9, 0.58);
        now.standing = true;
        let turned = pulling(2.9, 0.54);
        assert_eq!(
            rank(&[turned, now], PEEL, &PROVEN)[0],
            1,
            "0.04 ml is under the 0.05 ml a support reading is good to"
        );
    }

    #[test]
    fn past_enough_the_least_peel_takes_less_pull_over_less_support() {
        let readings = [pulling(28.4, 3.4), pulling(26.2, 5.1)];
        assert_eq!(
            rank(&readings, PEEL, &PROVEN)[0],
            1,
            "both in the band over 26.2 N, neither with enough: the pull decides"
        );
    }
}
