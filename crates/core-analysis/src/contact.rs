use core_raster::{LayerRuns, PixelPitch};

use crate::layer::{Cured, Moments, cure};

/// A piece smaller than this is a speck of edge grey, not an island worth naming, mm^2.
pub const MIN_ISLAND_MM2: f32 = 0.01;

/// The pixels a piece shares with one piece of the layer under it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Contact {
    pub above: u32,
    pub below: u32,
    /// The shared pixels' area and its moments, for where the contact is and how it spreads.
    pub moments: Moments,
}

/// Every pair of pieces that share a pixel between `above` and the layer `below` it,
/// sorted by the piece above. One walk down both layers' spans.
pub(crate) fn contacts(above: &Cured, below: &Cured) -> Vec<Contact> {
    let (upper, lower) = (above.spans(), below.spans());
    let pitch = above.pitch();
    // The pair each piece below last touched and where that went, so a piece resting on
    // one other adds to one entry rather than to one a row; the sort then has little to do.
    let mut last = vec![(u32::MAX, 0usize); below.pieces().len()];
    let mut found: Vec<Contact> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < upper.len() && j < lower.len() {
        let (a, b) = (upper[i], lower[j]);
        if (a.row, a.x1) <= (b.row, b.x0) {
            i += 1;
            continue;
        }
        if (b.row, b.x1) <= (a.row, a.x0) {
            j += 1;
            continue;
        }
        let (from, to) = (a.x0.max(b.x0), a.x1.min(b.x1));
        let mut run = Contact {
            above: a.piece,
            below: b.piece,
            moments: Moments::default(),
        };
        run.moments.add(a.row, from..to, 1.0, pitch);
        let (touched, at) = &mut last[b.piece as usize];
        if *touched == a.piece {
            found[*at].moments.merge(&run.moments);
        } else {
            (*touched, *at) = (a.piece, found.len());
            found.push(run);
        }
        if a.x1 <= b.x1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    merged(found)
}

fn merged(mut found: Vec<Contact>) -> Vec<Contact> {
    found.sort_unstable_by_key(|contact| (contact.above, contact.below));
    let mut merged: Vec<Contact> = Vec::with_capacity(found.len());
    for contact in found {
        match merged.last_mut() {
            Some(last) if (last.above, last.below) == (contact.above, contact.below) => {
                last.moments.merge(&contact.moments);
            }
            _ => merged.push(contact),
        }
    }
    merged
}

/// Every pair of pieces, above then below, sharing a pixel between `layer` and the layer `below`:
/// what following a part up a stack joins.
pub fn standing_on(layer: &Cured, below: &Cured) -> Vec<(u32, u32)> {
    contacts(layer, below)
        .into_iter()
        .map(|contact| (contact.above, contact.below))
        .collect()
}

/// Which pieces of `layer` stand on nothing in the layer `below`, one flag a piece.
pub fn islands(layer: &Cured, below: &Cured) -> Vec<bool> {
    let mut held = vec![false; layer.pieces().len()];
    for contact in contacts(layer, below) {
        held[contact.above as usize] = true;
    }
    layer
        .pieces()
        .iter()
        .zip(held)
        .map(|(piece, held)| !held && piece.area_mm2 >= MIN_ISLAND_MM2)
        .collect()
}

/// The pixels of `layer` that stand on nothing in `below`, lit, for a picture to paint.
pub fn island_runs(layer: &LayerRuns, below: &LayerRuns, pitch: PixelPitch) -> LayerRuns {
    let layer = cure(layer, pitch);
    let flags = islands(&layer, &cure(below, pitch));
    layer.runs_of(&flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::tests::{UNIT, paint};

    #[test]
    fn a_piece_resting_on_the_layer_below_touches_it_where_they_overlap() {
        let above = cure(&paint(6, 1, |x, _| u8::from(x >= 2) * 255), UNIT);
        let below = cure(&paint(6, 1, |x, _| u8::from(x < 4) * 255), UNIT);
        let found = contacts(&above, &below);
        assert_eq!(found.len(), 1);
        let moments = found[0].moments;
        assert!(
            (moments.area_mm2() - 2.0).abs() < 1e-9,
            "columns 2 and 3 are shared"
        );
        assert!(
            (moments.centre_mm()[0] - 3.0).abs() < 1e-9,
            "centred on x = 3"
        );
    }

    #[test]
    fn a_piece_on_two_pieces_stands_on_both() {
        let above = cure(&paint(9, 1, |_, _| 255), UNIT);
        let below = cure(
            &paint(9, 1, |x, _| u8::from(!(3..=5).contains(&x)) * 255),
            UNIT,
        );
        assert_eq!(standing_on(&above, &below), vec![(0, 0), (0, 1)]);
    }

    #[test]
    fn a_piece_over_nothing_is_an_island_and_one_over_something_is_not() {
        let above = cure(
            &paint(10, 3, |x, _| u8::from(!(3..6).contains(&x)) * 255),
            UNIT,
        );
        let below = cure(&paint(10, 3, |x, _| u8::from(x < 2) * 255), UNIT);
        assert_eq!(islands(&above, &below), vec![false, true]);
    }

    #[test]
    fn a_speck_of_grey_is_not_an_island() {
        let pitch = PixelPitch { x: 0.02, y: 0.02 };
        let above = cure(&paint(4, 4, |x, y| u8::from((x, y) == (2, 2)) * 255), pitch);
        let below = cure(&paint(4, 4, |_, _| 0), pitch);
        assert_eq!(islands(&above, &below), vec![false]);
    }

    #[test]
    fn island_pixels_are_painted_and_the_rest_left_dark() {
        let above = paint(8, 1, |x, _| u8::from(!(2..5).contains(&x)) * 255);
        let below = paint(8, 1, |x, _| u8::from(x < 1) * 255);
        let lit: Vec<u8> = island_runs(&above, &below, UNIT)
            .to_mask()
            .pixels()
            .to_vec();
        assert_eq!(lit, vec![0, 0, 0, 0, 0, 255, 255, 255]);
    }
}
