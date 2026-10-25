use super::*;
use core_slicer::Contour;

/// A tenth of a millimetre a cell, the grid placement reads layers on.
const PITCH_MM: Scalar = 0.1;

pub(super) fn grid() -> Grid {
    Grid::covering(Vec2::ZERO, Vec2::splat(40.0), PITCH_MM)
}

/// One axis-aligned rectangle, read onto the grid.
pub(super) fn rectangle(x: Scalar, y: Scalar, width: Scalar, height: Scalar) -> Field {
    let points = vec![
        Vec2::new(x, y),
        Vec2::new(x + width, y),
        Vec2::new(x + width, y + height),
        Vec2::new(x, y + height),
    ];
    let layer = Layer {
        z: 1.0,
        contours: vec![Contour::from_points(points).expect("a rectangle encloses area")],
        extra: Vec::new(),
    };
    Field::of_layer(&layer, &grid())
}

pub(super) fn square(x: Scalar, y: Scalar, side: Scalar) -> Field {
    rectangle(x, y, side, side)
}

#[test]
fn a_square_covers_the_cells_its_own_area_comes_to() {
    let field = square(10.0, 10.0, 5.0);
    let area = field.area_mm2(&grid());
    assert!(
        (area - 25.0).abs() < 0.2,
        "a 5 mm square covers 25 mm2, got {area}"
    );
}

#[test]
fn a_square_lands_where_it_was_drawn() {
    let field = square(10.0, 20.0, 5.0);
    let grid = grid();
    let (min, max) = field.bounds().expect("the square has cells");

    let low = grid.centre_mm(min[0], min[1]);
    let high = grid.centre_mm(max[0], max[1]);
    assert!(
        (low - Vec2::new(10.0, 20.0)).length() < 2.0 * PITCH_MM,
        "the low corner landed at {low}"
    );
    assert!(
        (high - Vec2::new(15.0, 25.0)).length() < 2.0 * PITCH_MM,
        "the high corner landed at {high}"
    );
}

#[test]
fn a_hole_in_a_layer_is_a_hole_in_the_field() {
    let mut points = vec![
        Vec2::new(13.0, 13.0),
        Vec2::new(17.0, 13.0),
        Vec2::new(17.0, 17.0),
        Vec2::new(13.0, 17.0),
    ];
    points.reverse();
    let layer = Layer {
        z: 1.0,
        contours: vec![
            Contour::from_points(vec![
                Vec2::new(10.0, 10.0),
                Vec2::new(20.0, 10.0),
                Vec2::new(20.0, 20.0),
                Vec2::new(10.0, 20.0),
            ])
            .expect("the outside encloses area"),
            Contour::from_points(points).expect("the hole encloses area"),
        ],
        extra: Vec::new(),
    };

    let field = Field::of_layer(&layer, &grid());
    let area = field.area_mm2(&grid());
    assert!(
        (area - (100.0 - 16.0)).abs() < 0.5,
        "a 10 mm square with a 4 mm hole covers 84 mm2, got {area}"
    );
    assert!(!field.contains(150, 150), "the middle is a hole");
    assert!(field.contains(110, 110), "the rim is material");
}

#[test]
fn an_empty_layer_is_an_empty_field() {
    let field = Field::of_layer(&Layer::empty(1.0), &grid());
    assert!(field.is_empty());
    assert!(field.bounds().is_none());
    assert_eq!(field.cell_count(), 0);
    assert!(field.pieces().is_empty());
}

#[test]
fn taking_one_field_out_of_another_leaves_what_it_did_not_cover() {
    let block = square(10.0, 10.0, 10.0);
    let bite = square(10.0, 10.0, 5.0);
    let left = block.without(&bite);

    let area = left.area_mm2(&grid());
    assert!(
        (area - 75.0).abs() < 0.5,
        "100 mm2 less the 25 taken out, got {area}"
    );
    assert!(block.without(&block).is_empty());
}

#[test]
fn two_fields_touch_only_where_they_share_a_cell() {
    let here = square(10.0, 10.0, 5.0);
    let there = square(20.0, 20.0, 5.0);
    assert!(!here.touches(&there));
    assert!(here.touches(&here));
    assert!(here.touches(&square(12.0, 12.0, 5.0)));
}

#[test]
fn growing_a_square_pushes_every_side_out_by_the_radius() {
    let grown = square(10.0, 10.0, 10.0).grown(20);
    let area = grown.area_mm2(&grid());
    // A 10 mm square grown by 2 mm: the square, four 2 mm sides, four quarter discs.
    let expected = 14.0f32.mul_add(14.0, -(4.0 * 4.0)) + std::f32::consts::PI * 4.0;
    assert!(
        (area - expected).abs() / expected < 0.02,
        "expected about {expected} mm2, got {area}"
    );
}

#[test]
fn growing_rounds_the_corners_instead_of_squaring_them() {
    let grown = square(10.0, 10.0, 10.0).grown(20);
    let grid = grid();
    let corner = grid.cell_of(Vec2::new(21.8, 21.8));
    assert!(
        !grown.contains(corner[0], corner[1]),
        "a square corner would claim to reach 2.8 mm across the diagonal"
    );
}

