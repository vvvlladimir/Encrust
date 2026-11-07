//! The one rule an orientation is chosen by, whatever the stage and the preset: each goal
//! ranks by its own reading alone, and the auto preset by the [`Balance`] of them all. See
//! `docs/design/orientation.md`.

use core_analysis::PEEL_LIMIT_N;

use crate::score::{OrientGoal, OrientPreset};

/// What one way to stand reads on everything the choice weighs; lower is better in each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Readings {
    /// What would lose the print outright whatever the goal, weighted by how surely: a
    /// pose past the build volume, or balanced on the plate on a point.
    pub fatal: f64,
    /// Suction cups, and a model set on the plate with something left hanging off it,
    /// weighted the same way: fatal to the auto preset, nothing to a single goal.
    pub suction: f64,
    /// Pulls past what the film lets go of, weighted the same way: fatal to the auto preset.
    pub tearing: f64,
    /// Islands left with nothing under them, where the stage measured them.
    pub unheld: Option<f64>,
    /// Contacts placement wanted and found no room for a column under, counted with the
    /// islands as left hanging.
    pub roomless: f64,
    /// Every goal's own reading, in the order of [`OrientGoal::ALL`].
    pub goals: [f64; OrientGoal::ALL.len()],
    /// What decides between candidates the goal reads alike, compared first to last.
    pub tiebreak: [f64; 2],
    /// Whether this is the way the model stands now, which a turn has to be worth. Where
    /// several are, set on the plate and on supports, the best ranked of them is.
    pub standing: bool,
}

/// What of each goal's reading is noise, in the order of [`OrientGoal::ALL`] and the unit a
/// stage reads it in: millilitres or a dimensionless share, newtons, millimetres.
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

const EXACT: Band = Band {
    share: 0.0,
    slack: 0.0,
};

/// How candidates are read for one goal: alone, or as the balance weighs it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Policy {
    goal: OrientGoal,
    /// Whether suction and tearing pulls fail the print and islands and contacts with no
    /// room hang: the balance's, never a single goal's (ADR 0410).
    weighs_hazards: bool,
    unheld: Band,
    primary: Band,
    /// The goal's reading at or under which the balance reads every candidate alike.
    enough: f64,
    /// How many of its noise the goal's reading is held to at least where the balance
    /// weighs it as a share of the best: less spares nothing worth another goal's share.
    floor_noises: f64,
}

impl Policy {
    /// The goal's own reading, no lower than enough.
    fn primary(&self, r: &Readings) -> f64 {
        r.goals[self.goal.index()].max(self.enough)
    }

    fn fatal(&self, r: &Readings) -> f64 {
        r.fatal
            + if self.weighs_hazards {
                r.suction + r.tearing
            } else {
                0.0
            }
    }

    /// The order key: fatal, then the least reading, then the tie-break.
    fn key(&self, r: &Readings) -> Key {
        (
            self.fatal(r),
            false,
            false,
            0.0,
            self.primary(r),
            0.0,
            r.tiebreak,
        )
    }

    /// What is left hanging, where the stage measured it and the goal weighs it.
    fn left(&self, r: &Readings) -> Option<f64> {
        r.unheld
            .filter(|_| self.weighs_hazards)
            .map(|islands| islands + r.roomless)
    }

    /// How far over the best the goal reads as good as the best beside the other goals: its
    /// own band, or the common one where it ranks strictly by the least.
    fn alike(&self, noise: f64) -> Band {
        if self.primary == EXACT {
            Band {
                share: PRIMARY_SHARE,
                slack: noise,
            }
        } else {
            self.primary
        }
    }

    /// A single goal's policy: strictly its least reading past what fails every goal, and
    /// any turn that lowers it; nothing another goal reads (ADR 0279, 0410).
    fn alone(goal: OrientGoal) -> Self {
        Self {
            goal,
            weighs_hazards: false,
            unheld: EXACT,
            primary: EXACT,
            enough: f64::NEG_INFINITY,
            floor_noises: 0.0,
        }
    }

