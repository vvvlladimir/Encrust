//! Points given with no `objects` while several stand on the plate, each handed to the object it
//! lies at, so a client need not name what the command line would ask it to (ADR 0458).

use anyhow::{Result, bail};

use super::scoped::ForObjects;
use super::{Drains, Hollow, Supports};

/// One object's box on the plate as the latest check reports it: its id, then its low and high
/// corners, mm.
pub type ObjectBox = (u64, [[f32; 3]; 2]);

/// Nearer than this to two boxes a point could be on either object, mm.
const EITHER_MM: f32 = 0.5;

/// A step whose points stand on one object each.
pub trait PlatePoints: ForObjects + Clone {
    /// The point that places each of its points or groups of points, mm.
    fn anchors(&self) -> Vec<[f32; 3]>;
    /// Keeps the points and groups whose anchor `keep` takes.
    fn keep_points(&mut self, keep: &dyn Fn([f32; 3]) -> bool);
    fn name_objects(&mut self, ids: Vec<u64>);
}

/// `asked` as it stands where it names objects, has no points or there is one object; else the
/// step for every object without the points, then one for each object holding its own.
pub fn split_by_object<T: PlatePoints>(asked: T, boxes: &[ObjectBox]) -> Result<Vec<T>> {
    if !asked.objects().is_empty() || boxes.len() < 2 || asked.anchors().is_empty() {
        return Ok(vec![asked]);
    }
    let mut owners = Vec::new();
    for anchor in asked.anchors() {
        owners.push(owner_of(anchor, boxes)?);
    }
    let mut every = asked.clone();
    every.keep_points(&|_| false);
    let mut steps = vec![every];
    owners.sort_unstable();
    owners.dedup();
    for id in owners {
        let mut one = asked.clone();
        one.keep_points(&|point| owner_of(point, boxes).is_ok_and(|owner| owner == id));
        one.name_objects(vec![id]);
        steps.push(one);
    }
    Ok(steps)
}

/// The object whose box `point` lies in or nearest; refused where two are as near.
fn owner_of(point: [f32; 3], boxes: &[ObjectBox]) -> Result<u64> {
    let mut by_distance: Vec<(f32, u64)> = (boxes.iter())
        .map(|(id, corners)| (distance_to_box(point, corners), *id))
        .collect();
    by_distance.sort_by(|a, b| a.0.total_cmp(&b.0));
    match by_distance.as_slice() {
        [(near, first), (next, second), ..] if next - near < EITHER_MM => bail!(
            "the point {point:?} lies at objects {first} and {second} alike; name one in `objects`"
        ),
        [(_, first), ..] => Ok(*first),
        [] => bail!("the plate has no object for the point {point:?}"),
    }
}

fn distance_to_box(point: [f32; 3], [low, high]: &[[f32; 3]; 2]) -> f32 {
    let outside = |axis: usize| {
        (low[axis] - point[axis])
            .max(point[axis] - high[axis])
            .max(0.0)
    };
    (0..3).map(outside).map(|d| d * d).sum::<f32>().sqrt()
}

fn first_of(group: &[[f32; 3]]) -> Option<[f32; 3]> {
    group.first().copied()
}

impl PlatePoints for Drains {
    fn anchors(&self) -> Vec<[f32; 3]> {
        let groups = self.channels.iter().filter_map(|channel| first_of(channel));
        (self.at_mm.iter().chain(&self.support_at_mm).copied())
            .chain(groups)
            .collect()
    }

    fn keep_points(&mut self, keep: &dyn Fn([f32; 3]) -> bool) {
        self.at_mm.retain(|point| keep(*point));
        self.support_at_mm.retain(|point| keep(*point));
        (self.channels).retain(|channel| first_of(channel).is_some_and(keep));
    }

    fn name_objects(&mut self, ids: Vec<u64>) {
        self.objects = ids;
    }
}

impl PlatePoints for Supports {
    fn anchors(&self) -> Vec<[f32; 3]> {
        let groups = self.merge_at_mm.iter().filter_map(|group| first_of(group));
        (self.at_mm.iter().chain(&self.shown_at_mm).copied())
            .chain(groups)
            .collect()
    }

