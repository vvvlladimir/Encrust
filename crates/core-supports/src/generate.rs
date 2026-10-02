use std::collections::HashMap;

use core_geometry::{Scalar, Vec2, Vec3};
use core_slicer::{Layer, Sliced};
use printer_profiles::SupportProfile;
use rayon::prelude::*;

use crate::column::MIN_PILLAR_HEIGHT_MM;
use crate::config::{GRID_PITCH_MM, PLATE_CONTACT_MM, REFERENCE_RISE_MM, SampleConfig};
use crate::detect::unsupported;
use crate::field::{Field, Grid, Parts};
use crate::region::Blocked;
use crate::sample::sample_piece;

/// How many layers are read and looked at in one go. Enough to keep every core busy, few
/// enough that a cancelled run stops inside a frame; see `docs/decisions/0033`.
const READ_BLOCK: usize = 32;

/// Where automatic placement wants supports, in plate coordinates.
///
/// `stack` is the model alone, already cut in plate coordinates; `seeds` are the contacts
/// of supports that already exist, which hold up what is near them and are never moved or
/// returned. `progress` is called with each layer index before what was found there is
/// put down, and stops the run by returning `false`, which leaves whatever was found up
/// to that layer.
///
/// Nothing is put where `blocked` reaches: a painted patch keeps automatic placement off
/// it the way it keeps a hand-placed support off it; see `docs/decisions/0093`.
///
/// The returned contacts sit a layer under the surface that needs them. Whether one can
/// actually stand there is a question for [`crate::columns`].
pub fn generate_supports(
    stack: &Sliced,
    layer_height_mm: Scalar,
    profile: &SupportProfile,
    seeds: &[Vec3],
    blocked: Option<&Blocked>,
    progress: &mut dyn FnMut(usize) -> bool,
) -> Vec<Vec3> {
    let Some(grid) = grid_of(stack) else {
        return Vec::new();
    };
    let config = SampleConfig::for_profile(profile);
    let pass = Pass {
        grid: &grid,
        config: &config,
        spacing: grid.cells_of(config.spacing_mm).max(1),
        // How many layers back the lean is measured over.
        window: ((REFERENCE_RISE_MM / layer_height_mm).round() as usize).max(1),
        layer_height_mm,
        keep_out_mm: profile.contact_radius_mm() + profile.clearance_mm,
        // A contact lower than a foot and the shortest pillar that can stand on it has
        // nowhere to put a column, and material that close to the plate is not going to
        // sag before it cures anyway.
        lowest_contact_mm: PLATE_CONTACT_MM.max(profile.base_height_mm() + MIN_PILLAR_HEIGHT_MM),
    };

    let mut standing = Standing::new(seeds, &grid, &config, layer_height_mm);
    let mut placed = Vec::new();
    // The block being looked at, with as much of the block before it as the lean is
    // measured over still in front of it.
    let mut fields: Vec<Field> = Vec::new();
    let mut first_held = 0usize;

    for (block, layers) in stack.layers.chunks(READ_BLOCK).enumerate() {
        let fresh = fields.len();
        fields.par_extend(layers.par_iter().map(|layer| Field::of_layer(layer, &grid)));
        let looked: Vec<(Parts, Vec<[i32; 2]>)> = (fresh..fields.len())
            .into_par_iter()
            .map(|at| {
                (
                    fields[at].parts(),
                    pass.wanted(&fields, at, first_held + at, &stack.layers[first_held + at]),
                )
            })
            .collect();

        for (offset, (parts, cells)) in looked.into_iter().enumerate() {
            let index = block * READ_BLOCK + offset;
            if !progress(index) {
                return placed;
            }
            let contact_mm = contact_of(&stack.layers[index], layer_height_mm);
            standing.climb(&parts, contact_mm);
            for cell in cells {
                let at = grid.centre_mm(cell[0], cell[1]);
                // Asked before the cell is taken, or a place nothing may stand on would
                // still count as held and leave its neighbours a support short.
                if blocked
                    .is_some_and(|blocked| blocked.covers(at.extend(contact_mm), pass.keep_out_mm))
                {
                    continue;
                }
                if let Some(contact) = standing.take(parts.at(cell[0], cell[1]), at, contact_mm) {
                    placed.push(contact);
                }
            }
            standing.rest_on(parts);
        }

        let spent = fields.len() - pass.window.min(fields.len());
        fields.drain(..spent);
        first_held += spent;
    }

    placed
}

