use core_geometry::{Bvh, Mesh, Ray, Scalar, Vec2, Vec3};

use crate::join::{Halves, Planned};
use crate::section::{Frame, Section};
use crate::settings::{JointKind, Magnets, Pegs};
use crate::shapes;
use crate::spots::{Room, spread};

/// Least resin a socket keeps round it and under its floor, mm.
const WALL_MM: Scalar = 0.8;
/// Socket depth past the peg's end, mm, where spare glue goes.
const GLUE_MM: Scalar = 0.2;
/// How far a pin reaches back into its half, so the two are one solid, mm.
const SINK_MM: Scalar = 0.1;
/// How far a socket's cutter starts outside its half and a pillar runs on into the far
/// wall, mm, so neither ends on a face the boolean would have to split hairs over.
const CLEAR_MM: Scalar = 0.5;

/// An automatic peg's diameter as a share of the widest circle its piece of the face
/// holds, and the bounds it is kept within, mm; its length is half as much again.
const AUTO_SHARE: Scalar = 0.5;
const AUTO_DIAMETER_MM: (Scalar, Scalar) = (2.0, 6.0);
const AUTO_LENGTH: Scalar = 1.5;

/// The narrower peg of a keyed set, as a share of the others' width, and its least, mm.
const KEYED_SHARE: Scalar = 0.7;
const KEYED_LEAST_MM: Scalar = 0.75;

/// A peg shortened to fit keeps at least this share of its length, or the spot is refused.
const SHORTEST_SHARE: Scalar = 0.5;

/// Points round a socket's rim that are sounded, besides its centre.
const RIM_RAYS: usize = 8;

/// One peg or magnet as it was put in: where on the cut, how wide, how long, and the
/// pillars standing under it in a hollow model, one each half.
#[derive(Debug, Clone, Copy)]
struct Placed {
    at: Vec3,
    radius_mm: Scalar,
    length_mm: Scalar,
    pillars: [Option<Scalar>; 2],
}

/// The two halves' sides, into the lower one and into the upper.
const LOWER: usize = 0;
const UPPER: usize = 1;

/// A model's material along a ray from the cut, read off one of its meshes.
struct Probe<'a> {
    mesh: &'a Mesh,
    bvh: Bvh,
}

impl<'a> Probe<'a> {
    fn new(mesh: &'a Mesh) -> Self {
        Self {
            mesh,
            bvh: Bvh::build(mesh),
        }
    }

    /// Distance to the surface along `direction` from `at` and from points round it at
    /// `radius_mm`: the least of them, or the most.
    fn sound(&self, at: Vec3, direction: Vec3, radius_mm: Scalar, most: bool) -> Scalar {
        let side = direction.any_orthonormal_vector();
        let up = direction.cross(side);
        let rim = (0..RIM_RAYS).map(|step| {
            let angle = std::f32::consts::TAU * step as Scalar / RIM_RAYS as Scalar;
            at + (side * angle.cos() + up * angle.sin()) * radius_mm
        });
        let depths = std::iter::once(at).chain(rim).map(|from| {
            self.bvh
                .raycast(self.mesh, &Ray::new(from, direction))
                .map_or(0.0, |hit| hit.t)
        });
        if most {
            depths.fold(0.0, Scalar::max)
        } else {
            depths.fold(Scalar::MAX, Scalar::min)
        }
    }
}

/// One kind of fixing, as far as placing it goes: how wide its socket is and how deep each
/// half needs to be for a peg of some length.
#[derive(Debug, Clone, Copy)]
struct Fixing {
    kind: JointKind,
    radius_mm: Scalar,
    length_mm: Scalar,
    clearance_mm: Scalar,
}

impl Fixing {
    fn socket_radius(&self) -> Scalar {
        self.radius_mm + self.clearance_mm
    }

    fn room(&self) -> Room {
        let reach_mm = self.socket_radius() + WALL_MM;
        Room {
            reach_mm,
            gap_mm: 2.0 * reach_mm + CLEAR_MM,
        }
    }