    /// How the balance weighs `goal`: its band, enough and floor.
    fn balanced(goal: OrientGoal, noise: &Noise) -> Self {
        let primary_slack = noise[goal.index()];
        match goal {
            OrientGoal::FewestSupports => Self {
                floor_noises: SUPPORT_FLOOR_NOISES,
                ..Self::alone(goal)
            },
            OrientGoal::LeastPeel => Self {
                enough: ENOUGH_PULL_N,
                ..Self::banded(goal, primary_slack)
            },
            OrientGoal::ShortestPrint => Self {
                primary: Band {
                    share: STACK_SHARE,
                    slack: primary_slack.max(STACK_SLACK_MM),
                },
                floor_noises: STACK_FLOOR_NOISES,
                ..Self::banded(goal, primary_slack)
            },
            // Not in `OrientGoal::BALANCED`: what shows is the user's to ask for (ADR 0409).
            OrientGoal::CleanSurface => Self::banded(goal, primary_slack),
        }
    }

    /// Nothing failing or hanging, and the reading within a band.
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
            floor_noises: 0.0,
        }
    }
}

/// Islands and hanging contacts within a quarter and two of the fewest are the grain of
/// the support grid, as islands are counted at the print's own pixel and layer (ADR 0285).
const UNHELD: Band = Band {
    share: 0.25,
    slack: 2.0,
};
/// The goal's own reading within a tenth of the best is as good as the best.
const PRIMARY_SHARE: f64 = 0.10;
/// A turn has to win a twentieth of the balance's cost over standing still.
const STAY_GAIN: f64 = 0.05;
/// A pull under a third of the limit is no longer worth another goal's share: the scale of
/// the pull is known to about a factor of three (`docs/design/analysis.md`).
const ENOUGH_PULL_N: f64 = PEEL_LIMIT_N / 3.0;
/// A stack within a fiftieth and a millimetre of the shortest prints as fast: some four
/// minutes on a 100 mm figure, two on a flat part.
const STACK_SHARE: f64 = 0.02;
const STACK_SLACK_MM: f64 = 1.0;
/// Ten struts of support, half a millilitre as a proof reads it, is resin nobody misses
/// and a minute to clip off.
const SUPPORT_FLOOR_NOISES: f64 = 10.0;
/// Five noises of stack, 1.5 mm as a proof reads it, is about what the bottom layers add
/// to any print whatever its height.
const STACK_FLOOR_NOISES: f64 = 5.0;

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

    /// Whether `r` fails the print more surely than the best, or leaves more hanging,
    /// compared in that order with `other`.
    fn fails_less(&self, r: &Readings, other: &Readings, policy: &Policy) -> bool {
        (policy.fatal(r), !self.held(r, policy))
            .partial_cmp(&(policy.fatal(other), !self.held(other, policy)))
            .is_some_and(std::cmp::Ordering::is_lt)
    }
}

/// How the auto preset weighs every goal at once: the product of each reading's share of
/// the best a sound pose reads, `docs/design/orientation.md`.
struct Balance {
    /// What fails the print and what hangs, weighed as strictly as any goal weighs them.
    hazards: Policy,
    bests: Bests,
    goals: [Policy; OrientGoal::BALANCED.len()],
    noise: Noise,
    /// Each goal's best reading among the sound, and the floor under every reading of it.
    best: [f64; OrientGoal::BALANCED.len()],
    floor: [f64; OrientGoal::BALANCED.len()],
}

/// A share no reading falls under, so that a goal at zero at best still divides.
const TINY: f64 = 1e-9;

impl Balance {
    fn of(readings: &[Readings], noise: &Noise) -> Option<Self> {
        let hazards = Policy::banded(OrientGoal::ALL[0], 0.0);
        let bests = Bests::of(readings, &hazards)?;
        let goals = OrientGoal::BALANCED.map(|goal| Policy::balanced(goal, noise));
        let best = goals.map(|policy| {
            readings
                .iter()
                .filter(|r| bests.first_tier(r, &hazards))
                .map(|r| policy.primary(r))
                .min_by(f64::total_cmp)
                .unwrap_or(f64::INFINITY)
        });
        let floor = std::array::from_fn(|at| goals[at].floor_noises * noise[at]);
        Some(Self {
            hazards,
            bests,
            goals,
            noise: *noise,
            best,
            floor,
        })
    }

