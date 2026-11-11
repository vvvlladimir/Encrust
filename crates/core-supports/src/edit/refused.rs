/// Why a carried support may not stand where the hand put it (ADR 0441).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refused {
    /// A body would run into the model or keep less than its clearance from it.
    Collides,
    /// A strut would lean past 45 degrees, or a trunk on the plate off its foot.
    Leans,
    /// The foot has no plate or face of the model it may stand on there.
    NoFooting,
    /// The tip has no surface it may hold there.
    NoHold,
}
