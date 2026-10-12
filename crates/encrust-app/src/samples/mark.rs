//! The Encrust mark built as the solid it draws: three slabs stacked with a gap between
//! each, the top one cut into the letter. Seen from the front right corner it is the logo.

use core_geometry::Vec2;

use crate::samples::solid::{EMBED, Solid};

/// The slabs' depth over their width, and their corners' radius over their width.
const DEPTH: f32 = 0.867;
const CORNER: f32 = 0.067;
/// Of each tier, the share the slab takes; the rest is a neck set in by as much.
const SLAB_SHARE: f32 = 0.7;
const CORNER_STEPS: usize = 6;

/// Stands the mark on `floor` at `centre`, `width_mm` along the letter's arms and
/// `height_mm` tall. The arms run along +Y and the letter's spine faces -X.
pub fn stand(solid: &mut Solid, centre: Vec2, width_mm: f32, height_mm: f32, floor: f32) {
    let tier = height_mm / 3.0;
    let neck = tier * (1.0 - SLAB_SHARE);
    let place = |outline: Vec<Vec2>| -> Vec<Vec2> {
        outline
            .into_iter()
            .map(|point| centre + Vec2::new(-point.y, point.x))
            .collect()
    };
    let slab = place(rounded(width_mm, 0.0));
    let necked = place(rounded(width_mm, neck));
    let letter = place(
        LETTER
            .iter()
            .map(|&[u, v]| Vec2::new(u, v) * width_mm)
            .collect(),
    );
    for level in 0..3 {
        let bottom = floor + EMBED + tier * level as f32;
        let top = bottom + tier * SLAB_SHARE;
        if level > 0 {
            solid.prism(&necked, bottom - neck - EMBED, bottom + EMBED);
        }
        let outline = if level == 2 { &letter } else { &slab };
        let sunk = if level == 0 { EMBED } else { 0.0 };
        solid.prism(outline, bottom - sunk, top);
    }
}

/// A slab's outline `width` across, set in by `inset` all round, counter-clockwise.
fn rounded(width: f32, inset: f32) -> Vec<Vec2> {
    let half = Vec2::new(width, width * DEPTH) / 2.0 - Vec2::splat(inset);
    let radius = (width * CORNER - inset).max(width * CORNER / 2.0);
    let centres = [
        Vec2::new(half.x - radius, -half.y + radius),
        Vec2::new(half.x - radius, half.y - radius),
        Vec2::new(-half.x + radius, half.y - radius),
        Vec2::new(-half.x + radius, -half.y + radius),
    ];
    let mut outline = Vec::with_capacity(4 * (CORNER_STEPS + 1));
    for (corner, centre) in centres.iter().enumerate() {
        for step in 0..=CORNER_STEPS {
            let quarter = std::f32::consts::FRAC_PI_2;
            let angle = quarter * (corner as f32 - 1.0 + step as f32 / CORNER_STEPS as f32);
            outline.push(*centre + Vec2::from_angle(angle) * radius);
        }
    }
    outline
}

/// The letter's outline a unit along its arms, centred, counter-clockwise.
const LETTER: [[f32; 2]; 50] = [
    [-0.4399, -0.4330],
    [0.2334, -0.4333],
    [0.2589, -0.4283],
    [0.2757, -0.4182],
    [0.2922, -0.3980],
    [0.2987, -0.3798],
    [0.2996, -0.2935],
    [0.2888, -0.2629],
    [0.2647, -0.2413],
    [0.2334, -0.2334],
    [-0.2215, -0.2326],
    [-0.2333, -0.2183],
    [-0.2333, -0.1166],
    [-0.2284, -0.1050],
    [-0.2166, -0.1000],
    [0.2333, -0.1001],
    [0.2589, -0.0949],
    [0.2756, -0.0849],
    [0.2848, -0.0757],
    [0.2950, -0.0589],
    [0.2987, -0.0464],
    [0.2997, 0.0399],
    [0.2950, 0.0588],
    [0.2849, 0.0756],
    [0.2647, 0.0921],
    [0.2399, 0.0997],
    [-0.2214, 0.1007],
    [-0.2332, 0.1150],
    [-0.2327, 0.2215],
    [-0.2272, 0.2295],
    [-0.2167, 0.2333],
    [0.4464, 0.2345],
    [0.4588, 0.2384],
    [0.4757, 0.2484],
    [0.4887, 0.2629],
    [0.4971, 0.2807],
    [0.5000, 0.3666],
    [0.4949, 0.3921],
    [0.4704, 0.4221],
    [0.4527, 0.4305],
    [0.4334, 0.4333],
    [-0.4333, 0.4333],
    [-0.4588, 0.4283],
    [-0.4756, 0.4182],
    [-0.4921, 0.3980],
    [-0.5000, 0.3666],
    [-0.4997, -0.3732],
    [-0.4949, -0.3923],
    [-0.4805, -0.4139],
    [-0.4647, -0.4255],
];
