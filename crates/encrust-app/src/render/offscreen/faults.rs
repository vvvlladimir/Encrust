use super::SIZE;
use super::marks::Marks;
use super::paint::paint;
use crate::camera::OrbitCamera;
use crate::render::gpu::ModelDraw;
use crate::render::vertex::{LineVertex, ModelInstance, NOT_MARKED, WideLine};
use crate::ui::theme;
use core_geometry::{Mesh, Region, Transform, Vec3, cuboid};
use std::sync::Arc;

/// A 40 mm cube about where the camera looks, and the two faces of its top.
fn cube_and_top() -> (Arc<Mesh>, Region) {
    let mesh = cuboid(Vec3::new(55.0, 20.0, 25.0), Vec3::new(95.0, 60.0, 65.0));
    let mut top = Region::default();
    for face in 0..mesh.faces.len() {
        let on_top = mesh.faces[face]
            .iter()
            .all(|&corner| (mesh.vertices[corner as usize].z - 65.0).abs() < 1e-4);
        top.set(face, on_top);
    }
    (Arc::new(mesh), top)
}

fn frame(models: &[ModelDraw], lines: &[LineVertex]) -> Option<Vec<u8>> {
    let marks = Marks {
        lines,
        ..Marks::default()
    };
    paint(camera(), None, models, &[], marks)
}

/// A camera with the whole cube in view, from above the front.
fn camera() -> OrbitCamera {
    OrbitCamera {
        target: Vec3::new(75.0, 40.0, 45.0),
        distance_mm: 170.0,
        ..OrbitCamera::default()
    }
}

/// The pixels that differ between two frames, and how many of those are the fault's
/// yellow-green however lit: green over blue by a clear margin, which the resin never is.
fn changed_and_marked(plain: &[u8], marked: &[u8]) -> (usize, usize) {
    let pairs = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(marked.as_chunks::<4>().0);
    let changed: Vec<&[u8; 4]> = pairs.filter(|(a, b)| a != b).map(|(_, b)| b).collect();
    let fault = changed
        .iter()
        .filter(|pixel| i32::from(pixel[1]) > i32::from(pixel[2]) + 40)
        .count();
    (changed.len(), fault)
}

/// A patch of the faces the Repair tool marks, stood off the surface, wins the depth over
/// the faces it lies on every frame: the top reads in the fault colour, not in stripes.
#[test]
fn a_marked_patch_covers_the_faces_it_lies_on() {
    let (mesh, top) = cube_and_top();
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let patch = top
        .submesh(&mesh, Transform::default(), 0.05)
        .expect("the top has two faces");
    let fault = ModelInstance::new(Transform::default(), theme::scene().fault, NOT_MARKED);
    let cube = ModelDraw::whole(Arc::clone(&mesh), object);
    let Some(plain) = frame(std::slice::from_ref(&cube), &[]) else {
        return;
    };
    let marked = frame(&[cube, ModelDraw::whole(Arc::new(patch), fault)], &[])
        .expect("the adapter drew once");

    let (changed, marked_px) = changed_and_marked(&plain, &marked);
    let share = SIZE as usize * SIZE as usize / 100;
    assert!(
        changed > share,
        "the top covers more than 1 % of the frame: {changed} px"
    );
    assert!(
        marked_px * 100 >= changed * 95,
        "{marked_px} of {changed} changed pixels are the fault colour"
    );
}

/// An open edge drawn as a line on the rim of a face shows along it rather than sinking
/// into the face it bounds.
#[test]
fn a_marked_edge_shows_along_the_face_it_bounds() {
    let (mesh, _) = cube_and_top();
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cube = [ModelDraw::whole(mesh, object)];
    let colour = theme::scene().fault;
    let rim = [
        (55.0, 20.0),
        (95.0, 20.0),
        (95.0, 60.0),
        (55.0, 60.0),
        (55.0, 20.0),
    ];
    let lines: Vec<LineVertex> = rim
        .windows(2)
        .flat_map(|ends| {
            ends.iter()
                .map(|&(x, y)| LineVertex::new(Vec3::new(x, y, 65.0), colour))
        })
        .collect();
    let Some(plain) = frame(&cube, &[]) else {
        return;
    };
    let marked = frame(&cube, &lines).expect("the adapter drew once");

    let (changed, marked_px) = changed_and_marked(&plain, &marked);
    // The four sides of the rim run some 500 pixels across this frame; most must show.
    assert!(
        marked_px > 300,
        "{marked_px} of the rim's pixels show, {changed} changed"
    );
}

