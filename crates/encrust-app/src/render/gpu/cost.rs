//! What a frame of the viewport costs, counted from what `prepare` recorded.

use web_time::Instant;

use crate::render::frame_time::FrameCost;

use super::*;

impl ViewportResources {
    /// Writes the frame whose preparing began at `started` to the frame log, with the
    /// `ui_ms` the window's own pass took before it.
    pub fn log_frame(&mut self, started: Instant, ui_ms: f32) {
        let now = Instant::now();
        let (drawn_faces, draws) = self.sent();
        let cost = FrameCost {
            ui_ms,
            prepare_ms: now.duration_since(started).as_secs_f32() * 1000.0,
            drawn_faces,
            draws,
            uploaded_faces: self.uploaded_faces,
            shadow_baked: self.shadow_baked,
            card_ms: self.card_ms,
        };
        self.frame_log.record(now, cost);
    }

    /// Triangles sent to the card this frame and the mesh draws that sent them, every pass
    /// counted: the models once, the solids once a cap, a cut's body three times and its
    /// object once (ADR 0201), and the models again when the shadow was baked.
    fn sent(&self) -> (usize, u32) {
        let caps = usize::from(self.frame.cutting) + usize::from(self.frame.cutting_bottom);
        let shadow = usize::from(self.shadow_baked);
        let drawn = self.scene_faces() + shadow * faces(&self.frame.models);
        let draws = self.frame.models.len() * (1 + shadow)
            + self.frame.solids.len() * caps
            + 4 * self.frame.cuts.len()
            + self.frame.reliefs.len();
        (drawn, draws as u32)
    }

    /// Triangles the scene pass draws, the shadow's bake left out.
    pub(super) fn scene_faces(&self) -> usize {
        let caps = usize::from(self.frame.cutting) + usize::from(self.frame.cutting_bottom);
        let cuts: usize = self
            .frame
            .cuts
            .iter()
            .map(|(body, solid)| 3 * body.faces.len() + solid.faces.len())
            .sum();
        faces(&self.frame.models) + faces(&self.frame.solids) * caps + cuts
    }
}

fn faces(draws: &[Drawn]) -> usize {
    draws.iter().map(|draw| draw.faces.len()).sum()
}
