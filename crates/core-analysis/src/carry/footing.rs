//! A support standing on the model: the part under its foot, found as the stack climbs
//! past it, and how that part gives there (ADR 0455).

use super::frame::{Frame, Stiffness, own};
use super::read::Hold;
use super::{REACH_MM, Tip, TipLoads};

impl TipLoads<'_> {
    /// Finds the part under each foot the stack has just climbed past, on the layer below.
    pub(super) fn seat_feet(&mut self, z_mm: f32) {
        let Some((below, under)) = self.below.as_ref() else {
            return;
        };
        while let Some(&tip) = self.by_foot.get(self.footed) {
            let Some(footing) = self.tips[tip as usize].stands_on else {
                self.footed += 1;
                continue;
            };
            if footing.at_mm[2] >= z_mm {
                break;
            }
            self.footed += 1;
            let [x, y, _] = footing.at_mm;
            self.feet[tip as usize] = below
                .piece_near([x, y], REACH_MM)
                .map(|piece| under[piece as usize]);
        }
    }

    /// What each of `part`'s tips stands on. A foot on `part` itself, or on a part with nothing
    /// to stand on, holds nothing; one on a part held by springs gives as that part does there,
    /// read on its own tips' springs alone.
    pub(super) fn holds(&mut self, part: u32, frames: &mut Vec<(u32, Option<Frame>)>) -> Vec<Hold> {
        let tips = self.parts[part as usize].tips.clone();
        tips.iter()
            .map(|&tip| {
                let Some(found) = self.feet[tip as usize] else {
                    return Hold::Still;
                };
                let under = self.root(found);
                if under == part {
                    return Hold::Loose;
                }
                let on = &self.parts[under as usize];
                if on.on_plate {
                    return Hold::Still;
                }
                let Some(footing) = self.tips[tip as usize].stands_on else {
                    return Hold::Still;
                };
                let frame = self.frame_of(under, frames);
                match frame.map(|frame| frame.compliance_at(footing.at_mm.map(f64::from))) {
                    Some(Some(compliance)) => Hold::On {
                        part: under,
                        compliance,
                    },
                    Some(None) => Hold::Loose,
                    None => Hold::Still,
                }
            })
            .collect()
    }

    /// `part` on its tips' own springs, kept in `frames` for the layer; `None` where a tip of
    /// it has no spring, so it is read as holding still.
    fn frame_of<'f>(
        &self,
        part: u32,
        frames: &'f mut Vec<(u32, Option<Frame>)>,
    ) -> Option<&'f Frame> {
        if !frames.iter().any(|(kept, _)| *kept == part) {
            let held: Option<Vec<(&Tip, Stiffness)>> = self.parts[part as usize]
                .tips
                .iter()
                .map(|tip| {
                    let tip = &self.tips[*tip as usize];
                    Some((tip, own(tip)?))
                })
                .collect();
            let frame = held
                .filter(|held| !held.is_empty())
                .map(|held| Frame::of(&held));
            frames.push((part, frame));
        }
        frames
            .iter()
            .find(|(kept, _)| *kept == part)
            .and_then(|(_, frame)| frame.as_ref())
    }
}
