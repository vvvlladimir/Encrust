//! What a panel's tests draw a window over.

use super::Window;
use crate::state::{Doc, Machine, Tools, View};
use crate::workspace::{Mode, Tool};

/// What a window is drawn over, owned by the test.
pub(crate) struct Parts {
    pub mode: Mode,
    pub tool: Tool,
    pub doc: Doc,
    pub view: View,
    pub tools: Tools,
    pub machine: Machine,
}

impl Parts {
    /// The parts of `doc`, with `tool` picked and every setting at its default.
    pub(crate) fn of(doc: Doc, tool: Tool) -> Self {
        Self {
            mode: Mode::default(),
            tool,
            view: View::framing(&doc.plate),
            doc,
            tools: Tools::default(),
            machine: Machine::default(),
        }
    }

    pub(crate) fn window(&mut self) -> Window<'_> {
        Window {
            mode: &mut self.mode,
            tool: &mut self.tool,
            doc: &mut self.doc,
            view: &mut self.view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        }
    }
}