/// Where a column put under `layer` touches, in millimetres above the plate.
///
/// A whole layer under the plane, not half of one: a cell this layer is the first to
/// cover has the model's own surface somewhere inside that rise, and a contact above it
/// would stand a column on the very surface it is meant to hold up. The tip sinks further
/// than this into the model anyway; see `docs/design/supports.md`.
fn contact_of(layer: &Layer, layer_height_mm: Scalar) -> Scalar {
    layer.z - layer_height_mm
}

/// What one run measures with, so that one layer can be looked at on its own.
///
/// Every layer of a block is read and examined at once, because what a layer needs holding
/// up depends only on the layers under it and not on where a support was put; see
/// `docs/decisions/0033`.
struct Pass<'a> {
    grid: &'a Grid,
    config: &'a SampleConfig,
    /// How far apart supports go, in cells.
    spacing: i32,
    /// How many layers back the lean is measured over.
    window: usize,
    layer_height_mm: Scalar,
    /// How close to a blocked patch a contact may still be put.
    keep_out_mm: Scalar,
    lowest_contact_mm: Scalar,
}

impl Pass<'_> {
    /// The cells of `fields[at]` that want a support, where that layer is `index` of the
    /// stack and was cut at `cut`. `fields` holds it and the layers the lean is measured
    /// back over.
    fn wanted(&self, fields: &[Field], at: usize, index: usize, cut: &Layer) -> Vec<[i32; 2]> {
        let layer = &fields[at];
        if layer.is_empty() || contact_of(cut, self.layer_height_mm) <= self.lowest_contact_mm {
            return Vec::new();
        }

        // The first layers of a stack have nothing under them to be measured against.
        let seen = self.window.min(index).min(at);
        let nothing = Field::default();
        let below = if seen == 0 { &nothing } else { &fields[at - 1] };
        let reference = if seen == 0 {
            &nothing
        } else {
            &fields[at - seen]
        };
        let rise_mm = seen as Scalar * self.layer_height_mm;

        let found = unsupported(layer, below, reference, self.grid, self.config, rise_mm);
        // Islands first: a support put down under one also holds the overhang it is part
        // of, which the coverage check then leaves alone.
        found
            .islands
            .iter()
            .chain(&found.overhangs)
            .flat_map(|piece| sample_piece(piece, self.spacing))
            .collect()
    }
}

/// The supports standing under each piece of the layer the run has reached.
///
/// A piece inherits what stood under the pieces it grew out of, so a support suppresses a
/// sample only where the two hold the same part of the model: a column passing unrelated
/// material no longer leaves it a support short. See `docs/decisions/0081`.
///
/// Each piece's points are kept in buckets of the plate one coverage radius across, so
/// asking whether a place is already held reads nine buckets rather than every support
/// placed so far.
struct Standing {
    /// One set per piece of the layer last climbed to, numbered as `Parts` numbers them.
    held: Vec<Held>,
    /// The pieces of the layer below, so a piece can be followed onto what it grew from.
    below: Option<Parts>,
    /// Supports handed to the run that the climb has not reached yet, highest last.
    pending: Vec<(Vec3, [i32; 2])>,
    /// Side of one bucket, millimetres: the radius over which a support holds.
    radius_mm: Scalar,
    /// How far above a place a support may stand and still hold it. A contact is put a
    /// layer under the surface it holds and a seed sits on it, so the two disagree by
    /// exactly that much.
    above_mm: Scalar,
}

