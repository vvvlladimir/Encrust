use super::{BALL_STEPS, Part};
use printer_profiles::{ContactShape, PlatformShape, SupportProfile};

/// One ring of a support swept about its axis: how far along it stands and half its
/// width, millimetres, and the part the stretch from it to the next belongs to.
#[derive(Clone, Copy)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) half: f32,
    pub(super) part: Part,
}

pub(super) fn station(x: f32, half: f32, part: Part) -> Station {
    Station { x, half, part }
}

/// Half the width of `stations` at `x_mm`, straight between them.
pub(super) fn half_at(stations: &[Station], x_mm: f32) -> f32 {
    for pair in stations.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if x_mm >= a.x && x_mm <= b.x {
            let along = if b.x > a.x {
                (x_mm - a.x) / (b.x - a.x)
            } else {
                1.0
            };
            return a.half + (b.half - a.half) * along;
        }
    }
    stations.last().map_or(0.0, |station| station.half)
}

/// A support standing on the plate as the mesher builds it, and where its parts meet,
/// millimetres along.
pub(super) struct Column {
    pub(super) stations: Vec<Station>,
    pub(super) neck: f32,
    pub(super) flare: f32,
    pub(super) foot: f32,
    pub(super) plate: f32,
    /// Half the trunk's width at its base, the flare's where it lands and the pad's.
    pub(super) base: f32,
    pub(super) lower: f32,
    pub(super) pad: f32,
}

impl Column {
    /// One carrying `tips` whose trunk stands `trunk` millimetres tall.
    pub(super) fn of(profile: &SupportProfile, tips: usize, trunk: f32) -> Self {
        let mut stations = tip(profile);
        let neck = stations.last().map_or(0.0, |station| station.x);
        let base = profile.trunk_base_radius_mm(trunk, tips);
        let foot = neck + trunk;
        let plate = foot + profile.bottom.platform_thickness_mm;
        let (lower, feet) = foot_stations(profile, base, foot, plate);
        let flare = foot - (lower - base).max(0.0);
        stations.push(station(flare, base, Part::Base));
        stations.push(station(foot, lower, Part::Base));
        let pad = feet.iter().map(|station| station.half).fold(0.0, f32::max);
        stations.extend(feet);
        Self {
            stations,
            neck,
            flare,
            foot,
            plate,
            base,
            lower,
            pad,
        }
    }

    /// One standing on a raft instead: no pad, the flare widening onto the slab's top
    /// unchecked by a rim. See `docs/decisions/0266`.
    pub(super) fn on_raft(profile: &SupportProfile, tips: usize, trunk: f32) -> Self {
        let mut stations = tip(profile);
        let neck = stations.last().map_or(0.0, |station| station.x);
        let base = profile.trunk_base_radius_mm(trunk, tips);
        let foot = neck + trunk;
        let lower = profile.flare_lower_radius_mm().max(base);
        let flare = foot - (lower - base);
        stations.push(station(flare, base, Part::Base));
        stations.push(station(foot, lower, Part::Base));
        Self {
            stations,
            neck,
            flare,
            foot,
            plate: foot + profile.raft.thickness_mm,
            base,
            lower,
            pad: lower,
        }
    }

    pub(super) fn half_at(&self, x_mm: f32) -> f32 {
        half_at(&self.stations, x_mm)
    }

    /// The stations from `x_mm` on, the first of them there.
    pub(super) fn from(&self, x_mm: f32) -> Vec<Station> {
        let mut rest = vec![station(x_mm, self.half_at(x_mm), Part::Body)];
        rest.extend(self.stations.iter().filter(|station| station.x > x_mm));
        rest
    }

    /// The stations up to the neck.
    pub(super) fn tip(&self) -> Vec<Station> {
        let tip = self
            .stations
            .iter()
            .filter(|station| station.x <= self.neck);
        tip.copied().collect()
    }
}

/// How tall the trunk is of a support under a part stood as low as the profile lets it,
/// millimetres.
pub(super) fn lowest_trunk_mm(profile: &SupportProfile) -> f32 {
    let (top, foot) = (profile.top.length_mm, profile.footing_height_mm());
    (profile.lift_mm() - top - foot).max(2.0 * top).max(1.0)
}

/// How tall a trunk carrying `tips` stands before its base reaches the cap, millimetres.
pub(super) fn trunk_height_mm(profile: &SupportProfile, tips: usize) -> f32 {
    let cap = profile.trunk_base_radius_mm(f32::MAX / 4.0, tips);
    let (mut low, mut high) = (0.0_f32, 4096.0_f32);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if profile.trunk_base_radius_mm(middle, tips) >= cap - 1e-5 {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

/// The tip: a plane pushed its bite into the model, or a ball whose top is that far in,
/// then the cone out to the pillar. See `docs/decisions/0243`.
pub(super) fn tip(profile: &SupportProfile) -> Vec<Station> {
    let (contact, bite) = (profile.contact_radius_mm(), profile.tip.contact_depth_mm);
    let (length, pillar) = (profile.top.length_mm, profile.pillar_radius_mm());
    match profile.tip.shape {
        ContactShape::Plane => {
            let bite = bite.min(length);
            vec![
                station(-bite, contact, Part::Tip),
                station(length - bite, pillar, Part::Body),
            ]
        }
        ContactShape::Sphere => {
            let mut stations = vec![station(-bite, 0.0, Part::Tip)];
            for step in 1..BALL_STEPS {
                let angle = std::f32::consts::PI * step as f32 / BALL_STEPS as f32;
                let x = contact.mul_add(1.0 - angle.cos(), -bite);
                stations.push(station(x, contact * angle.sin(), Part::Tip));
            }
            let last = stations.last().map_or(0.0, |station| station.x);
            stations.push(station(length.max(last), pillar, Part::Body));
            stations
        }
    }
}

/// The flare's half-width where it lands, and the foot from the pad's top to the plate:
/// rings about the axis, or a skate seen along its length.
pub(super) fn foot_stations(
    profile: &SupportProfile,
    base: f32,
    foot: f32,
    plate: f32,
) -> (f32, Vec<Station>) {
    use core_supports::{FOOT_BEVEL, FOOT_TAPER, SKATE_BEVEL, SKATE_LENGTH, SKATE_WIDTH};
    let shape = profile.bottom.shape;
    let rim = profile.base_radius_mm();
    let bevel = profile.bottom.platform_thickness_mm * FOOT_BEVEL;
    let flare = profile.flare_lower_radius_mm();
    if shape == PlatformShape::Skate {
        let lower = flare.min(rim * SKATE_WIDTH * (1.0 - SKATE_BEVEL)).max(base);
        let long = rim * SKATE_LENGTH;
        let feet = vec![
            station(foot, long, Part::Base),
            station(plate, long * (1.0 - SKATE_BEVEL), Part::Base),
        ];
        return (lower, feet);
    }
    let on_plate = (rim - bevel).max(base);
    let top = if shape.tapers() {
        (rim * FOOT_TAPER).max(base)
    } else {
        rim
    };
    let mut feet = vec![station(foot, top, Part::Base)];
    if shape.tapers() {
        feet.push(station(plate - bevel, rim, Part::Base));
    }
    feet.push(station(plate, on_plate, Part::Base));
    (flare.min(on_plate.min(top)).max(base), feet)
}