    /// How deep the socket in one half is for a peg `length_mm` long, or how far the pin
    /// reaches into the lower half.
    fn socket_depth(&self, side: usize, length_mm: Scalar) -> Scalar {
        match (self.kind, side) {
            (JointKind::Pins, LOWER) => SINK_MM,
            (JointKind::Pins, _) => length_mm + self.clearance_mm + GLUE_MM,
            (JointKind::Dowels, _) => length_mm / 2.0 + self.clearance_mm + GLUE_MM,
            _ => length_mm + self.clearance_mm,
        }
    }

    /// The longest peg the material `depth` into each half takes, or `None` where it is
    /// too short to hold.
    fn length_for(&self, depth: [Scalar; 2]) -> Option<Scalar> {
        let room = |side: usize| depth[side] - WALL_MM - self.socket_depth(side, 0.0);
        let length = match self.kind {
            JointKind::Pins => room(UPPER).min(self.length_mm),
            JointKind::Dowels => (2.0 * room(LOWER).min(room(UPPER))).min(self.length_mm),
            _ => {
                return (room(LOWER).min(room(UPPER)) >= self.length_mm).then_some(self.length_mm);
            }
        };
        let seated = depth[LOWER] >= SINK_MM + WALL_MM;
        (seated && length >= SHORTEST_SHARE * self.length_mm).then_some(length)
    }
}

/// Pegs or magnets for every piece of the cut face, with the sockets they go into.
pub(crate) fn peg(halves: &Halves<'_>, kind: JointKind, pegs: Pegs, magnets: Magnets) -> Planned {
    let face = halves
        .solid
        .map(|solid| core_geometry::cut(solid, halves.plane).below);
    let face = face.as_ref().unwrap_or(&halves.cut.below);
    let Some(section) = Section::of(face, halves.plane, Scalar::MAX) else {
        return Planned::default();
    };
    let printed = halves
        .solid
        .and_then(|_| Section::of(&halves.cut.below, halves.plane, Scalar::MAX));
    let material = Probe::new(halves.solid.unwrap_or(halves.printed));
    let shell = halves.solid.map(|_| Probe::new(halves.printed));
    let context = Context {
        frame: section.frame,
        material,
        shell: shell.zip(printed),
    };

    let mut placed = Vec::new();
    let mut bare = 0;
    for cells in section.cells_by_piece() {
        let (count, fixing) = fixing_for(&section, &cells, kind, pegs, magnets);
        let spots = spread(&section, &cells, count, fixing.room(), |point| {
            context.fit(point, &fixing)
        });
        bare += usize::from(spots.is_empty());
        placed.extend(spots.into_iter().map(|(_, placed)| placed));
    }
    if pegs.keyed && matches!(kind, JointKind::Pins | JointKind::Dowels) && placed.len() > 1 {
        placed[0].radius_mm = (placed[0].radius_mm * KEYED_SHARE).max(KEYED_LEAST_MM);
    }
    built(
        &placed,
        kind,
        &context.frame,
        clearance(kind, pegs, magnets),
        bare,
    )
}

fn clearance(kind: JointKind, pegs: Pegs, magnets: Magnets) -> Scalar {
    match kind {
        JointKind::Magnets => magnets.clearance_mm,
        _ => pegs.clearance_mm,
    }
}