    /// The natural logarithm of each goal's reading as a share of its best.
    fn shares(&self, r: &Readings) -> [f64; OrientGoal::BALANCED.len()] {
        std::array::from_fn(|at| {
            let over = |reading: f64| (reading + self.floor[at]).max(TINY);
            (over(self.goals[at].primary(r)) / over(self.best[at])).ln()
        })
    }

    /// What every share costs together: the logarithm of their product.
    fn cost(&self, r: &Readings) -> f64 {
        self.shares(r).iter().sum()
    }

    /// What the noise of every goal's reading moves the cost by at most.
    fn cost_noise(&self) -> f64 {
        (0..self.goals.len())
            .map(|at| (1.0 + self.noise[at] / (self.best[at] + self.floor[at]).max(TINY)).ln())
            .sum()
    }

    fn sound(&self, r: &Readings) -> bool {
        self.bests.first_tier(r, &self.hazards)
    }

    /// Whether `r` is sound and as good as the best on every goal at once.
    fn in_every_band(&self, r: &Readings) -> bool {
        self.sound(r)
            && self.goals.iter().enumerate().all(|(at, policy)| {
                policy.primary(r) <= policy.alike(self.noise[at]).limit(self.best[at])
            })
    }

    /// Whether `a` reads no worse than `b` on any goal and better past the noise on one.
    fn dominates(&self, a: &Readings, b: &Readings) -> bool {
        let pairs = self
            .goals
            .iter()
            .map(|policy| (policy.primary(a), policy.primary(b)));
        let mut better = false;
        for (at, (own, other)) in pairs.enumerate() {
            if own > other {
                return false;
            }
            better |= own < other - self.noise[at];
        }
        better
    }

    /// For each of `readings`, whether a sound one dominates it.
    fn dominated(&self, readings: &[Readings]) -> Vec<bool> {
        readings
            .iter()
            .map(|b| {
                readings
                    .iter()
                    .any(|a| self.sound(a) && self.dominates(a, b))
            })
            .collect()
    }

    /// The order key: fatal, then held, then good on every goal, then dominated, then the
    /// cost, then the worst share, then the tie-break.
    fn key(&self, r: &Readings, dominated: bool) -> Key {
        let worst = self.shares(r).into_iter().fold(f64::NEG_INFINITY, f64::max);
        (
            self.hazards.fatal(r),
            !self.bests.held(r, &self.hazards),
            !self.in_every_band(r),
            f64::from(u8::from(dominated)),
            self.cost(r),
            worst,
            r.tiebreak,
        )
    }

    /// Whether `turned` is worth leaving `now` for: it fails less, or standing is not good
    /// on every goal and the turn is, or it costs a twentieth less and more than the noise.
    fn worth_turning(&self, turned: &Readings, now: &Readings) -> bool {
        if self.bests.fails_less(turned, now, &self.hazards) {
            return true;
        }
        if self.in_every_band(now) {
            return false;
        }
        let gain = self.cost(now) - self.cost(turned);
        self.in_every_band(turned) || gain > (-(-STAY_GAIN).ln_1p()).max(self.cost_noise())
    }
}

type Key = (f64, bool, bool, f64, f64, f64, [f64; 2]);

