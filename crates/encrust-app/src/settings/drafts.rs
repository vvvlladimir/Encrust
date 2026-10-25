use crate::slicing::Rescaled;
use crate::undo::Frame;
use printer_profiles::{MaterialProfile, PrinterTuning};
use std::collections::VecDeque;

/// How many settled edits a form takes back.
pub(super) const FORM_DEPTH: usize = 64;

/// One profile open for editing, what of it was last written, and the edits made to it
/// since it was opened, which undo and redo walk (ADR 0268).
pub struct Draft<T> {
    pub id: String,
    pub values: T,
    pub(super) saved: T,
    /// What this build ships under the same id, when it ships anything. A copy taken out
    /// of the library by an older release stands over it and says nothing; see ADR 0196.
    pub(super) shipped: Option<T>,
    /// The values as the last recorded edit left them, what stood before each edit, oldest
    /// first, and what undo took away.
    pub(super) checkpoint: T,
    pub(super) past: VecDeque<T>,
    pub(super) future: Vec<T>,
}

impl<T: Clone + PartialEq> Draft<T> {
    pub(super) fn new(id: String, values: T) -> Self {
        Self {
            id,
            saved: values.clone(),
            checkpoint: values.clone(),
            values,
            shipped: None,
            past: VecDeque::new(),
            future: Vec::new(),
        }
    }

    /// Records an edit once the gesture that made it is over, as the plate's history does:
    /// a drag is one entry, and a field typed into one once it is left.
    pub fn observe(&mut self, frame: Frame) {
        if !frame.settled || frame.typing || self.values == self.checkpoint {
            return;
        }
        let was = std::mem::replace(&mut self.checkpoint, self.values.clone());
        self.past.push_back(was);
        if self.past.len() > FORM_DEPTH {
            self.past.pop_front();
        }
        self.future.clear();
    }

    /// Takes the form back to before its last edit, one not yet recorded first. The form
    /// writes what it then holds as it would any edit.
    pub fn undo(&mut self) -> bool {
        if self.values == self.checkpoint {
            let Some(previous) = self.past.pop_back() else {
                return false;
            };
            self.checkpoint = previous;
        }
        let undone = std::mem::replace(&mut self.values, self.checkpoint.clone());
        self.future.push(undone);
        true
    }

    /// Puts back what the last undo took away, unless the form was edited since.
    pub fn redo(&mut self) -> bool {
        if self.values != self.checkpoint {
            return false;
        }
        let Some(next) = self.future.pop() else {
            return false;
        };
        let was = std::mem::replace(&mut self.checkpoint, next.clone());
        self.past.push_back(was);
        self.values = next;
        true
    }

    /// The same, over the profile the build ships under that id.
    pub(super) fn over_shipped(id: String, values: T, shipped: Option<T>) -> Self {
        Self {
            shipped,
            ..Self::new(id, values)
        }
    }

    /// Whether the form holds something not yet written.
    pub fn is_dirty(&self) -> bool {
        self.values != self.saved
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.values.clone();
    }

    /// The profile the build ships under this id, while the user's copy differs from it.
    pub fn shipped(&self) -> Option<&T> {
        self.shipped
            .as_ref()
            .filter(|shipped| **shipped != self.saved)
    }

    /// The profile the build ships under this id, whether the user's copy differs or not.
    pub fn as_shipped(&self) -> Option<&T> {
        self.shipped.as_ref()
    }
}

/// A resin as one printer needs it, beside the resin with every printer's table.
pub struct ResinDraft {
    pub printer: String,
    pub draft: Draft<MaterialProfile>,
    pub base: MaterialProfile,
    /// The exposure a change of the measured height carried along, as it was. A profile
    /// has no bands, so the bands of it stay empty.
    pub(super) carried: Option<Rescaled>,
}

impl ResinDraft {
    pub(super) fn new(printer: String, id: String, base: MaterialProfile) -> Self {
        Self {
            draft: Draft::new(id, base.starting_point(&printer)),
            printer,
            base,
            carried: None,
        }
    }

    /// Moves the height the exposure is measured at, carrying the exposure along the
    /// working curve so that the profile still cures a layer as deep; see
    /// `docs/decisions/0128`.
    pub fn set_layer_height(&mut self, layer_height_mm: f32) {
        let values = &mut self.draft.values;
        let from_mm = values.layer_height_mm;
        if (layer_height_mm - from_mm).abs() < f32::EPSILON {
            return;
        }
        let before = self.carried.take().unwrap_or(Rescaled {
            from_mm,
            exposure_s: values.exposure_s,
            bands: Vec::new(),
        });
        if (layer_height_mm - before.from_mm).abs() < f32::EPSILON {
            values.layer_height_mm = layer_height_mm;
            values.exposure_s = before.exposure_s;
            return;
        }
        *values = values.rescaled_to(layer_height_mm);
        self.carried = Some(before);
    }

    /// What the last change of the measured height carried along, while untouched.
    pub fn carried(&self) -> Option<&Rescaled> {
        self.carried.as_ref()
    }

    /// Puts back the exposure the last change of height carried along.
    pub fn revert_exposure(&mut self) {
        if let Some(before) = self.carried.take() {
            self.draft.values.exposure_s = before.exposure_s;
        }
    }

    /// The exposure was set by hand.
    pub fn exposure_edited(&mut self) {
        self.carried = None;
    }

    /// `Draft::undo`, forgetting what a change of height carried along: the values it
    /// would put back are no longer the ones in the form.
    pub fn undo(&mut self) -> bool {
        self.carried = None;
        self.draft.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.carried = None;
        self.draft.redo()
    }

    /// The resin to write: the edits become this printer's table and every other printer
    /// keeps its own. What the resin is and what it costs belong to the resin itself.
    pub fn to_saved(&self) -> MaterialProfile {
        let edited = &self.draft.values;
        let mut saved = self.base.clone();
        saved.name = edited.name.clone();
        saved.density_g_cm3 = edited.density_g_cm3;
        saved.details = edited.details.clone();
        let tuning = PrinterTuning::of_changes(&saved, edited);
        saved.printers.insert(self.printer.clone(), tuning);
        saved.last_printer = Some(self.printer.clone());
        saved
    }
}