impl Standing {
    fn new(seeds: &[Vec3], grid: &Grid, config: &SampleConfig, layer_height_mm: Scalar) -> Self {
        let mut pending: Vec<(Vec3, [i32; 2])> = seeds
            .iter()
            .map(|seed| (*seed, grid.cell_of(seed.truncate())))
            .collect();
        pending.sort_by(|a, b| b.0.z.total_cmp(&a.0.z));

        Self {
            held: Vec::new(),
            below: None,
            pending,
            radius_mm: config.coverage_radius_mm.max(Scalar::MIN_POSITIVE),
            above_mm: layer_height_mm,
        }
    }

    /// Carries what is standing onto the pieces of the next layer up, whose contacts sit
    /// at `z`, and takes on the seeds that layer has reached. The layer is handed back
    /// through [`Standing::rest_on`] once its own samples have been asked about.
    ///
    /// A point further below than the coverage radius is dropped on the way: it can hold
    /// nothing on this layer or on any layer above it, which is what keeps the run's
    /// memory to the supports that still matter rather than to every one ever placed.
    fn climb(&mut self, parts: &Parts, z: Scalar) {
        let floor_z = z - self.radius_mm;
        let parents = self.below.as_ref().map(|below| parts.parents(below));
        let Some(parents) = parents else {
            self.held = vec![Held::default(); parts.count()];
            self.take_seeds(parts, z);
            return;
        };

        // A piece that is the only child of its only parent takes that parent's set over
        // rather than copying it, which is the whole of a wall going straight up.
        let mut children = vec![0u32; self.held.len()];
        for grown_from in &parents {
            for parent in grown_from {
                children[*parent as usize] += 1;
            }
        }

        let mut held = Vec::with_capacity(parts.count());
        for grown_from in &parents {
            let mut piece = match grown_from.as_slice() {
                [only] if children[*only as usize] == 1 => {
                    let mut taken = std::mem::take(&mut self.held[*only as usize]);
                    taken.prune(floor_z, self.radius_mm);
                    taken
                }
                parents => {
                    let mut piece = Held::default();
                    for parent in parents {
                        piece.take_in(&self.held[*parent as usize], floor_z, self.radius_mm);
                    }
                    piece
                }
            };
            piece.buckets.shrink_to_fit();
            held.push(piece);
        }
        self.held = held;
        self.take_seeds(parts, z);
    }

    /// Takes on every support handed to the run that the climb has reached.
    fn take_seeds(&mut self, parts: &Parts, z: Scalar) {
        while let Some((seed, cell)) = self.pending.last().copied() {
            if seed.z > z + self.above_mm {
                break;
            }
            self.pending.pop();
            if let Some(piece) = parts.at(cell[0], cell[1]) {
                self.held[piece as usize].add(seed, self.radius_mm);
            }
        }
    }

    /// Keeps the layer just sampled as what the next one grows out of.
    fn rest_on(&mut self, parts: Parts) {
        self.below = Some(parts);
    }

    /// Puts a support at `at` on the underside of `piece` at height `z`, unless one
    /// standing under the same piece already holds that place. Returns the contact it
    /// placed.
    fn take(&mut self, piece: Option<u32>, at: Vec2, z: Scalar) -> Option<Vec3> {
        let contact = at.extend(z);
        let Some(held) = piece.and_then(|piece| self.held.get_mut(piece as usize)) else {
            return Some(contact);
        };
        if held.holds(at, z, self.radius_mm, self.above_mm) {
            return None;
        }
        held.add(contact, self.radius_mm);
        Some(contact)
    }
}

/// The supports standing under one piece of a layer.
#[derive(Debug, Clone, Default)]
struct Held {
    buckets: HashMap<[i32; 2], Vec<Vec3>>,
    /// The lowest point held, so that a set with nothing to drop is not walked at all.
    lowest_z: Scalar,
}

