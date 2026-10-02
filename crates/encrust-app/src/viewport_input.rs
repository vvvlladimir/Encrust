use core_geometry::Vec2;

/// Points the cursor may travel between press and release and still count as a click
/// rather than a drag.
const CLICK_SLOP_POINTS: f32 = 4.0;

/// What the viewport is doing with the pointer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Drag {
    #[default]
    None,
    Orbit,
    Pan,
}

/// The raw pointer state one frame of the viewport reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pointer {
    pub over_viewport: bool,
    /// Where the cursor is, in panel points.
    pub position: Option<Vec2>,
    /// A mouse button went down this frame.
    pub pressed: bool,
    pub primary_down: bool,
    /// The secondary or middle button, either of which pans.
    pub pan_button_down: bool,
    pub shift: bool,
    pub delta: Vec2,
    pub scroll: f32,
}

impl Pointer {
    /// A pointer that is doing nothing, which is what the viewport reads while something
    /// modal is over it.
    pub fn idle() -> Self {
        Self {
            over_viewport: false,
            position: None,
            pressed: false,
            primary_down: false,
            pan_button_down: false,
            shift: false,
            delta: Vec2::ZERO,
            scroll: 0.0,
        }
    }

    fn any_button_down(&self) -> bool {
        self.primary_down || self.pan_button_down
    }
}

/// What the viewport should do with this frame's pointer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gesture {
    pub drag: Drag,
    /// The press and release both landed on the viewport without moving: a selection.
    pub clicked: bool,
}

/// Which drag the viewport owns, carried across frames.
///
/// egui cannot arbitrate this for us. `transform-gizmo-egui` registers an interaction
/// widget one point across under the cursor on every frame it draws, and that widget is
/// created after the viewport's own, so it wins the pointer: from the first selected
/// model onwards the viewport's `Response` reports neither drags nor clicks. The viewport
/// therefore reads the raw pointer and decides for itself, yielding to the gizmo whenever
/// a handle is focused.
#[derive(Debug, Default)]
pub struct ViewportInput {
    drag: Drag,
    /// The press that started this drag was a left click on the viewport, and the cursor
    /// has not travelled far enough to stop being one.
    candidate_click: bool,
    travel_points: f32,
}

impl ViewportInput {
    /// Advances the gesture by one frame.
    pub fn update(&mut self, pointer: &Pointer, gizmo_focused: bool) -> Gesture {
        if !pointer.any_button_down() {
            return self.release();
        }

        if self.drag == Drag::None {
            // A drag starts only where it is pressed: on the viewport, off the gizmo, and
            // on the frame the button goes down, so a drag that began in another panel
            // never captures the camera on its way across.
            if !pointer.pressed || !pointer.over_viewport || gizmo_focused {
                return Gesture::default();
            }
            self.drag = if pointer.pan_button_down || pointer.shift {
                Drag::Pan
            } else {
                Drag::Orbit
            };
            self.candidate_click = pointer.primary_down && !pointer.shift;
            self.travel_points = 0.0;
        }

        self.travel_points += pointer.delta.length();
        if self.travel_points > CLICK_SLOP_POINTS {
            self.candidate_click = false;
        }

        Gesture {
            drag: self.drag,
            clicked: false,
        }
    }

    /// Ends the gesture, reporting a click when the pointer barely moved while it lasted.
    fn release(&mut self) -> Gesture {
        let clicked = self.candidate_click;
        self.drag = Drag::None;
        self.candidate_click = false;
        self.travel_points = 0.0;
        Gesture {
            drag: Drag::None,
            clicked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(button: Drag, shift: bool) -> Pointer {
        Pointer {
            over_viewport: true,
            position: Some(Vec2::new(10.0, 10.0)),
            pressed: true,
            primary_down: button == Drag::Orbit || shift,
            pan_button_down: button == Drag::Pan && !shift,
            shift,
            ..Pointer::default()
        }
    }

    fn hold(pointer: &Pointer, delta: Vec2) -> Pointer {
        Pointer {
            pressed: false,
            delta,
            ..*pointer
        }
    }

    fn released() -> Pointer {
        Pointer::default()
    }

    #[test]
    fn the_left_button_orbits_and_the_right_one_pans() {
        let mut input = ViewportInput::default();
        assert_eq!(
            input.update(&press(Drag::Orbit, false), false).drag,
            Drag::Orbit
        );

        let mut input = ViewportInput::default();
        assert_eq!(
            input.update(&press(Drag::Pan, false), false).drag,
            Drag::Pan
        );
    }

    #[test]
    fn shift_turns_a_left_drag_into_a_pan() {
        let mut input = ViewportInput::default();
        assert_eq!(
            input.update(&press(Drag::Orbit, true), false).drag,
            Drag::Pan
        );
    }

    #[test]
    fn a_press_on_a_gizmo_handle_never_moves_the_camera() {
        let mut input = ViewportInput::default();
        let pressed = press(Drag::Orbit, false);
        assert_eq!(input.update(&pressed, true).drag, Drag::None);

        // The handle stops being focused the moment the gizmo takes the drag, and the
        // camera must not pick it up on the next frame.
        let held = hold(&pressed, Vec2::new(20.0, 0.0));
        assert_eq!(input.update(&held, false).drag, Drag::None);
    }

    #[test]
    fn a_drag_that_started_elsewhere_does_not_capture_the_camera() {
        let mut input = ViewportInput::default();
        let outside = Pointer {
            over_viewport: false,
            pressed: true,
            primary_down: true,
            ..Pointer::default()
        };
        assert_eq!(input.update(&outside, false).drag, Drag::None);

        let entering = Pointer {
            over_viewport: true,
            ..hold(&outside, Vec2::new(5.0, 5.0))
        };
        assert_eq!(input.update(&entering, false).drag, Drag::None);
    }

    #[test]
    fn a_drag_survives_the_cursor_leaving_the_viewport() {
        let mut input = ViewportInput::default();
        let pressed = press(Drag::Orbit, false);
        input.update(&pressed, false);

        let outside = Pointer {
            over_viewport: false,
            ..hold(&pressed, Vec2::new(50.0, 0.0))
        };
        assert_eq!(input.update(&outside, false).drag, Drag::Orbit);
    }

    #[test]
    fn a_press_and_release_on_the_spot_is_a_click() {
        let mut input = ViewportInput::default();
        input.update(&press(Drag::Orbit, false), false);
        let gesture = input.update(&released(), false);
        assert!(gesture.clicked);
        assert_eq!(gesture.drag, Drag::None);
    }

    #[test]
    fn a_release_after_an_orbit_selects_nothing() {
        let mut input = ViewportInput::default();
        let pressed = press(Drag::Orbit, false);
        input.update(&pressed, false);
        input.update(&hold(&pressed, Vec2::new(30.0, 0.0)), false);
        assert!(!input.update(&released(), false).clicked);
    }

    #[test]
    fn a_pan_never_ends_in_a_selection() {
        let mut input = ViewportInput::default();
        input.update(&press(Drag::Pan, false), false);
        assert!(!input.update(&released(), false).clicked);
    }

    #[test]
    fn a_release_without_a_press_reports_nothing() {
        let mut input = ViewportInput::default();
        assert_eq!(input.update(&released(), false), Gesture::default());
    }
}