/// The same rim drawn as wide lines covers about its width in pixels along each side,
/// against the one pixel a plain line takes, and the faces beside it hide none of it.
#[test]
fn a_wide_marked_edge_covers_its_width_along_the_face_it_bounds() {
    let (mesh, _) = cube_and_top();
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cube = [ModelDraw::whole(mesh, object)];
    let corners = [
        (55.0, 20.0),
        (95.0, 20.0),
        (95.0, 60.0),
        (55.0, 60.0),
        (55.0, 20.0),
    ];
    let at = |(x, y)| Vec3::new(x, y, 65.0);
    let colour = theme::scene().fault;
    let size = core_geometry::Vec2::splat(SIZE as f32);
    let thin: Vec<LineVertex> = corners
        .windows(2)
        .flat_map(|ends| ends.iter().map(|&end| LineVertex::new(at(end), colour)))
        .collect();
    let wide: Vec<WideLine> = corners
        .windows(2)
        .map(|ends| WideLine::new([at(ends[0]), at(ends[1])], colour, 4.0, size))
        .collect();
    let Some(plain) = frame(&cube, &[]) else {
        return;
    };
    let hairline = frame(&cube, &thin).expect("the adapter drew once");
    let marks = Marks {
        wide_lines: &wide,
        ..Marks::default()
    };
    let bold = paint(camera(), None, &cube, &[], marks).expect("the adapter drew once");

    let (_, thin_px) = changed_and_marked(&plain, &hairline);
    let (_, wide_px) = changed_and_marked(&plain, &bold);
    assert!(
        wide_px > thin_px * 3,
        "4 px wide covers some four times the hairline: {wide_px} against {thin_px} px"
    );
}

/// An edge marked twice, as a picked open edge is over the rest, shows in the later mark's
/// colour: the marks are drawn in the order listed, and the later wins the depth they share.
#[test]
fn a_later_wide_mark_on_the_same_edge_shows_over_the_earlier() {
    let (mesh, _) = cube_and_top();
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cube = [ModelDraw::whole(mesh, object)];
    let ends = [Vec3::new(55.0, 20.0, 65.0), Vec3::new(95.0, 20.0, 65.0)];
    let size = core_geometry::Vec2::splat(SIZE as f32);
    let (rest, picked) = (theme::scene().fault_rest, theme::scene().fault_picked);
    let wide = [
        WideLine::new(ends, rest, 4.0, size),
        WideLine::new(ends, picked, 4.0, size),
    ];
    let Some(plain) = frame(&cube, &[]) else {
        return;
    };
    let marks = Marks {
        wide_lines: &wide,
        ..Marks::default()
    };
    let marked = paint(camera(), None, &cube, &[], marks).expect("the adapter drew once");

    let changed: Vec<&[u8; 4]> = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(marked.as_chunks::<4>().0)
        .filter(|(a, b)| a != b)
        .map(|(_, b)| b)
        .collect();
    let picked_px = changed.iter().filter(|pixel| pixel[0] > pixel[1]).count();
    assert!(
        !changed.is_empty() && picked_px * 100 >= changed.len() * 95,
        "{picked_px} of {} changed pixels are the picked colour, red over green",
        changed.len()
    );
}

/// A hole in the side of a box, away from where the camera looks: brought to it, the
/// camera frames the rim in the middle of the view, seen face on.
#[test]
fn the_camera_brought_to_a_hole_has_its_rim_in_the_middle_of_the_view() {
    let (mesh, _) = cube_and_top();
    let colour = theme::scene().fault;
    // The square of the face at x = 95, 5 mm across about its middle.
    let rim = [
        (37.5, 42.5),
        (42.5, 42.5),
        (42.5, 47.5),
        (37.5, 47.5),
        (37.5, 42.5),
    ];
    let lines: Vec<LineVertex> = rim
        .windows(2)
        .flat_map(|ends| {
            ends.iter()
                .map(|&(y, z)| LineVertex::new(Vec3::new(95.0, y, z), colour))
        })
        .collect();
    let spot =
        crate::highlight::Spot::around(rim.iter().map(|&(y, z)| Vec3::new(95.0, y, z)), Vec3::X)
            .expect("four corners");
    let camera = OrbitCamera::default().focusing(&spot, 1.0, |_, _| false);
    let object = ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED);
    let cube = [ModelDraw::whole(mesh, object)];
    let paint_at = |lines| {
        let marks = Marks {
            lines,
            ..Marks::default()
        };
        paint(camera, None, &cube, &[], marks)
    };
    let Some(plain) = paint_at(&[]) else {
        return;
    };
    let marked = paint_at(&lines).expect("the adapter drew once");

    let side = SIZE as usize;
    let changed: Vec<(usize, usize)> = plain
        .as_chunks::<4>()
        .0
        .iter()
        .zip(marked.as_chunks::<4>().0)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(at, _)| (at % side, at / side))
        .collect();
    assert!(changed.len() > 200, "the rim shows: {} px", changed.len());
    let middle = side as f32 / 2.0;
    let reach = changed
        .iter()
        .map(|&(x, y)| (x as f32 - middle).abs().max((y as f32 - middle).abs()))
        .fold(0.0, f32::max);
    let framed = middle * SPOT_SHARE_SEEN;
    assert!(
        reach < framed,
        "the rim stays within {framed} px of the middle; it reaches {reach}"
    );
    assert!(reach > framed / 3.0, "and is not a speck: {reach} px");
}

/// How far from the middle a framed spot's rim may reach, as a share of half the view:
/// the spot fills about a third of it, with room for the square's corners.
const SPOT_SHARE_SEEN: f32 = 0.6;
