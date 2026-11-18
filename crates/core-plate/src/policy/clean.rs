//! Holding any preset to the poses with the fewest contacts where the model shows, and
//! offering the poses that give up a few of them for far more of what it wins (ADR 0410).

use super::{Band, Readings};

/// Within a tenth and one contact of the fewest on show is as clean: one contact on open
/// detail is a scar a brush stroke covers.
const CLEAN: Band = Band {
    share: 0.10,
    slack: 1.0,
};

/// A pose that shows more is offered only for a quarter more of what the preset wins.
const TRADE_SHARE: f64 = 0.25;

/// The band over the fewest contacts on show among the sound poses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Clean {
    limit: f64,
}

impl Clean {
    /// The band over the fewest of `sound` on show, its slack no less than `noise`.
    pub(super) fn of<'a>(sound: impl Iterator<Item = &'a Readings>, noise: f64) -> Self {
        let fewest = sound.map(|r| r.seen).min_by(f64::total_cmp);
        let band = Band {
            slack: CLEAN.slack.max(noise),
            ..CLEAN
        };
        Self {
            limit: fewest.map_or(f64::INFINITY, |fewest| band.limit(fewest)),
        }
    }

    pub(super) fn holds(self, r: &Readings) -> bool {
        r.seen <= self.limit
    }

    /// For each of `readings`, whether it is sound, shows past the band, wins far more than
    /// `chosen` by `trade`, and no sound pose shows less and wins as much.
    pub(super) fn traded(
        self,
        readings: &[Readings],
        chosen: &Readings,
        sound: impl Fn(&Readings) -> bool,
        trade: &Trade<'_>,
        seen_noise: f64,
    ) -> Vec<bool> {
        let dominates = |a: &Readings, b: &Readings| {
            let (own, other) = ((trade.main)(a), (trade.main)(b));
            a.seen <= b.seen
                && own <= other
                && (a.seen < b.seen - seen_noise || own < other - trade.noise)
        };
        readings
            .iter()
            .map(|b| {
                sound(b)
                    && !self.holds(b)
                    && trade.worth(chosen, b)
                    && !readings.iter().any(|a| sound(a) && dominates(a, b))
            })
            .collect()
    }
}

/// What a trade away from a clean surface is weighed on, lower being better, what of it is
/// noise, and how a gain on it is told worth the trade.
pub(super) struct Trade<'a> {
    main: Box<dyn Fn(&Readings) -> f64 + 'a>,
    noise: f64,
    /// Whether the gain is a share of the reading, or a cost that is already a logarithm.
    by_share: bool,
}

impl<'a> Trade<'a> {
    /// A reading whose gain counts as a share of the chosen's.
    pub(super) fn by_share(main: impl Fn(&Readings) -> f64 + 'a, noise: f64) -> Self {
        Self {
            main: Box::new(main),
            noise,
            by_share: true,
        }
    }

    /// A cost that is the logarithm of a product of shares.
    pub(super) fn by_cost(main: impl Fn(&Readings) -> f64 + 'a, noise: f64) -> Self {
        Self {
            main: Box::new(main),
            noise,
            by_share: false,
        }
    }

    /// Whether `traded` wins a quarter over `chosen`, and more than the noise.
    fn worth(&self, chosen: &Readings, traded: &Readings) -> bool {
        let (best, own) = ((self.main)(chosen), (self.main)(traded));
        let needed = if self.by_share {
            TRADE_SHARE * best.abs()
        } else {
            -(-TRADE_SHARE).ln_1p()
        };
        best - own > needed.max(self.noise)
    }
}