impl Held {
    /// Whether anything here already holds `at` on the layer whose contacts sit at `z`.
    fn holds(&self, at: Vec2, z: Scalar, radius_mm: Scalar, above_mm: Scalar) -> bool {
        let home = bucket_of(at, radius_mm);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                self.buckets
                    .get(&[home[0] + dx, home[1] + dy])
                    .is_some_and(|points| {
                        points.iter().any(|point| {
                            let reach = reach_at(z - point.z, radius_mm, above_mm);
                            (point.truncate() - at).length_squared() < reach * reach
                        })
                    })
            })
        })
    }

    fn add(&mut self, contact: Vec3, radius_mm: Scalar) {
        if self.buckets.is_empty() {
            self.lowest_z = contact.z;
        }
        self.lowest_z = self.lowest_z.min(contact.z);
        self.buckets
            .entry(bucket_of(contact.truncate(), radius_mm))
            .or_default()
            .push(contact);
    }

    /// Drops everything too far below `floor_z` to hold anything there or above it.
    fn prune(&mut self, floor_z: Scalar, radius_mm: Scalar) {
        if self.buckets.is_empty() || self.lowest_z >= floor_z {
            return;
        }
        let kept = std::mem::take(self);
        self.take_in(&kept, floor_z, radius_mm);
    }

    /// Takes on everything of `other` that can still hold something at `floor_z` or above.
    fn take_in(&mut self, other: &Self, floor_z: Scalar, radius_mm: Scalar) {
        for point in other.buckets.values().flatten() {
            if point.z >= floor_z {
                self.add(*point, radius_mm);
            }
        }
    }
}

/// How far a standing support still holds, `rise` millimetres above itself: a ball of the
/// coverage radius, so what is climbing away from it is on its own. See
/// `docs/decisions/0079`.
fn reach_at(rise: Scalar, radius_mm: Scalar, above_mm: Scalar) -> Scalar {
    // A seed sits on the surface and a placed contact a layer under it, so a support that
    // much above the place still counts as under it.
    let rise = (rise + above_mm).max(0.0);
    (radius_mm * radius_mm - rise * rise).max(0.0).sqrt()
}

fn bucket_of(at: Vec2, radius_mm: Scalar) -> [i32; 2] {
    [
        (at.x / radius_mm).floor() as i32,
        (at.y / radius_mm).floor() as i32,
    ]
}

