use core_geometry::{Scalar, Vec2};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::error::TextError;
use crate::font::Font;

/// How far a flattened curve may stray from the font's own, in millimetres: a tenth of
/// the finest pixel a resin printer has.
const CURVE_TOLERANCE_MM: Scalar = 0.002;

/// The most straight pieces one curve of a glyph is cut into, whatever its size.
const MOST_PIECES: usize = 64;

/// One filled region of the text: an outer ring anticlockwise and the holes in it
/// clockwise, in millimetres on the label's plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub outer: Vec<Vec2>,
    pub holes: Vec<Vec<Vec2>>,
}

/// `text` written in `font` with capitals `height_mm` tall, one line per `\n`, each line
/// centred and the whole block centred on the origin.
pub fn outline(font: &Font, text: &str, height_mm: Scalar) -> Result<Vec<Shape>, TextError> {
    if !(height_mm.is_finite() && height_mm > 0.0) {
        return Err(TextError::BadSize {
            what: "letter height",
            value_mm: height_mm,
        });
    }
    let face = font.face()?;
    let capitals = face
        .capital_height()
        .filter(|units| *units > 0)
        .unwrap_or(face.ascender());
    let scale = height_mm / Scalar::from(capitals.max(1));
    let line_mm = Scalar::from(face.ascender() - face.descender() + face.line_gap()) * scale;

    let mut rings = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let baseline = -(row as Scalar) * line_mm;
        let start = rings.len();
        let width = write_line(&face, line, scale, baseline, &mut rings)?;
        for ring in &mut rings[start..] {
            for point in ring.iter_mut() {
                point.x -= width / 2.0;
            }
        }
    }
    let shapes = nested(&rings);
    if shapes.is_empty() {
        return Err(TextError::Blank);
    }
    Ok(centred(shapes))
}

/// Outlines one line from x = 0 into `rings`, and answers how far it advanced.
fn write_line(
    face: &Face<'_>,
    line: &str,
    scale: Scalar,
    baseline: Scalar,
    rings: &mut Vec<Vec<Vec2>>,
) -> Result<Scalar, TextError> {
    let mut pen = 0.0;
    let mut previous: Option<GlyphId> = None;
    for character in line.chars() {
        let glyph = face
            .glyph_index(character)
            .ok_or(TextError::MissingGlyph(character))?;
        if let Some(before) = previous {
            pen += kerning(face, before, glyph) * scale;
        }
        let mut pen_down = Pen {
            origin: Vec2::new(pen, baseline),
            scale,
            tolerance: CURVE_TOLERANCE_MM / scale,
            ring: Vec::new(),
            rings: Vec::new(),
        };
        face.outline_glyph(glyph, &mut pen_down);
        pen_down.close();
        rings.extend(pen_down.rings);
        pen += Scalar::from(face.glyph_hor_advance(glyph).unwrap_or(0)) * scale;
        previous = Some(glyph);
    }
    Ok(pen)
}

/// The horizontal kerning between two glyphs from the font's `kern` table, in font units.
fn kerning(face: &Face<'_>, left: GlyphId, right: GlyphId) -> Scalar {
    face.tables()
        .kern
        .into_iter()
        .flat_map(|table| table.subtables)
        .filter(|table| table.horizontal && !table.variable)
        .find_map(|table| table.glyphs_kerning(left, right))
        .map_or(0.0, Scalar::from)
}

/// Collects a glyph's contours as rings in millimetres, flattening its curves.
struct Pen {
    origin: Vec2,
    scale: Scalar,
    /// The tolerance in font units.
    tolerance: Scalar,
    ring: Vec<Vec2>,
    rings: Vec<Vec<Vec2>>,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> Vec2 {
        self.origin + Vec2::new(x, y) * self.scale
    }

    fn last(&self) -> Vec2 {
        self.ring.last().copied().unwrap_or(self.origin)
    }

    /// Pieces a curve with control polygon `length` long is cut into: the sag of a piece
    /// falls with the square of how many there are.
    fn pieces(&self, length_mm: Scalar) -> usize {
        let units = length_mm / self.scale;
        ((units / self.tolerance).sqrt().ceil() as usize).clamp(1, MOST_PIECES)
    }
}