#[test]
fn shrinking_a_square_pulls_every_side_in_by_the_radius() {
    let shrunk = square(10.0, 10.0, 10.0).shrunk(20);
    let area = shrunk.area_mm2(&grid());
    assert!(
        (area - 36.0).abs() / 36.0 < 0.05,
        "a 10 mm square pulled in 2 mm is a 6 mm one, got {area}"
    );
    assert!(
        square(10.0, 10.0, 1.0).shrunk(20).is_empty(),
        "a square narrower than the radius has nothing left"
    );
}

#[test]
fn a_cell_reaches_what_carries_it_and_nothing_else() {
    // An L of two rectangles against a ring of four, so rows meet spans on both
    // sides and at every distance the radii below span.
    let piece = rectangle(10.0, 10.0, 6.0, 1.5).with(&rectangle(10.0, 10.0, 1.5, 6.0));
    let ring = square(8.0, 8.0, 10.0).without(&square(9.0, 9.0, 8.0));
    for radius in [0, 1, 3, 7, 20] {
        let left = piece.uncarried_by(&ring, radius);
        for (y, spans) in piece.rows() {
            for x in spans.iter().flat_map(|span| span.x0..span.x1) {
                assert_eq!(
                    !ring.reaches(x, y, radius),
                    left.contains(x, y),
                    "cell ({x}, {y}) at radius {radius}"
                );
            }
        }
    }
}

#[test]
fn the_rim_of_a_square_is_its_own_outline() {
    let field = square(10.0, 10.0, 5.0);
    let rim = field.rim();
    let grid = grid();

    assert!(!rim.is_empty());
    assert!(
        rim.area_mm2(&grid) < field.area_mm2(&grid) / 4.0,
        "an outline one cell thick is a sliver of the area"
    );
    let (min, max) = field.bounds().expect("the square has cells");
    assert!(rim.contains(min[0], min[1]), "the corner is on the rim");
    assert!(
        !rim.contains(i32::midpoint(min[0], max[0]), i32::midpoint(min[1], max[1])),
        "the middle is not"
    );
}

/// A circle of `sides` points, fine enough that thinning it has something to do.
fn circle(centre: Vec2, radius: Scalar, sides: usize, from: usize) -> Field {
    let points: Vec<Vec2> = (0..sides)
        .map(|step| {
            let angle = std::f32::consts::TAU * ((step + from) % sides) as Scalar / sides as Scalar;
            centre + Vec2::new(radius * angle.cos(), radius * angle.sin())
        })
        .collect();
    let layer = Layer {
        z: 1.0,
        contours: vec![Contour::from_points(points).expect("a circle encloses area")],
        extra: Vec::new(),
    };
    Field::of_layer(&layer, &grid())
}

#[test]
fn a_ring_read_from_a_different_vertex_covers_the_same_cells() {
    // Two layers of one wall are the same ring stitched from different vertices; thinning them
    // differently would put a support under a wall.
    let here = circle(Vec2::splat(20.0), 8.0, 256, 0);
    let there = circle(Vec2::splat(20.0), 8.0, 256, 97);
    assert_eq!(here, there);
    assert!(here.without(&there).is_empty());
}

#[test]
fn thinning_a_ring_keeps_the_area_it_encloses() {
    let fine = circle(Vec2::splat(20.0), 8.0, 512, 0);
    let area = fine.area_mm2(&grid());
    let expected = std::f32::consts::PI * 8.0 * 8.0;
    assert!(
        (area - expected).abs() / expected < 0.01,
        "an 8 mm circle covers {expected} mm2, got {area}"
    );
}

#[test]
fn what_is_uncarried_is_what_growing_would_have_left_behind() {
    let over = square(10.0, 10.0, 14.0);
    let under = square(10.0, 10.0, 10.0);
    for radius in [1, 7, 20] {
        assert_eq!(
            over.uncarried_by(&under, radius),
            over.without(&under.grown(radius)),
            "growing by {radius} cells and subtracting must give the same field"
        );
    }
}

#[test]
fn a_field_lying_next_to_another_adjoins_it_without_touching_it() {
    let here = square(10.0, 10.0, 5.0);
    let (min, max) = here.bounds().expect("the square has cells");

    // One cell to the right of its right-hand edge: no cell in common, side by side.
    let beside = Field::of_spans(vec![(
        min[1],
        Span {
            x0: max[0] + 1,
            x1: max[0] + 2,
        },
    )]);
    assert!(!here.touches(&beside));
    assert!(here.adjoins(&beside));

    // One cell up and across from its corner: no edge in common, only a corner.
    let corner = Field::of_spans(vec![(
        max[1] + 1,
        Span {
            x0: max[0] + 1,
            x1: max[0] + 2,
        },
    )]);
    assert!(here.adjoins(&corner), "a corner is still next to it");

    let away = square(30.0, 30.0, 5.0);
    assert!(!here.adjoins(&away));
}