/// The grid the whole stack is read on, or `None` when there is nothing on it.
fn grid_of(stack: &Sliced) -> Option<Grid> {
    let mut min = Vec2::splat(Scalar::INFINITY);
    let mut max = Vec2::splat(Scalar::NEG_INFINITY);
    for point in stack
        .layers
        .iter()
        .flat_map(|layer| &layer.contours)
        .flat_map(|contour| &contour.points)
    {
        min = min.min(*point);
        max = max.max(*point);
    }
    (min.x <= max.x).then(|| Grid::covering(min, max, GRID_PITCH_MM))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::box_mesh;
    use core_geometry::Mesh;
    use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};

    /// Layer height every test in this module cuts at, millimetres.
    const LAYER_HEIGHT_MM: Scalar = 0.05;

    fn profile() -> SupportProfile {
        crate::tests::profile()
    }

    fn slice(mesh: &Mesh) -> Sliced {
        PlaneSliceEngine
            .slice(
                mesh,
                &SliceSettings {
                    layer_height: LAYER_HEIGHT_MM,
                    ..SliceSettings::default()
                },
            )
            .expect("a closed mesh slices")
    }

    /// Everything automatic placement puts under `mesh`, with nothing there already.
    fn supports_for(mesh: &Mesh) -> Vec<Vec3> {
        generate_supports(
            &slice(mesh),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            None,
            &mut |_| true,
        )
    }

    fn merged(a: &Mesh, b: &Mesh) -> Mesh {
        let offset = a.vertices.len() as u32;
        let mut mesh = a.clone();
        mesh.vertices.extend_from_slice(&b.vertices);
        mesh.faces.extend(
            b.faces
                .iter()
                .map(|[x, y, z]| [x + offset, y + offset, z + offset]),
        );
        mesh
    }

    /// A ball of `radius` with its south pole at `z`, fine enough that its lower cap is a
    /// smooth overhang rather than a staircase.
    fn sphere(radius: Scalar, base_mm: Scalar) -> Mesh {
        let (segments, rings) = (96u32, 48u32);
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for ring in 0..=rings {
            let phi = std::f32::consts::PI * ring as Scalar / rings as Scalar;
            for segment in 0..segments {
                let theta = std::f32::consts::TAU * segment as Scalar / segments as Scalar;
                vertices.push(Vec3::new(
                    radius * phi.sin() * theta.cos(),
                    radius * phi.sin() * theta.sin(),
                    base_mm + radius - radius * phi.cos(),
                ));
            }
        }
        for ring in 0..rings {
            for segment in 0..segments {
                let next = (segment + 1) % segments;
                let low_left = ring * segments + segment;
                let low_right = ring * segments + next;
                let high_left = (ring + 1) * segments + segment;
                let high_right = (ring + 1) * segments + next;
                faces.push([low_left, high_right, high_left]);
                faces.push([low_left, low_right, high_right]);
            }
        }
        Mesh::new(vertices, faces)
    }

    /// Two boxes, the upper one jutting `step` further out over the lower one's ledge:
    /// a staircase of overhangs, one above the other, which is what a stepped model is.
    fn steps(step: Scalar, rise: Scalar) -> Mesh {
        let lower = box_mesh(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(10.0, 10.0, 10.0 + rise),
        );
        let upper = box_mesh(
            Vec3::new(0.0, -step, 10.0 + rise),
            Vec3::new(10.0, 10.0, 10.0 + rise + rise),
        );
        merged(&lower, &upper)
    }

    #[test]
    fn a_step_above_another_one_is_held_as_well() {
        // One millimetre of jut in a single layer, four millimetres above the ledge under
        // it: too wide for the resin to bridge, and too far up for the supports below to
        // reach. See `docs/decisions/0079` and `docs/decisions/0080`.
        let points = supports_for(&steps(1.0, 4.0));
        let upper = points.iter().filter(|point| point.y < 0.0).count();

        // Ten millimetres of shelf at the medium preset's five millimetre spacing.
        assert!(
            upper >= 2,
            "the upper step is 10 mm of shelf and got {upper} supports"
        );
    }

    /// The region every face of `mesh` looking down belongs to: the underside a support
    /// would otherwise be put against.
    fn underside(mesh: &Mesh) -> crate::Region {
        let mut region = crate::Region::default();
        for face in 0..mesh.faces.len() {
            let looking_down = mesh.triangle(face).is_some_and(|triangle| {
                triangle.normal_unnormalized().normalize_or_zero().z < -0.9
            });
            region.set(face, looking_down);
        }
        region
    }

    #[test]
    fn an_island_painted_out_of_bounds_is_left_alone() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let blocked = Blocked::new(
            &floating,
            &underside(&floating),
            core_geometry::Transform::default(),
        )
        .expect("the underside was painted");

        let points = generate_supports(
            &slice(&floating),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            Some(&blocked),
            &mut |_| true,
        );

        assert!(
            points.is_empty(),
            "the only surface that needed holding up was blocked, got {} contacts",
            points.len()
        );
    }

    #[test]
    fn a_box_standing_on_the_plate_needs_no_supports() {
        let standing = box_mesh(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));
        assert!(supports_for(&standing).is_empty());
    }

    #[test]
    fn a_box_floating_over_the_plate_is_an_island_and_is_held_from_underneath() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 20.0));
        let points = supports_for(&floating);

        assert!(!points.is_empty(), "an island must be held up");
        for point in &points {
            assert!(
                (0.0..=10.0).contains(&point.x) && (0.0..=10.0).contains(&point.y),
                "{point} is not under the island"
            );
            assert!(
                (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
                "{point} is not on the underside of the island"
            );
        }
    }

    #[test]
    fn an_island_is_held_at_the_spacing_the_profile_asks_for() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
        let points = supports_for(&floating);
        let spacing = SampleConfig::for_profile(&profile()).spacing_mm;

        for row in 1..30 {
            for column in 1..30 {
                let probe = Vec2::new(column as Scalar, row as Scalar);
                let nearest = points
                    .iter()
                    .map(|point| (point.truncate() - probe).length())
                    .fold(Scalar::INFINITY, Scalar::min);
                assert!(
                    nearest <= spacing,
                    "{probe} is {nearest} mm from the nearest support, past {spacing}"
                );
            }
        }
    }

    #[test]
    fn a_ball_resting_on_the_plate_is_held_under_its_lower_cap() {
        let points = supports_for(&sphere(15.0, 0.0));
        assert!(
            points.len() >= 8,
            "a 30 mm ball hangs over nothing all round its lower cap, got {} supports",
            points.len()
        );

        // Everything leaning further than 45 degrees is below the ball's own equator by
        // the radius over root two, and its supports stand under that cap.
        for point in &points {
            assert!(
                point.z <= 15.0 - 15.0 / 2.0_f32.sqrt() + 1.0,
                "{point} is above the latitude that holds itself up"
            );
        }
    }

    #[test]
    fn a_wall_leaning_inside_the_angle_holds_itself_up() {
        // A pyramid standing on its base: every face leans 45 degrees inwards.
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(20.0, 20.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(10.0, 10.0, 10.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [0, 1, 4],
                [1, 2, 4],
                [2, 3, 4],
                [3, 0, 4],
            ],
        );
        assert!(supports_for(&mesh).is_empty());
    }

    #[test]
    fn a_table_on_a_leg_is_held_around_its_coast_and_not_over_the_leg() {
        let leg = box_mesh(Vec3::new(10.0, 10.0, 0.0), Vec3::new(14.0, 14.0, 10.0));
        let table = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(24.0, 24.0, 12.0));
        let points = supports_for(&merged(&leg, &table));

        assert!(points.len() >= 4, "a 24 mm table needs holding all round");
        for point in &points {
            assert!(
                (point.z - 10.0).abs() < LAYER_HEIGHT_MM,
                "{point} is not on the underside of the table"
            );
            let over_the_leg = (10.0..=14.0).contains(&point.x) && (10.0..=14.0).contains(&point.y);
            assert!(!over_the_leg, "{point} stands where the leg already holds");
        }
    }

    #[test]
    fn a_support_that_is_already_there_holds_what_is_around_it() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(4.0, 4.0, 14.0));
        assert!(!supports_for(&floating).is_empty(), "a bare island is held");

        let seeded = generate_supports(
            &slice(&floating),
            LAYER_HEIGHT_MM,
            &profile(),
            &[Vec3::new(2.0, 2.0, 10.0)],
            None,
            &mut |_| true,
        );
        assert!(
            seeded.is_empty(),
            "a hand-placed support must not be doubled up on, got {seeded:?}"
        );
    }

    #[test]
    fn a_support_under_one_island_does_not_hold_the_island_over_it() {
        // Two slabs with 0.5 mm of air between them. The upper one is well inside the
        // reach of what holds the lower one, and it is a part of its own; see
        // `docs/decisions/0081`.
        let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 11.0));
        let upper = box_mesh(Vec3::new(0.0, 0.0, 11.5), Vec3::new(10.0, 10.0, 12.5));
        let points = supports_for(&merged(&lower, &upper));

        let held_at = |z: Scalar| {
            points
                .iter()
                .filter(|point| (point.z - z).abs() < 0.5)
                .count()
        };
        assert!(held_at(10.0) >= 4, "the lower island is held");
        assert!(
            held_at(11.5) >= 4,
            "the upper island is a part of its own and is held on its own account, got \
             {} supports under it",
            held_at(11.5)
        );
    }

    #[test]
    fn a_speck_too_small_to_print_is_left_alone() {
        let speck = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.15, 0.15, 10.4));
        assert!(supports_for(&speck).is_empty());
    }

    #[test]
    fn a_shallower_angle_puts_down_more_supports() {
        let ball = sphere(15.0, 0.0);
        let stack = slice(&ball);
        let loose = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                max_overhang_deg: 60.0,
                ..profile()
            },
            &[],
            None,
            &mut |_| true,
        );
        let strict = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                max_overhang_deg: 30.0,
                ..profile()
            },
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            strict.len() > loose.len(),
            "30 degrees placed {} against 60 degrees' {}",
            strict.len(),
            loose.len()
        );
    }

    #[test]
    fn a_denser_profile_puts_down_more_supports() {
        let ball = sphere(15.0, 0.0);
        let stack = slice(&ball);
        let normal = generate_supports(&stack, LAYER_HEIGHT_MM, &profile(), &[], None, &mut |_| {
            true
        });
        let dense = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &SupportProfile {
                density: 3.0,
                ..profile()
            },
            &[],
            None,
            &mut |_| true,
        );
        assert!(
            dense.len() > normal.len(),
            "three times the density put down {} against {}",
            dense.len(),
            normal.len()
        );
    }

    #[test]
    fn a_run_that_is_stopped_before_it_starts_places_nothing() {
        let floating = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(30.0, 30.0, 20.0));
        let stack = slice(&floating);

        let mut looked = 0;
        let stopped =
            generate_supports(&stack, LAYER_HEIGHT_MM, &profile(), &[], None, &mut |_| {
                looked += 1;
                false
            });
        assert!(stopped.is_empty());
        assert_eq!(
            looked, 1,
            "the run stopped at the first layer it asked about"
        );
    }

    #[test]
    fn stopping_part_way_keeps_what_the_run_had_already_found() {
        let lower = box_mesh(Vec3::new(0.0, 0.0, 10.0), Vec3::new(10.0, 10.0, 12.0));
        let upper = box_mesh(Vec3::new(20.0, 0.0, 20.0), Vec3::new(30.0, 10.0, 22.0));
        let stack = slice(&merged(&lower, &upper));

        let half = stack.layers.len() / 2;
        let stopped = generate_supports(
            &stack,
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            None,
            &mut |layer| layer < half,
        );

        assert!(!stopped.is_empty(), "the lower island was reached");
        for point in &stopped {
            assert!(point.x < 20.0, "{point} is on the island never reached");
        }
        let whole = generate_supports(&stack, LAYER_HEIGHT_MM, &profile(), &[], None, &mut |_| {
            true
        });
        assert!(whole.len() > stopped.len());
    }

    #[test]
    fn the_same_model_always_produces_the_same_supports() {
        let ball = sphere(15.0, 3.0);
        let stack = slice(&ball);
        let once = generate_supports(&stack, LAYER_HEIGHT_MM, &profile(), &[], None, &mut |_| {
            true
        });
        let twice = generate_supports(&stack, LAYER_HEIGHT_MM, &profile(), &[], None, &mut |_| {
            true
        });
        assert_eq!(once, twice);
    }

    #[test]
    fn an_empty_stack_produces_nothing() {
        let points = generate_supports(
            &Sliced::default(),
            LAYER_HEIGHT_MM,
            &profile(),
            &[],
            None,
            &mut |_| true,
        );
        assert!(points.is_empty());
    }
}
