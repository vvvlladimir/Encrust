use core_geometry::{Scalar, Vec2, ring_area, ring_contains};
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

    let mut shapes = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let baseline = -(row as Scalar) * line_mm;
        let start = shapes.len();
        let width = write_line(&face, line, scale, baseline, &mut shapes)?;
        for shape in &mut shapes[start..] {
            for point in shape
                .outer
                .iter_mut()
                .chain(shape.holes.iter_mut().flatten())
            {
                point.x -= width / 2.0;
            }
        }
    }
    if shapes.is_empty() {
        return Err(TextError::Blank);
    }
    Ok(centred(shapes))
}

/// Outlines one line from x = 0 into `shapes`, and answers how far it advanced. Each glyph
/// is nested on its own, so letters kerned into each other stay separate shapes.
fn write_line(
    face: &Face<'_>,
    line: &str,
    scale: Scalar,
    baseline: Scalar,
    shapes: &mut Vec<Shape>,
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
        shapes.extend(nested(&pen_down.rings));
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
    ring_area(ring.iter().copied())
}

/// One glyph's rings grouped into shapes, whichever way the font winds them: a ring wound
/// against the smallest ring around it is a hole in that ring, and any other is filled, so
/// overlapping contours of one glyph stay filled as the nonzero rule fills them.
fn nested(rings: &[Vec<Vec2>]) -> Vec<Shape> {
    let parent: Vec<Option<usize>> = rings
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            (0..rings.len())
                .filter(|other| {
                    *other != index && ring_contains(rings[*other].iter().copied(), ring[0])
                })
                .min_by(|a, b| area(&rings[*a]).abs().total_cmp(&area(&rings[*b]).abs()))
        })
        .collect();
    let hole: Vec<bool> = (0..rings.len())
        .map(|index| {
            parent[index]
                .is_some_and(|around| (area(&rings[around]) > 0.0) != (area(&rings[index]) > 0.0))
        })
        .collect();
    let mut shapes: Vec<(usize, Shape)> = Vec::new();
    for (index, ring) in rings.iter().enumerate() {
        if !hole[index] {
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
        if hole[index]
            && let Some(shape) =
                parent[index].and_then(|around| shapes.iter_mut().find(|(at, _)| *at == around))
        {
            shape.1.holes.push(wound(ring.clone(), false));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn square(low: Scalar, high: Scalar, anticlockwise: bool) -> Vec<Vec2> {
        let ring = vec![
            Vec2::new(low, low),
            Vec2::new(high, low),
            Vec2::new(high, high),
            Vec2::new(low, high),
        ];
        wound(ring, anticlockwise)
    }

    #[test]
    fn a_ring_wound_against_the_one_around_it_is_a_hole() {
        let shapes = nested(&[square(0.0, 10.0, false), square(2.0, 8.0, true)]);
        assert_eq!(shapes.len(), 1, "an O is one shape");
        assert_eq!(shapes[0].holes.len(), 1, "with its counter as a hole");
    }

    #[test]
    fn overlapping_contours_wound_alike_both_stay_filled() {
        let shapes = nested(&[square(0.0, 10.0, false), square(5.0, 15.0, false)]);
        assert_eq!(shapes.len(), 2, "the nonzero rule fills both");
        assert!(shapes.iter().all(|shape| shape.holes.is_empty()));
    }
}
