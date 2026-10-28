/// One layer's peel, both stages, as the bottom block or every other layer takes it; mm and
/// mm/min. A bottom layer's first retract runs as far as its first lift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPeel {
    pub lift_mm: f32,
    pub lift_speed_mm_min: f32,
    pub second_lift_mm: f32,
    pub second_lift_speed_mm_min: f32,
    pub retract_mm: f32,
    pub retract_speed_mm_min: f32,
    pub second_retract_mm: f32,
    pub second_retract_speed_mm_min: f32,
}
