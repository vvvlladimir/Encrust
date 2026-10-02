use core_geometry::Scalar;

use crate::SliceSettings;

/// Where every layer of a stack starts and stops, lowest first.
///
/// One plan covers a uniform stack and an adaptive one: `bounds[i]` to `bounds[i + 1]` is
/// layer `i`, in plate millimetres. A layer is sampled at the middle of its own band,
/// which is what keeps the top and bottom of a model whose faces sit exactly on a
/// boundary; see `docs/design/slicing.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerPlan {
    bounds: Vec<Scalar>,
    ceiling: Scalar,
}

impl LayerPlan {
    /// A plan from the boundaries between layers, lowest first, and the height the model
    /// itself stops at. Fewer than two boundaries is an empty stack.
    ///
    /// The last boundary may stand above the ceiling: the plate travels a whole layer
    /// whether or not there is material all the way up, and only the sampling stops
    /// inside the mesh.
    pub fn from_bounds(bounds: Vec<Scalar>, ceiling: Scalar) -> Self {
        if bounds.len() < 2 {
            return Self {
                bounds: Vec::new(),
                ceiling,
            };
        }
        Self { bounds, ceiling }
    }

    /// Layers of one thickness covering `[z_min, z_max]`.
    pub fn uniform(settings: &SliceSettings, z_min: Scalar, z_max: Scalar) -> Self {
        let count = settings.layer_count(z_min, z_max);
        let bounds = (0..=count)
            .map(|layer| (layer as Scalar).mul_add(settings.layer_height, z_min))
            .collect();
        Self::from_bounds(bounds, z_max)
    }

    /// `count` layers of one thickness, standing on the plate.
    pub fn of_count(height_mm: Scalar, count: usize) -> Self {
        let bounds: Vec<Scalar> = (0..=count)
            .map(|layer| layer as Scalar * height_mm)
            .collect();
        let ceiling = bounds.last().copied().unwrap_or(0.0);
        Self::from_bounds(bounds, ceiling)
    }

    pub fn layer_count(&self) -> usize {
        self.bounds.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.layer_count() == 0
    }

    /// Top of layer `index`, plate millimetres: the height the plate stands at while it
    /// is exposed.
    pub fn top_of(&self, index: usize) -> Option<Scalar> {
        self.bounds.get(index + 1).copied()
    }

    /// Thickness of layer `index`, millimetres.
    pub fn thickness_of(&self, index: usize) -> Option<Scalar> {
        Some(self.bounds.get(index + 1)? - self.bounds.get(index)?)
    }

    /// The thickest layer in the plan, which is what a file header states.
    pub fn nominal_thickness(&self) -> Scalar {
        (0..self.layer_count())
            .filter_map(|index| self.thickness_of(index))
            .fold(0.0, Scalar::max)
    }

    /// Whether every layer is the same thickness, so the header alone describes the stack.
    pub fn is_uniform(&self) -> bool {
        let Some(first) = self.thickness_of(0) else {
            return true;
        };
        // A tenth of a micron: below the Z resolution of any MSLA machine, and well above
        // the drift of accumulating a few thousand f32 boundaries.
        (0..self.layer_count())
            .filter_map(|index| self.thickness_of(index))
            .all(|thickness| (thickness - first).abs() < 1e-4)
    }

    /// The band of material layer `index` covers, clipped to the top of the mesh so a
    /// partial layer is sampled inside the material rather than above it.
    pub fn band_of(&self, index: usize) -> Option<(Scalar, Scalar)> {
        let bottom = *self.bounds.get(index)?;
        let top = self.bounds.get(index + 1)?.min(self.ceiling);
        Some((bottom, top))
    }

    /// Z of every sampling plane: the middle of each layer's band of material. A band
    /// reaching past the top of the mesh is sampled inside the material, not above it.
    pub fn planes(&self) -> Vec<Scalar> {
        self.bounds
            .windows(2)
            .map(|band| Scalar::midpoint(band[0], band[1].min(self.ceiling)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uniform_plan_samples_the_middle_of_every_band() {
        let plan = LayerPlan::uniform(
            &SliceSettings {
                layer_height: 1.0,
                ..SliceSettings::default()
            },
            0.0,
            10.0,
        );

        assert_eq!(plan.layer_count(), 10);
        assert_eq!(plan.planes().len(), 10);
        assert!((plan.planes()[0] - 0.5).abs() < 1e-6);
        assert!((plan.top_of(0).expect("a first layer") - 1.0).abs() < 1e-6);
        assert!((plan.thickness_of(9).expect("a last layer") - 1.0).abs() < 1e-6);
        assert!(plan.is_uniform());
    }

    #[test]
    fn a_partial_top_layer_travels_in_full_and_is_sampled_inside_the_material() {
        let plan = LayerPlan::uniform(
            &SliceSettings {
                layer_height: 1.0,
                ..SliceSettings::default()
            },
            0.0,
            10.4,
        );

        assert_eq!(plan.layer_count(), 11);
        assert!(
            (plan.thickness_of(10).expect("a last layer") - 1.0).abs() < 1e-6,
            "the plate travels a whole layer whether or not there is material up there"
        );
        assert!((plan.planes()[10] - 10.2).abs() < 1e-6);
        assert!(plan.is_uniform());
    }

    #[test]
    fn a_plan_that_covers_nothing_has_no_layers() {
        let plan = LayerPlan::uniform(
            &SliceSettings {
                layer_height: 1.0,
                ..SliceSettings::default()
            },
            3.0,
            3.0,
        );
        assert!(plan.is_empty());
        assert_eq!(plan.top_of(0), None);
        assert_eq!(plan.thickness_of(0), None);
        assert!(plan.planes().is_empty());

        assert!(LayerPlan::from_bounds(vec![1.0], 1.0).is_empty());
    }

    #[test]
    fn a_plan_of_a_count_stands_on_the_plate() {
        let plan = LayerPlan::of_count(0.05, 20);
        assert_eq!(plan.layer_count(), 20);
        assert!((plan.top_of(0).expect("a first layer") - 0.05).abs() < 1e-6);
        assert!((plan.top_of(19).expect("a last layer") - 1.0).abs() < 1e-6);
        assert!(plan.is_uniform());
        assert!(LayerPlan::of_count(0.05, 0).is_empty());
    }

    #[test]
    fn a_plan_of_its_own_bounds_reports_each_layers_own_thickness() {
        let plan = LayerPlan::from_bounds(vec![0.0, 0.1, 0.4, 0.5], 0.5);

        assert_eq!(plan.layer_count(), 3);
        assert!((plan.thickness_of(1).expect("a middle layer") - 0.3).abs() < 1e-6);
        assert!((plan.nominal_thickness() - 0.3).abs() < 1e-6);
        assert!(!plan.is_uniform());
        assert!((plan.planes()[1] - 0.25).abs() < 1e-6);
    }
}
