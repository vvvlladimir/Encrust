//! Carrying held support parts square to the camera, or level with Alt, drawn refused where
//! `core_supports::eased` finds no room and kept where they last could stand (ADR 0438, 0440).

use std::sync::Arc;

use core_geometry::{Mesh, Transform, Vec3};
use core_supports::{Blocked, Carried, Part, Profiles, Refused, SupportTree};
use printer_profiles::SupportProfile;

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::pick::ray_through;
use crate::render::{Ghost, GhostShape};
use crate::scene::{ObjectId, SceneObject};
use crate::state::{Doc, Tools};
use crate::supports::{Carry, Picked};
use crate::ui::theme;
use crate::viewport_input::Pointer;
use crate::workspace::Tool;

use super::super::picking::{Aimed, grab_under_cursor, surface_under};

/// A ray flatter than this against the drag's plane, as the cosine between them, has no
/// crossing worth using: the part would shoot off to the horizon.
const PLANE_GRAZE: f32 = 0.05;

/// The frozen trees of one object a carry moves, each with its index.
type Moved = Vec<(usize, SupportTree)>;

/// Picks and carries the support parts the Edit mode works on; answers whether it has the
/// pointer. A press on a part carries it and the picked set; shift toggles; a miss is the camera's.
pub(in crate::panels::viewport_panel) fn drag_support(
    window: &mut Window,
    viewport: egui::Rect,
    pointer: &Pointer,
) -> bool {
    // Whatever tool is in hand by then, a carry ends with the button: nothing stays lifted.
    if !pointer.primary_down {
        set_down(window);
    }
    if *window.tool != Tool::Supports {
        return false;
    }
    if !pointer.primary_down {
        window.tools.supports.stroke = false;
        return false;
    }
    let Some(cursor) = pointer.position.map(|at| egui::Pos2::new(at.x, at.y)) else {
        return window.tools.supports.stroke;
    };

    if pointer.pressed {
        // A Cmd press is the click that picks a model, not a hold on a support.
        let on_plate = pointer.over_viewport && !pointer.command && !window.view.gizmo.is_focused();
        window.tools.supports.stroke =
            on_plate && take_hold(window, viewport, cursor, pointer.shift, pointer.alt);
        return window.tools.supports.stroke;
    }
    if !window.tools.supports.stroke {
        return false;
    }
    // A shift press only picks and carries nothing: it must not move the tip left in hand.
    let Some(level) = window
        .tools
        .supports
        .carry
        .as_ref()
        .map(|carry| carry.level)
    else {
        return true;
    };

    if let Some(tip) = lone_tip(window) {
        aim_tip(window, viewport, cursor, tip);
        return true;
    }
    if level != pointer.alt {
        retake(window, viewport, cursor, pointer.alt);
    }
    let Some(carry) = &window.tools.supports.carry else {
        return true;
    };
    let camera = &window.view.camera;
    if let Some(now) = on_drag_plane(camera, viewport, cursor, carry.from, carry.level) {
        let target = now - carry.from;
        if target != carry.aimed {
            aim(window, target);
        }
    }
    true
}

/// What is held where the cursor puts it, while a carry has it lifted: in its group's tint
/// where it may stand, in danger's where the model refuses it and it stays where it last could.
pub(in crate::panels::viewport_panel) fn carried_ghost(window: &Window) -> Option<Ghost> {
    let carry = window.tools.supports.carry.as_ref()?;
    let colour = if carry.refused.is_some() {
        theme::colors().danger
    } else {
        theme::support_tint(usize::from(carry.group))
    };
    Some(Ghost {
        shape: GhostShape::Body {
            mesh: Arc::clone(carry.shown.as_ref()?),
            transform: Transform::default(),
        },
        colour,
    })
}

/// Shows a closed hand while a carry has hold, and one that says no while what is held may not
/// stand where the cursor puts it, so the refusal does not rest on colour alone.
pub(in crate::panels::viewport_panel) fn show_carrying(ctx: &egui::Context, window: &Window) {
    let Some(carry) = &window.tools.supports.carry else {
        return;
    };
    ctx.set_cursor_icon(if carry.refused.is_some() {
        egui::CursorIcon::NotAllowed
    } else {
        egui::CursorIcon::Grabbing
    });
}