    fn keep_points(&mut self, keep: &dyn Fn([f32; 3]) -> bool) {
        self.at_mm.retain(|point| keep(*point));
        self.shown_at_mm.retain(|point| keep(*point));
        (self.merge_at_mm).retain(|group| first_of(group).is_some_and(keep));
    }

    fn name_objects(&mut self, ids: Vec<u64>) {
        self.objects = ids;
    }
}

impl PlatePoints for Hollow {
    fn anchors(&self) -> Vec<[f32; 3]> {
        self.keep_solid_at_mm.clone()
    }

    fn keep_points(&mut self, keep: &dyn Fn([f32; 3]) -> bool) {
        self.keep_solid_at_mm.retain(|point| keep(*point));
    }

    fn name_objects(&mut self, ids: Vec<u64>) {
        self.objects = ids;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALVES: [ObjectBox; 2] = [
        (2, [[0.0, 0.0, 0.0], [100.0, 80.0, 40.0]]),
        (3, [[110.0, 0.0, 0.0], [210.0, 80.0, 80.0]]),
    ];

    fn drains_at(at_mm: Vec<[f32; 3]>) -> Drains {
        Drains {
            diameter_mm: 3.0,
            auto: true,
            at_mm,
            ..Drains::default()
        }
    }

    #[test]
    fn a_point_on_one_half_goes_to_that_half_and_every_half_keeps_the_rest() {
        let asked = drains_at(vec![[30.0, 40.0, 8.0], [170.0, 50.0, 44.0]]);

        let steps = split_by_object(asked, &HALVES).expect("each point lies on one half");

        assert_eq!(
            steps.len(),
            3,
            "every half, then each half with its point: {steps:?}"
        );
        assert!(
            steps[0].objects.is_empty() && steps[0].at_mm.is_empty(),
            "{:?}",
            steps[0]
        );
        assert!(steps[0].auto, "the search stays for every half");
        assert_eq!(
            (steps[1].objects.as_slice(), steps[1].at_mm.as_slice()),
            (&[2][..], &[[30.0, 40.0, 8.0]][..])
        );
        assert_eq!(
            (steps[2].objects.as_slice(), steps[2].at_mm.as_slice()),
            (&[3][..], &[[170.0, 50.0, 44.0]][..])
        );
    }

    #[test]
    fn a_point_just_outside_a_box_goes_to_the_nearest() {
        let steps = split_by_object(drains_at(vec![[102.0, 40.0, 8.0]]), &HALVES)
            .expect("2 mm from one half, 8 mm from the other");

        assert_eq!(steps[1].objects, [2], "{steps:?}");
    }

    #[test]
    fn a_point_in_two_boxes_asks_for_the_object() {
        let overlapping = [HALVES[0], (3, [[90.0, 0.0, 0.0], [210.0, 80.0, 80.0]])];

        let refused = split_by_object(drains_at(vec![[95.0, 40.0, 8.0]]), &overlapping);

        assert!(refused.is_err(), "either half could hold it");
    }

    #[test]
    fn a_step_naming_its_objects_or_one_object_on_the_plate_is_left_as_asked() {
        let named = Drains {
            objects: vec![3],
            ..drains_at(vec![[30.0, 40.0, 8.0]])
        };
        let one = [HALVES[0]];

        assert_eq!(
            split_by_object(named.clone(), &HALVES).expect("named"),
            [named]
        );
        let alone = drains_at(vec![[30.0, 40.0, 8.0]]);
        assert_eq!(
            split_by_object(alone.clone(), &one).expect("one object"),
            [alone]
        );
    }

    #[test]
    fn a_group_of_merged_supports_goes_where_its_first_point_lies() {
        let asked = Supports {
            profile: "auto".into(),
            merge_at_mm: vec![vec![[150.0, 10.0, 5.0], [151.0, 10.0, 5.0]]],
            ..Supports::default()
        };

        let steps = split_by_object(asked, &HALVES).expect("on the second half");

        assert!(steps[0].merge_at_mm.is_empty(), "{steps:?}");
        assert_eq!(steps[1].objects, [3], "{steps:?}");
        assert_eq!(steps[1].merge_at_mm.len(), 1, "{steps:?}");
    }
}
