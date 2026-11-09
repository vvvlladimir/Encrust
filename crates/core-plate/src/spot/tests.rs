use super::*;

fn settings() -> ArrangeSettings {
    ArrangeSettings {
        clearance_mm: 3.0,
        margin_mm: 2.0,
        ..ArrangeSettings::default()
    }
}

fn rect(min: (Scalar, Scalar), max: (Scalar, Scalar)) -> Rect {
    Rect {
        min: Vec2::new(min.0, min.1),
        max: Vec2::new(max.0, max.1),
    }
}

const PLATE: Vec2 = Vec2::new(100.0, 60.0);

#[test]
fn a_part_with_room_where_it_is_asked_stays_there() {
    let at = nearest_free(
        Vec2::splat(4.0),
        &[],
        Vec2::new(30.0, 30.0),
        PLATE,
        &settings(),
    );
    assert_eq!(at, Some(Vec2::new(30.0, 30.0)));
}

#[test]
fn a_part_asked_onto_a_model_moves_to_its_nearest_side() {
    // A model from x 20 to 40; the part asked 2 mm left of its right side.
    let model = rect((20.0, 10.0), (40.0, 50.0));
    let at = nearest_free(
        Vec2::splat(4.0),
        &[model],
        Vec2::new(38.0, 30.0),
        PLATE,
        &settings(),
    )
    .expect("the plate has room");
    // Its right side, the 3 mm clearance and half the 4 mm part: x = 45.
    assert!(
        (at - Vec2::new(45.0, 30.0)).length() < 1e-5,
        "the part stands 3 mm right of the model, got {at}"
    );
}

#[test]
fn a_part_asked_off_the_plate_is_brought_back_inside_its_margin() {
    let at = nearest_free(
        Vec2::splat(4.0),
        &[],
        Vec2::new(120.0, 30.0),
        PLATE,
        &settings(),
    );
    // 100 mm less the 2 mm margin and half the part.
    assert_eq!(at, Some(Vec2::new(96.0, 30.0)));
}

#[test]
fn a_part_between_two_models_takes_the_gap_only_where_it_fits() {
    // Two models 8 mm apart: a 4 mm part needs 3 + 4 + 3 = 10 mm, so it goes round.
    let left = rect((10.0, 10.0), (40.0, 50.0));
    let right = rect((48.0, 10.0), (80.0, 50.0));
    let at = nearest_free(
        Vec2::splat(4.0),
        &[left, right],
        Vec2::new(44.0, 30.0),
        PLATE,
        &settings(),
    )
    .expect("the plate has room");
    assert!(
        at.y <= 10.0 - 5.0 + 1e-5 || at.y >= 50.0 + 5.0 - 1e-5,
        "the part clears both models above or below them, got {at}"
    );
}

/// Every crossing of the grown boxes' sides tried, the slow way the walk must agree with.
fn by_every_crossing(size: Vec2, taken: &[Rect], toward: Vec2) -> Option<Vec2> {
    let settings = settings();
    let half = size / 2.0;
    let room = Rect {
        min: Vec2::splat(settings.margin_mm) + half,
        max: PLATE - Vec2::splat(settings.margin_mm) - half,
    };
    // The nearest free point is `toward` itself or lies on a side of a box grown by the part,
    // so only those sides are walked, nearest first: exact, with no grid.
    let reach = half + Vec2::splat(settings.clearance_mm);
    let grown: Vec<Rect> = taken
        .iter()
        .map(|taken| Rect {
            min: taken.min - reach,
            max: taken.max + reach,
        })
        .collect();
    let lines = |axis: fn(Vec2) -> Scalar| -> Vec<Scalar> {
        let sides = grown
            .iter()
            .flat_map(|rect| [axis(rect.min), axis(rect.max)]);
        [axis(toward), axis(room.min), axis(room.max)]
            .into_iter()
            .chain(sides)
            .collect()
    };
    let (xs, ys) = (lines(|at| at.x), lines(|at| at.y));
    xs.iter()
        .flat_map(|x| ys.iter().map(move |y| Vec2::new(*x, *y)))
        .map(|at| at.clamp(room.min, room.max))
        .filter(|at| grown.iter().all(|rect| !strictly_inside(rect, *at)))
        .min_by(|a, b| {
            a.distance_squared(toward)
                .total_cmp(&b.distance_squared(toward))
        })
}