/// Starts the drag again from where what is held now stands, on the plane `level` asks for,
/// so pressing or letting go of Alt mid-drag moves nothing by itself.
fn retake(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, level: bool) {
    let Some(carry) = window.tools.supports.carry.take() else {
        return;
    };
    repick(window, &carry.before);
    let through = carry.from + carry.taken;
    let taken = carry_from(window, viewport, cursor, through, level).map(|fresh| Carry {
        shown: carry.shown.clone(),
        last: carry.last.clone(),
        ..fresh
    });
    window.tools.supports.carry = taken.or(Some(carry));
}

/// A drag starting with the cursor on the plane through `through`, plate mm, everything held
/// as it now stands; `None` when that plane is seen edge-on.
fn carry_from(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    through: Vec3,
    level: bool,
) -> Option<Carry> {
    let from = on_drag_plane(&window.view.camera, viewport, cursor, through, level)?;
    let before = held_objects(window.tools)
        .into_iter()
        .filter_map(|id| Some((id, window.doc.scene.get(id)?.supports.frozen().to_vec())))
        .collect();
    let group = window.tools.supports.picked.first().and_then(|picked| {
        let object = window.doc.scene.get(picked.id)?;
        Some(object.supports.frozen().get(picked.frozen)?.group())
    });
    Some(Carry {
        from,
        taken: Vec3::ZERO,
        aimed: Vec3::NAN,
        level,
        before,
        last: Vec::new(),
        shown: None,
        group: group.unwrap_or_default(),
        refused: None,
    })
}

/// Where the cursor now points on the drag's plane through `through`: square to the camera, so
/// what is held keeps its depth and stays under the cursor, or level with the plate when `level`.
fn on_drag_plane(
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    through: Vec3,
    level: bool,
) -> Option<Vec3> {
    let normal = if level {
        Vec3::Z
    } else {
        (camera.sight_to(through) - through).normalize_or_zero()
    };
    crossing(camera, viewport, cursor, through, normal)
}

/// Where the cursor's ray crosses the plane through `through` square to `normal`, or `None`
/// when it runs too nearly along it.
fn crossing(
    camera: &OrbitCamera,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    through: Vec3,
    normal: Vec3,
) -> Option<Vec3> {
    let ray = ray_through(camera, viewport, cursor)?;
    let facing = ray.direction.dot(normal);
    if facing.abs() < PLANE_GRAZE {
        return None;
    }
    Some(ray.at((through - ray.origin).dot(normal) / facing))
}

/// Carries what is held `target` from where the drag began: drawn there whatever the model
/// says, and put there in the scene only where it may stand.
fn aim(window: &mut Window, target: Vec3) {
    let found = window
        .tools
        .supports
        .carry
        .as_ref()
        .and_then(|carry| carried_by(&held_on(window), carry, target));
    let Some(mut carry) = window.tools.supports.carry.take() else {
        return;
    };
    match found {
        Some(found) => {
            carry.aimed = target;
            carry.refused = found.refused;
            carry.last = found.moved.clone();
            carry.shown = Some(Arc::new(found.mesh));
            if found.refused.is_none() {
                carry.taken = target;
                put_back(window, found.moved);
            } else {
                lift(window, &found.moved);
            }
        }
        // Nowhere for the column to stand: the last place drawn stays, refused.
        None => carry.refused = Some(Refused::NoFooting),
    }
    window.tools.supports.carry = Some(carry);
}

/// What a carry would put where, and why some of it may not stand there.
#[derive(Default)]
struct Found {
    moved: Vec<(ObjectId, Moved)>,
    refused: Option<Refused>,
    /// Every tree it moves, plate coordinates, meshed apart from the rest.
    mesh: Mesh,
}

impl Found {
    /// Takes in what a carry moves on `object`, meshed apart from the rest.
    fn take(
        &mut self,
        object: &SceneObject,
        carried: Carried,
        placed: &core_supports::Placed,
        profiles: Profiles,
    ) {
        let matrix = object.transform.to_matrix();
        if let Some(why) = carried.refused {
            self.refused.get_or_insert(why);
        }
        for (_, tree) in &carried.moved {
            self.mesh.append(&core_supports::mesh_alone(
                &tree.moved(matrix),
                profiles,
                placed,
            ));
        }
        self.moved.push((object.id, carried.moved));
    }
}

