//! The test prints the start page offers: resin tests of the project's own, built in code
//! with the mark raised on each, and pictured from the same mesh; see `docs/decisions/0232`.

mod detail;
mod exposure;
mod fit;
mod mark;
mod solid;
mod surface;

use std::sync::Arc;

use core_geometry::Mesh;
use core_thumbnail::{Part, Rgb, ThumbnailSettings, render};

use crate::files::Handed;

/// One test print, opened as if its file had been dropped on the window.
pub struct Sample {
    pub name: &'static str,
    /// What printing it tells, in two lines of a card at most.
    pub about: &'static str,
    build: fn() -> Mesh,
}

pub const SAMPLES: [Sample; 4] = [
    Sample {
        name: "Detail test",
        about: "Spikes, ledges at three angles and the mark's overhangs, all without supports.",
        build: detail::build,
    },
    Sample {
        name: "Exposure check",
        about: "Pins and fins break off underexposed, slots and holes close up overexposed.",
        build: exposure::build,
    },
    Sample {
        name: "Surface finish",
        about: "A dome and a column show the pixel steps and anti-aliasing, a ramp the layers.",
        build: surface::build,
    },
    Sample {
        name: "Fit gauge",
        about: "Pins 0.3 mm under to 0.1 mm over a 6 mm hole, shortest smallest: which go in.",
        build: fit::build,
    },
];

/// Rendered at twice the size it is shown at and halved, which smooths its edges.
const SUPERSAMPLE: usize = 2;

impl Sample {
    /// The model as an STL handed to the window.
    pub fn handed(&self) -> Handed {
        let bytes = solid::stl(&(self.build)());
        Handed::bytes(format!("{}.stl", self.name), Arc::from(bytes))
    }

    /// The model shaded in `model` on `background`, `size` pixels.
    pub fn picture(&self, size: [usize; 2], background: Rgb, model: Rgb) -> egui::ColorImage {
        let mesh = (self.build)();
        let settings = ThumbnailSettings {
            width_px: (size[0] * SUPERSAMPLE) as u32,
            height_px: (size[1] * SUPERSAMPLE) as u32,
            background,
            model,
        };
        let large = render(&[Part::placed(&mesh)], &settings);
        egui::ColorImage::from_rgb(size, &halved(large.pixels(), size))
    }
}

/// Each block of `SUPERSAMPLE` pixels square averaged into one, as RGB bytes.
fn halved(pixels: &[Rgb], size: [usize; 2]) -> Vec<u8> {
    let wide = size[0] * SUPERSAMPLE;
    let count = (SUPERSAMPLE * SUPERSAMPLE) as u32;
    let mut bytes = Vec::with_capacity(size[0] * size[1] * 3);
    for y in 0..size[1] {
        for x in 0..size[0] {
            let mut sum = [0u32; 3];
            for dy in 0..SUPERSAMPLE {
                let row = (y * SUPERSAMPLE + dy) * wide + x * SUPERSAMPLE;
                for pixel in &pixels[row..row + SUPERSAMPLE] {
                    for (total, channel) in sum.iter_mut().zip(pixel) {
                        *total += u32::from(*channel);
                    }
                }
            }
            bytes.extend(sum.map(|total| (total / count) as u8));
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_test_print_opens_sound() {
        for sample in &SAMPLES {
            let plate = crate::plate::BuildPlate::default();
            let imported = crate::import::prepare(&sample.handed(), &plate, &mut |_| {})
                .expect("a generated STL reads");
            let summary = &imported.summary;
            assert!(summary.is_sound(), "{}: {summary:?}", sample.name);
        }
    }

    #[test]
    fn every_test_print_stays_small() {
        for sample in &SAMPLES {
            let faces = (sample.build)().faces.len();
            assert!(faces < 40_000, "{} has {faces} faces", sample.name);
        }
    }

    #[test]
    fn the_fit_gauge_pins_differ_from_the_hole_by_their_clearances() {
        let mesh = fit::build();
        let [low, high] = mesh
            .vertices
            .iter()
            .filter(|v| v.y < -1.0)
            .fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], v| {
                [low.min(v.x), high.max(v.x)]
            });
        let last = fit::CLEARANCES_MM.len() - 1;
        // The first pin's left edge to the last one's right: the pitches between, both radii.
        let expected = fit::PIN_PITCH * last as f32
            + (fit::HOLE_MM + fit::CLEARANCES_MM[0]) / 2.0
            + (fit::HOLE_MM + fit::CLEARANCES_MM[last]) / 2.0;
        let span = high - low;
        assert!(
            (span - expected).abs() < 1e-3,
            "{span} mm against {expected} mm"
        );
    }
}