impl OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.ring.push(self.at(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.ring.push(self.at(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (from, control, to) = (self.last(), self.at(x1, y1), self.at(x, y));
        let pieces = self.pieces(from.distance(control) + control.distance(to));
        for step in 1..=pieces {
            let t = step as Scalar / pieces as Scalar;
            let a = from.lerp(control, t);
            self.ring.push(a.lerp(control.lerp(to, t), t));
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let from = self.last();
        let (one, two, to) = (self.at(x1, y1), self.at(x2, y2), self.at(x, y));
        let length = from.distance(one) + one.distance(two) + two.distance(to);
        let pieces = self.pieces(length);
        for step in 1..=pieces {
            let along = step as Scalar / pieces as Scalar;
            let near = from.lerp(one, along).lerp(one.lerp(two, along), along);
            let far = one.lerp(two, along).lerp(two.lerp(to, along), along);
            self.ring.push(near.lerp(far, along));
        }
    }

    fn close(&mut self) {
        let mut ring = std::mem::take(&mut self.ring);
        ring.dedup_by(|a, b| a.distance_squared(*b) < Scalar::EPSILON);
        while ring.len() > 1 && ring[0].distance_squared(ring[ring.len() - 1]) < Scalar::EPSILON {
            ring.pop();
        }
        if ring.len() >= 3 && area(&ring).abs() > Scalar::EPSILON {
            self.rings.push(ring);
        }
    }
}

/// Twice the signed area of a ring, positive anticlockwise.
fn area(ring: &[Vec2]) -> Scalar {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.perp_dot(*b))
        .sum()
}

fn contains(ring: &[Vec2], point: Vec2) -> bool {
    let mut inside = false;
    for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Rings grouped into shapes by how deep each lies in the others, whichever way the font
/// winds them: a ring inside an even number of others is filled, inside an odd number it is
/// a hole in the smallest ring around it.
fn nested(rings: &[Vec<Vec2>]) -> Vec<Shape> {
    let parents: Vec<Vec<usize>> = rings
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            (0..rings.len())
                .filter(|other| *other != index && contains(&rings[*other], ring[0]))
                .collect()
        })
        .collect();
    let mut shapes: Vec<(usize, Shape)> = Vec::new();
    for (index, ring) in rings.iter().enumerate() {
        if parents[index].len().is_multiple_of(2) {
            shapes.push((
                index,
                Shape {
                    outer: wound(ring.clone(), true),
                    holes: Vec::new(),
                },
            ));
        }
    }
    for (index, ring) in rings.iter().enumerate() {
        if !parents[index].len().is_multiple_of(2) {
            let around = parents[index]
                .iter()
                .filter(|parent| parents[**parent].len() + 1 == parents[index].len())
                .min_by(|a, b| area(&rings[**a]).abs().total_cmp(&area(&rings[**b]).abs()));
            if let Some(shape) =
                around.and_then(|parent| shapes.iter_mut().find(|(at, _)| at == parent))
            {
                shape.1.holes.push(wound(ring.clone(), false));
            }
        }
    }
    shapes.into_iter().map(|(_, shape)| shape).collect()
}

fn wound(mut ring: Vec<Vec2>, anticlockwise: bool) -> Vec<Vec2> {
    if (area(&ring) > 0.0) != anticlockwise {
        ring.reverse();
    }
    ring
}

/// The shapes moved so the middle of the box around them is the origin.
fn centred(mut shapes: Vec<Shape>) -> Vec<Shape> {
    let points = shapes.iter().flat_map(|shape| shape.outer.iter());
    let (low, high) = points.fold(
        (Vec2::splat(Scalar::MAX), Vec2::splat(Scalar::MIN)),
        |(low, high), point| (low.min(*point), high.max(*point)),
    );
    let middle = (low + high) / 2.0;
    for shape in &mut shapes {
        for point in shape
            .outer
            .iter_mut()
            .chain(shape.holes.iter_mut().flatten())
        {
            *point -= middle;
        }
    }
    shapes
}