/// One object something is held on, with what a carry asks of it.
struct Held<'a> {
    object: &'a SceneObject,
    table: Vec<SupportProfile>,
    keep_out: Option<Blocked>,
    /// Each frozen tree held, with its parts held.
    trees: Vec<(usize, Vec<Part>)>,
}

/// Every object something is held on, each once.
fn held_on<'a>(window: &'a Window<'_>) -> Vec<Held<'a>> {
    let picked = &window.tools.supports.picked;
    held_objects(window.tools)
        .into_iter()
        .filter_map(|id| {
            let object = window.doc.scene.get(id)?;
            let mut trees: Vec<(usize, Vec<Part>)> = Vec::new();
            for held in picked.iter().filter(|held| held.id == id) {
                match trees.iter_mut().find(|(frozen, _)| *frozen == held.frozen) {
                    Some((_, parts)) => parts.push(held.part),
                    None => trees.push((held.frozen, vec![held.part])),
                }
            }
            Some(Held {
                object,
                table: window.tools.supports.table_for(object),
                keep_out: object.supports.keep_out(&object.mesh, object.transform),
                trees,
            })
        })
        .collect()
}

/// Every support held, and those standing on them, carried `offset` from how they stood when
/// `carry` began, or `None` where a column has nothing to stand on.
fn carried_by(held: &[Held<'_>], carry: &Carry, offset: Vec3) -> Option<Found> {
    let mut found = Found::default();
    for held in held {
        let id = held.object.id;
        let (_, standing) = carry.before.iter().find(|(object, _)| *object == id)?;
        let last = carry
            .last
            .iter()
            .find(|(object, _)| *object == id)
            .map_or(&[][..], |(_, last)| last);
        let profiles = Profiles::new(&held.table)?;
        let placed = held
            .object
            .support_placed()
            .blocking(held.keep_out.as_ref());
        let carried =
            core_supports::carried_held(standing, last, &held.trees, offset, &placed, profiles)?;
        found.take(held.object, carried, &placed, profiles);
    }
    Some(found)
}

/// The tip in hand, when a tip is the one thing held: it is carried to where the cursor
/// points rather than by the step the cursor took.
fn lone_tip(window: &Window) -> Option<Picked> {
    let [picked] = window.tools.supports.picked.as_slice() else {
        return None;
    };
    let Part::Node(node) = picked.part else {
        return None;
    };
    let tree = window
        .doc
        .scene
        .get(picked.id)?
        .supports
        .frozen()
        .get(picked.frozen)?;
    tree.nodes().get(node)?.is_leaf().then_some(*picked)
}

/// Puts the held tip where the cursor points on its own model, else on the nearest surface
/// at the tip's depth (ADR 0244); where neither may hold it, it stays and is shown refused.
fn aim_tip(window: &mut Window, viewport: egui::Rect, cursor: egui::Pos2, tip: Picked) {
    let targets = tip_targets(window, viewport, cursor, tip);
    // NaN where nothing is pointed at, which never matches: a refusal is cheap to ask again.
    let aimed = targets.first().copied().unwrap_or(Vec3::NAN);
    let carry = window.tools.supports.carry.as_ref();
    if carry.is_some_and(|carry| carry.aimed == aimed) {
        return;
    }
    let found = tip_carried(window, tip, &targets);
    let Some(mut carry) = window.tools.supports.carry.take() else {
        return;
    };
    carry.aimed = aimed;
    match found {
        Ok(found) => {
            carry.refused = found.refused;
            if found.refused.is_none() {
                carry.shown = Some(Arc::new(found.mesh));
                let point = held_point(window.doc, tip);
                put_back(window, found.moved);
                follow_tip(window, tip, point);
            }
        }
        Err(why) => carry.refused = Some(why),
    }
    window.tools.supports.carry = Some(carry);
}

/// Where the cursor asks the held tip to go, plate mm, first wanted first: the point of its own
/// model under the cursor, then the cursor at the tip's depth.
fn tip_targets(
    window: &Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    tip: Picked,
) -> Vec<Vec3> {
    let camera = &window.view.camera;
    let pointed = surface_under(window, viewport, cursor)
        .filter(|(id, _)| *id == tip.id)
        .map(|(_, hit)| hit.point);
    let beside = part_position(window.doc, tip).and_then(|stood| {
        let facing = (camera.sight_to(stood) - stood).normalize_or_zero();
        crossing(camera, viewport, cursor, stood, facing)
    });
    pointed.into_iter().chain(beside).collect()
}

/// The tip and what stands on its support at the first of `targets` that takes it, or why the
/// tip finds no surface to hold there.
fn tip_carried(window: &Window, tip: Picked, targets: &[Vec3]) -> Result<Found, Refused> {
    let Part::Node(node) = tip.part else {
        return Err(Refused::NoHold);
    };
    let gone = Refused::NoHold;
    let object = window.doc.scene.get(tip.id).ok_or(gone)?;
    let table = window.tools.supports.table_for(object);
    let profiles = Profiles::new(&table).ok_or(gone)?;
    let frozen = object.supports.frozen();
    let keep_out = object.supports.keep_out(&object.mesh, object.transform);
    let placed = object.support_placed().blocking(keep_out.as_ref());
    let mut why = gone;
    let mut carried = None;
    for to in targets {
        match core_supports::tip_carried(frozen, tip.frozen, node, *to, &placed, profiles) {
            Ok(tree) => {
                carried = Some(tree);
                break;
            }
            Err(refused) => why = refused,
        }
    }
    let carried = carried.ok_or(why)?;
    let carried = core_supports::with_riders(frozen, tip.frozen, &carried, &placed, profiles)
        .ok_or(Refused::NoFooting)?;
    let mut found = Found::default();
    found.take(object, carried, &placed, profiles);
    Ok(found)
}

/// The point the held tip holds, which names it whatever its place in its tree.
fn held_point(doc: &Doc, tip: Picked) -> Option<usize> {
    let Part::Node(node) = tip.part else {
        return None;
    };
    let object = doc.scene.get(tip.id)?;
    object
        .supports
        .frozen()
        .get(tip.frozen)?
        .nodes()
        .get(node)?
        .point
}

/// Names the held tip by its place in its tree as carried: a small pillar grown again from its
/// contact may be built of other nodes (ADR 0442).
fn follow_tip(window: &mut Window, tip: Picked, point: Option<usize>) {
    let Some(point) = point else {
        return;
    };
    let node = window
        .doc
        .scene
        .get(tip.id)
        .and_then(|object| object.supports.frozen().get(tip.frozen))
        .and_then(|tree| {
            tree.nodes()
                .iter()
                .position(|node| node.point == Some(point))
        });
    if let (Some(node), Some(held)) = (node, window.tools.supports.picked.first_mut()) {
        held.part = Part::Node(node);
    }
}

/// Names each part picked on a tree the carry built again by its tip: a small pillar grown
/// again from its contact is built of other nodes than `before` named (ADR 0442).
fn repick(window: &mut Window, before: &[(ObjectId, Vec<SupportTree>)]) {
    let scene = &window.doc.scene;
    for picked in &mut window.tools.supports.picked {
        let was = before
            .iter()
            .find(|(id, _)| *id == picked.id)
            .and_then(|(_, trees)| trees.get(picked.frozen));
        let now = scene
            .get(picked.id)
            .and_then(|object| object.supports.frozen().get(picked.frozen));
        let (Some(was), Some(now)) = (was, now) else {
            continue;
        };
        let kept = was.nodes().len() <= now.nodes().len()
            && (was.nodes().iter().zip(now.nodes())).all(|(a, b)| a.point == b.point);
        if !kept && let Some(tip) = now.nodes().iter().position(|node| node.is_leaf()) {
            picked.part = Part::Node(tip);
        }
    }
    let mut seen = Vec::new();
    window.tools.supports.picked.retain(|picked| {
        let first = !seen.contains(picked);
        seen.push(*picked);
        first
    });
}

/// Lifts every tree a carry moves out of its object's meshes, so the hand draws them; the
/// first time builds the rest once without them.
fn lift(window: &mut Window, moved: &[(ObjectId, Moved)]) {
    for (id, trees) in moved {
        let Some(object) = window.doc.scene.get_mut(*id) else {
            continue;
        };
        let mut lifted: Vec<usize> = trees.iter().map(|(index, _)| *index).collect();
        lifted.sort_unstable();
        lifted.dedup();
        if object.supports.lifted() != lifted.as_slice() {
            object.supports.lift(lifted);
            let table = window.tools.supports.table_for(object);
            object.refresh_supports(&table);
        }
    }
}

/// Puts each carried tree in its place among its object's frozen ones, lifted.
fn put_back(window: &mut Window, moved: Vec<(ObjectId, Moved)>) {
    lift(window, &moved);
    for (id, trees) in moved {
        if let Some(object) = window.doc.scene.get_mut(id) {
            for (frozen, tree) in trees {
                object.supports.carry_to(frozen, tree);
            }
        }
    }
}

/// Ends a carry: what was lifted is set down where it last could stand and meshed with the
/// rest, once.
fn set_down(window: &mut Window) {
    let Some(carry) = window.tools.supports.carry.take() else {
        return;
    };
    repick(window, &carry.before);
    for (id, _) in carry.before {
        if let Some(object) = window.doc.scene.get_mut(id) {
            object.supports.lift(Vec::new());
            let table = window.tools.supports.table_for(object);
            object.refresh_supports(&table);
        }
    }
}

/// Takes hold of the part under the cursor and freezes its support, with those standing on
/// it, against rebuilds; answers whether the press landed on a support. With `add` the part
/// is only toggled in the set and nothing is carried.
fn take_hold(
    window: &mut Window,
    viewport: egui::Rect,
    cursor: egui::Pos2,
    add: bool,
    level: bool,
) -> bool {
    set_down(window);
    let Some(Aimed { id, hit, at }) = grab_under_cursor(window, viewport, cursor) else {
        if !add {
            window.tools.supports.picked.clear();
        }
        return false;
    };
    if freeze_and_pick(window, id, hit, add).is_none() {
        return false;
    }

    // Measured from the point of the part the cursor took, so that point stays under it.
    if !add {
        window.tools.supports.carry = carry_from(window, viewport, cursor, at, level);
        // Drawn by the hand from the press on, so nothing blinks out before the first step.
        match lone_tip(window) {
            Some(tip) => aim_tip(window, viewport, cursor, tip),
            None => aim(window, Vec3::ZERO),
        }
    }
    true
}

/// Freezes the support `hit` names on `id`, with those standing on it, and picks the part:
/// alone, or toggled in the set with `add`. Answers what was picked.
fn freeze_and_pick(
    window: &mut Window,
    id: ObjectId,
    hit: core_supports::Grab,
    add: bool,
) -> Option<Picked> {
    let object = window.doc.scene.get(id)?;
    let table = window.tools.supports.table_for(object);
    let profiles = Profiles::new(&table)?;
    let object = window.doc.scene.get_mut(id)?;
    let transform = object.transform;
    let frozen = object
        .supports
        .freeze_with_riders(hit.tree, transform, profiles)?;
    window.doc.scene.select(Some(id));

    let picked = Picked {
        id,
        frozen,
        part: hit.part,
    };
    let held = &mut window.tools.supports.picked;
    let at = held.iter().position(|other| *other == picked);
    match (add, at) {
        (true, Some(at)) => {
            held.remove(at);
        }
        (true, None) => held.push(picked),
        (false, Some(_)) => {}
        (false, None) => {
            held.clear();
            held.push(picked);
        }
    }
    Some(picked)
}

/// Where a held part stands, in plate coordinates.
fn part_position(doc: &Doc, picked: Picked) -> Option<Vec3> {
    let object = doc.scene.get(picked.id)?;
    let matrix = object.transform.to_matrix();
    let tree = object.supports.frozen().get(picked.frozen)?;
    let node = |index: usize| tree.nodes().get(index).map(|node| node.position);

    let local = match picked.part {
        Part::Node(index) => node(index)?,
        Part::Strut(child) => {
            let parent = tree.nodes().get(child)?.parent?;
            node(child)?.lerp(node(parent)?, 0.5)
        }
        Part::Trunk => tree.root().position.lerp(tree.landing().base, 0.5),
        Part::Foot => tree.landing().base,
    };
    Some(matrix.transform_point3(local))
}

/// Every object something is held on, each once.
fn held_objects(tools: &Tools) -> Vec<ObjectId> {
    let mut held: Vec<ObjectId> = tools
        .supports
        .picked
        .iter()
        .map(|picked| picked.id)
        .collect();
    held.sort_unstable();
    held.dedup();
    held
}