fn compare(a: &Key, b: &Key) -> std::cmp::Ordering {
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
pub fn rank(readings: &[Readings], preset: OrientPreset, noise: &Noise) -> Vec<usize> {
    Ranking::of(readings, preset, noise).map_or_else(Vec::new, |ranking| ranking.order)
}

/// Why each of `readings` ended where [`rank`] put it, in the order given.
pub fn rulings(readings: &[Readings], preset: OrientPreset, noise: &Noise) -> Vec<Ruling> {
    let Some(ranking) = Ranking::of(readings, preset, noise) else {
        return Vec::new();
    };
    let mut rulings = vec![Ruling::TieBreak; readings.len()];
    for (place, at) in ranking.order.iter().enumerate() {
        rulings[*at] = ranking.ruling(readings, *at, place);
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
    /// Balancing every goal, another sound pose reads better on one and no worse on any.
    Dominated,
    /// Balancing every goal, a sound pose no other beats on all of them, which costs more
    /// than the chosen: a trade the user may prefer.
    Alternative,
}

impl Ruling {
    /// Whether a pose so ruled is one worth offering in turn beside the chosen: as good, or
    /// a trade no other pose beats.
    pub fn is_offered(self) -> bool {
        matches!(
            self,
            Self::Chosen
                | Self::Stayed
                | Self::NotWorthTurning
                | Self::TieBreak
                | Self::Alternative
        )
    }
}

/// What ranks the candidates: one goal's policy, or the balance of every goal.
enum Rule {
    Goal {
        policy: Policy,
        bests: Bests,
    },
    Balance {
        balance: Box<Balance>,
        dominated: Vec<bool>,
    },
}

impl Rule {
    fn of(readings: &[Readings], preset: OrientPreset, noise: &Noise) -> Option<Self> {
        Some(match preset {
            OrientPreset::Goal(goal) => {
                let policy = Policy::alone(goal);
                let bests = Bests::of(readings, &policy)?;
                Self::Goal { policy, bests }
            }
            OrientPreset::Auto => {
                let balance = Balance::of(readings, noise)?;
                let dominated = balance.dominated(readings);
                Self::Balance {
                    balance: Box::new(balance),
                    dominated,
                }
            }
        })
    }

    fn key(&self, readings: &[Readings], at: usize) -> Key {
        match self {
            Self::Goal { policy, .. } => policy.key(&readings[at]),
            Self::Balance { balance, dominated } => balance.key(&readings[at], dominated[at]),
        }
    }

    fn worth_turning(&self, turned: &Readings, now: &Readings) -> bool {
        match self {
            Self::Goal { policy, bests } => {
                bests.fails_less(turned, now, policy) || !bests.in_band(now, policy)
            }
            Self::Balance { balance, .. } => balance.worth_turning(turned, now),
        }
    }

    /// What a candidate ranked past the first lost on.
    fn lost_on(&self, readings: &[Readings], at: usize) -> Ruling {
        let r = &readings[at];
        match self {
            Self::Goal { policy, bests } => {
                if policy.fatal(r).total_cmp(&bests.fatal).is_gt() {
                    Ruling::Fatal
                } else if !bests.held(r, policy) {
                    Ruling::Unheld
                } else if !bests.in_band(r, policy) {
                    Ruling::OutOfBand
                } else {
                    Ruling::TieBreak
                }
            }
            Self::Balance { balance, dominated } => {
                if balance
                    .hazards
                    .fatal(r)
                    .total_cmp(&balance.bests.fatal)
                    .is_gt()
                {
                    Ruling::Fatal
                } else if !balance.bests.held(r, &balance.hazards) {
                    Ruling::Unheld
                } else if dominated[at] {
                    Ruling::Dominated
                } else {
                    Ruling::Alternative
                }
            }
        }
    }
}

/// The rule, the order, and whether standing still was moved ahead.
struct Ranking {
    rule: Rule,
    order: Vec<usize>,
    stayed: bool,
}

impl Ranking {
    fn of(readings: &[Readings], preset: OrientPreset, noise: &Noise) -> Option<Self> {
        let rule = Rule::of(readings, preset, noise)?;
        let keys: Vec<Key> = (0..readings.len())
            .map(|at| rule.key(readings, at))
            .collect();
        let mut order: Vec<usize> = (0..readings.len()).collect();
        order.sort_by(|a, b| compare(&keys[*a], &keys[*b]));

        let standing = order.iter().position(|at| readings[*at].standing);
        let stayed = standing
            .filter(|position| *position > 0)
            .is_some_and(|position| {
                let (turned, now) = (order[0], order[position]);
                compare(&keys[turned], &keys[now]).is_ge()
                    || !rule.worth_turning(&readings[turned], &readings[now])
            });
        if let (true, Some(position)) = (stayed, standing) {
            let now = order.remove(position);
            order.insert(0, now);
        }
        Some(Self {
            rule,
            order,
            stayed,
        })
    }

    fn ruling(&self, readings: &[Readings], at: usize, place: usize) -> Ruling {
        match (place, self.stayed) {
            (0, true) => Ruling::Stayed,
            (0, false) => Ruling::Chosen,
            (1, true) => Ruling::NotWorthTurning,
            _ => self.rule.lost_on(readings, at),
        }
    }
}

#[cfg(test)]
mod tests;