/// How many fixings one piece of the face takes, and their size: the magnet's, the pegs'
/// as set, or the pegs' as the width of the piece suggests.
fn fixing_for(
    section: &Section,
    cells: &[usize],
    kind: JointKind,
    pegs: Pegs,
    magnets: Magnets,
) -> (usize, Fixing) {
    if kind == JointKind::Magnets {
        let fixing = Fixing {
            kind,
            radius_mm: magnets.diameter_mm / 2.0,
            length_mm: magnets.thickness_mm,
            clearance_mm: magnets.clearance_mm,
        };
        return (magnets.per_piece as usize, fixing);
    }
    let (diameter_mm, length_mm) = if pegs.auto_size {
        let widest = cells
            .iter()
            .map(|cell| section.distance[*cell])
            .fold(0.0, Scalar::max);
        let (least, most) = AUTO_DIAMETER_MM;
        let diameter = ((AUTO_SHARE * widest * 2.0).floor() / 2.0).clamp(least, most);
        (diameter, AUTO_LENGTH * diameter)
    } else {
        (pegs.diameter_mm, pegs.length_mm)
    };
    let fixing = Fixing {
        kind,
        radius_mm: diameter_mm / 2.0,
        length_mm,
        clearance_mm: pegs.clearance_mm,
    };
    (pegs.per_piece as usize, fixing)
}

/// What a spot is sounded against: the model's material, and in a hollow model its shell
/// with the face the shell was cut to, to tell where a pillar has to carry the socket.
struct Context<'a> {
    frame: Frame,
    material: Probe<'a>,
    shell: Option<(Probe<'a>, Section)>,
}

impl Context<'_> {
    /// The peg a spot takes, sized to the material either side of it, or `None`.
    fn fit(&self, point: Vec2, fixing: &Fixing) -> Option<Placed> {
        let at = self.frame.lift(point);
        let into = [-self.frame.normal, self.frame.normal];
        let rim = fixing.socket_radius();
        let depth = into.map(|direction| self.material.sound(at, direction, rim, false));
        let length_mm = fixing.length_for(depth)?;
        let pillars = [LOWER, UPPER].map(|side| {
            let need = fixing.socket_depth(side, length_mm) + WALL_MM;
            self.pillar(point, at, into[side], rim, need)
        });
        Some(Placed {
            at,
            radius_mm: fixing.radius_mm,
            length_mm,
            pillars,
        })
    }

    /// How long a pillar has to be under a socket at `at` for the shell to hold it, or
    /// `None` where the shell is already thick enough there, or the model solid.
    fn pillar(
        &self,
        point: Vec2,
        at: Vec3,
        direction: Vec3,
        rim: Scalar,
        need: Scalar,
    ) -> Option<Scalar> {
        let (shell, face) = self.shell.as_ref()?;
        let wide = face.distance_at(point) >= rim + WALL_MM;
        if wide && shell.sound(at, direction, rim, false) >= need {
            return None;
        }
        Some(self.material.sound(at, direction, rim + WALL_MM, true) + CLEAR_MM)
    }
}

/// The pegs, sockets, pillars and loose dowels for what was placed.
fn built(
    placed: &[Placed],
    kind: JointKind,
    frame: &Frame,
    clearance_mm: Scalar,
    bare: usize,
) -> Planned {
    let mut pegged = Planned {
        placed: placed.len(),
        bare,
        ..Planned::default()
    };
    let into = [-frame.normal, frame.normal];
    for peg in placed {
        let socket_radius = peg.radius_mm + clearance_mm;
        for (side, work) in [(LOWER, &mut pegged.below), (UPPER, &mut pegged.above)] {
            if let Some(length) = peg.pillars[side] {
                let radius = socket_radius + WALL_MM;
                work.pillars
                    .append(&shapes::bar(peg.at, into[side], radius, length));
            }
            if kind == JointKind::Pins && side == LOWER {
                let pin = shapes::pin(peg.at, frame.normal, peg.radius_mm, peg.length_mm, SINK_MM);
                work.pegs.append(&pin);
                continue;
            }
            let fixing = Fixing {
                kind,
                radius_mm: peg.radius_mm,
                length_mm: peg.length_mm,
                clearance_mm,
            };
            let depth = fixing.socket_depth(side, peg.length_mm);
            let socket = shapes::socket(peg.at, into[side], socket_radius, depth, CLEAR_MM);
            work.sockets.append(&socket);
        }
        if kind == JointKind::Dowels {
            pegged
                .loose
                .push(shapes::dowel(peg.radius_mm, peg.length_mm));
        }
    }
    pegged
}
