use core_geometry::Vec3;

/// Where the plate is looked at from in the isometric view, plate coordinates with Z up: the front
/// right corner, a little above the part, so it shows a silhouette, a top and a side.
const EYE: Vec3 = Vec3::new(1.0, -1.0, 0.75);

/// A side the plate is looked at from, orthographic, the frame fitted to what is drawn. Plate
/// coordinates have X to the right, Y away from the front and Z up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
    /// From the front right corner, a little above: the sliced file's thumbnail.
    Iso,
    Front,
    Back,
    Left,
    Right,
    /// From above, X to the right and the front edge at the bottom.
    Top,
    /// From under the plate, X to the right and the front edge at the top: drain holes and tips.
    Bottom,
}

impl View {
    pub const ALL: [Self; 7] = [
        Self::Iso,
        Self::Front,
        Self::Back,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
    ];

    /// The view's name on a command line and in a file name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Front => "front",
            Self::Back => "back",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    /// The view called `name`, as [`View::name`] spells it, or `None`.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.name() == name)
    }

    pub(crate) fn basis(self) -> Basis {
        let axes = |right: Vec3, up: Vec3, forward: Vec3| Basis { right, up, forward };
        match self {
            Self::Iso => {
                let forward = -EYE.normalize();
                let right = forward.cross(Vec3::Z).normalize();
                axes(right, right.cross(forward), forward)
            }
            Self::Front => axes(Vec3::X, Vec3::Z, Vec3::Y),
            Self::Back => axes(-Vec3::X, Vec3::Z, -Vec3::Y),
            Self::Left => axes(-Vec3::Y, Vec3::Z, Vec3::X),
            Self::Right => axes(Vec3::Y, Vec3::Z, -Vec3::X),
            Self::Top => axes(Vec3::X, Vec3::Y, -Vec3::Z),
            Self::Bottom => axes(Vec3::X, -Vec3::Y, Vec3::Z),
        }
    }
}

/// A view's orthonormal axes in plate coordinates, `up = right × forward`; the forward one points
/// away from the eye.
pub(crate) struct Basis {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl Basis {
    /// A plate point in view coordinates: right, up and depth away from the eye.
    pub fn of(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            point.dot(self.right),
            point.dot(self.up),
            point.dot(self.forward),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_view_is_found_by_its_own_name_and_no_other() {
        for view in View::ALL {
            assert_eq!(View::named(view.name()), Some(view));
        }
        assert_eq!(View::named("underneath"), None);
    }
}
