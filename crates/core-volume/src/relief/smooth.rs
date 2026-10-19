use core_geometry::{Heightmap, Scalar};
use rayon::prelude::*;

/// Box blurs stacked to stand in for one Gaussian; three are within a few percent of it.
const PASSES: usize = 3;

/// `image` blurred by about a Gaussian of `sigma_px`, wrapping round its edges the way the
/// image tiles, so a seamless texture stays seamless.
pub fn smoothed(image: &Heightmap, sigma_px: Scalar) -> Heightmap {
    let radius = box_radius(sigma_px);
    let mut out = image.clone();
    if radius == 0 {
        return out;
    }
    let (width, height) = (image.width(), image.height());
    let mut samples = image.samples().to_vec();
    for _ in 0..PASSES {
        let across = blurred_rows(&samples, width, radius);
        let down = blurred_rows(&transposed(&across, width, height), height, radius);
        samples = transposed(&down, height, width);
    }
    out.samples_mut().copy_from_slice(&samples);
    out
}

/// The half-width of the box whose three passes have the variance of `sigma_px`: each pass
/// adds `r(r + 1) / 3`.
fn box_radius(sigma_px: Scalar) -> usize {
    let sigma = sigma_px.max(0.0);
    (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) / 2.0).round() as usize
}

fn blurred_rows(samples: &[Scalar], width: usize, radius: usize) -> Vec<Scalar> {
    let mut out = vec![0.0; samples.len()];
    out.par_chunks_mut(width)
        .zip(samples.par_chunks(width))
        .for_each(|(out, row)| slide(row, out, radius));
    out
}

/// A running mean of `radius` either side along one cyclic row.
fn slide(row: &[Scalar], out: &mut [Scalar], radius: usize) {
    let len = row.len().cast_signed();
    let at = |index: isize| row[index.rem_euclid(len) as usize];
    // A window wider than the row only counts it again, and costs a sum per pixel.
    let reach = radius.min(row.len()).cast_signed();
    let scale = 1.0 / (2 * reach + 1) as Scalar;
    let mut sum: Scalar = (-reach..=reach).map(at).sum();
    for (index, value) in out.iter_mut().enumerate() {
        *value = sum * scale;
        let index = index.cast_signed();
        sum += at(index + reach + 1) - at(index - reach);
    }
}

/// A `width` by `height` image turned to `height` by `width`.
fn transposed(samples: &[Scalar], width: usize, height: usize) -> Vec<Scalar> {
    let mut out = vec![0.0; samples.len()];
    out.par_chunks_mut(height)
        .enumerate()
        .for_each(|(column, out)| {
            for (row, value) in out.iter_mut().enumerate() {
                *value = samples[row * width + column];
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize, samples: Vec<Scalar>) -> Heightmap {
        Heightmap::new(width, height, samples).expect("one sample per pixel")
    }

    fn mean(image: &Heightmap) -> Scalar {
        image.samples().iter().sum::<Scalar>() / image.samples().len() as Scalar
    }

    #[test]
    fn no_smoothing_leaves_the_image_as_it_was() {
        let spike = image(3, 1, vec![0.0, 1.0, 0.0]);
        assert_eq!(smoothed(&spike, 0.0), spike);
    }

    #[test]
    fn smoothing_spreads_a_spike_and_keeps_its_mean() {
        let mut samples = vec![0.0; 64];
        samples[3 * 8 + 3] = 1.0;
        let spike = image(8, 8, samples);
        let soft = smoothed(&spike, 1.5);
        // A wrapping blur moves height around the tile and never adds or removes any.
        assert!(
            (mean(&soft) - mean(&spike)).abs() < 1e-5,
            "mean went from {} to {}",
            mean(&spike),
            mean(&soft)
        );
        let peak = soft.samples().iter().copied().fold(0.0, Scalar::max);
        assert!(peak < 0.5, "the spike stood at {peak} after smoothing");
    }

    #[test]
    fn smoothing_wraps_round_the_edges_of_the_tile() {
        // A stripe down the left column has its other neighbour in the right column.
        let mut samples = vec![0.0; 16];
        for row in 0..4 {
            samples[row * 4] = 1.0;
        }
        let soft = smoothed(&image(4, 4, samples), 1.0);
        let row = &soft.samples()[..4];
        assert!(
            (row[1] - row[3]).abs() < 1e-5,
            "both neighbours of the stripe take the same share, got {row:?}"
        );
    }
}