#[test]
fn walking_the_sides_finds_as_near_a_spot_as_trying_every_crossing() {
    let mut seed = 0x2545_f491_u32;
    let mut next = |span: Scalar| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        span * (seed % 10_000) as Scalar / 10_000.0
    };
    for _ in 0..300 {
        let taken: Vec<Rect> = (0..=next(12.0) as usize)
            .map(|_| {
                let min = Vec2::new(next(100.0), next(60.0));
                Rect {
                    min,
                    max: min + Vec2::new(1.0 + next(30.0), 1.0 + next(20.0)),
                }
            })
            .collect();
        let (size, toward) = (
            Vec2::splat(1.0 + next(8.0)),
            Vec2::new(next(110.0), next(70.0)),
        );
        let walked = nearest_free(size, &taken, toward, PLATE, &settings());
        let tried = by_every_crossing(size, &taken, toward);
        let distance = |at: Option<Vec2>| at.map(|at| at.distance(toward));
        match (distance(walked), distance(tried)) {
            (Some(walked), Some(tried)) => assert!(
                (walked - tried).abs() < 1e-3,
                "{walked} mm off against {tried} mm for {taken:?} toward {toward}"
            ),
            (walked, tried) => assert_eq!(walked, tried, "{taken:?} toward {toward}"),
        }
    }
}

#[test]
fn a_plate_with_no_room_left_says_so() {
    let everything = rect((0.0, 0.0), (100.0, 60.0));
    let at = nearest_free(
        Vec2::splat(4.0),
        &[everything],
        Vec2::new(50.0, 30.0),
        PLATE,
        &settings(),
    );
    assert_eq!(at, None);
}

fn square(x: Scalar, y: Scalar) -> Rect {
    Rect {
        min: Vec2::new(x, y),
        max: Vec2::new(x + 20.0, y + 20.0),
    }
}

#[test]
fn a_part_past_the_area_moves_in_only_as_far_as_its_margin() {
    let area = PlateArea {
        low_mm: Vec2::new(10.0, 0.0),
        high_mm: Vec2::new(150.0, 80.0),
    };
    let moved = clear_of(square(140.0, 30.0), &[], area, &settings()).expect("the area is empty");
    let margin = settings().margin_mm;
    assert!(
        (160.0 + moved.x - (150.0 - margin)).abs() < 1e-3 && moved.y.abs() < 1e-3,
        "the far edge at 160 mm comes back to {} mm, moved by {moved}",
        150.0 - margin
    );
}

#[test]
fn a_part_steps_off_its_neighbour_by_the_clearance() {
    let moved = clear_of(
        square(30.0, 30.0),
        &[square(20.0, 30.0)],
        PlateArea::whole(PLATE),
        &settings(),
    )
    .expect("the plate has room");
    let clearance = settings().clearance_mm;
    assert!(
        (moved.x - (10.0 + clearance)).abs() < 1e-3 && moved.y.abs() < 1e-3,
        "the nearest way off is {} mm along X, moved by {moved}",
        10.0 + clearance
    );
}

#[test]
fn a_part_with_no_room_in_the_area_is_not_moved() {
    let narrow = PlateArea {
        low_mm: Vec2::new(100.0, 0.0),
        high_mm: Vec2::new(110.0, 80.0),
    };
    assert_eq!(clear_of(square(0.0, 0.0), &[], narrow, &settings()), None);
}

#[test]
fn only_what_is_in_the_way_moves_and_of_a_meeting_the_smaller() {
    let parts = [
        square(30.0, 30.0),
        Rect {
            min: Vec2::new(45.0, 35.0),
            max: Vec2::new(55.0, 45.0),
        },
        square(90.0, 30.0),
    ];
    let cleared = move_clear(
        &parts,
        &[2],
        &[[0, 1]],
        PlateArea::whole(PLATE),
        &settings(),
    );
    let moved: Vec<usize> = cleared.moved.iter().map(|placed| placed.index).collect();
    assert_eq!(
        moved,
        vec![2, 1],
        "the box past the plate and the smaller of the pair, largest first"
    );
    assert!(cleared.left_out.is_empty());
}

#[test]
fn a_part_with_no_room_is_left_out_and_still_counts_as_standing() {
    let parts = [square(0.0, 0.0), square(0.0, 0.0)];
    let tiny = PlateArea::whole(Vec2::new(26.0, 26.0));
    let cleared = move_clear(&parts, &[0, 1], &[], tiny, &settings());
    assert_eq!(
        cleared.moved.len(),
        1,
        "one 20 mm box fits 26 mm with its margins"
    );
    assert_eq!(cleared.left_out.len(), 1);
}
