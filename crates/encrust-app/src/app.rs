use std::path::Path;

use core_geometry::Vec2;

use crate::drain::DrainTool;
use crate::files::{Arrived, Handed, Wanted};
use crate::i18n::tr;
use crate::panels::Window;
use crate::prefs::Preferences;
use crate::preview::stack_fingerprint;
use crate::project;
use crate::render;
use crate::scene::SceneObject;
use crate::shortcuts;
use crate::state::{Doc, Machine, Tools, View};
use crate::status::Status;
use crate::supports::with_own;
use crate::tool_settings::ToolSettings;
use crate::ui::theme;
use crate::undo::Frame;
use crate::workspace::{Mode, Tool};

/// Window state: what the user is doing, the scene being edited, and the camera looking
/// at it. The layout that draws it is fixed; see `crate::panels`.
pub struct SlicerApp {
    mode: Mode,
    /// The mode the last frame was drawn in, so a switch is noticed wherever it came from.
    drawn_mode: Mode,
    tool: Tool,
    doc: Doc,
    view: View,
    tools: Tools,
    machine: Machine,
    /// What the last run was set to, so a change to it is noticed and written once.
    prefs: Preferences,
    /// The keys this frame asked for, read off the raw input before egui saw it.
    keys: Vec<shortcuts::Action>,
}

impl Default for SlicerApp {
    fn default() -> Self {
        let doc = Doc::default();
        Self {
            mode: Mode::default(),
            drawn_mode: Mode::default(),
            tool: Tool::default(),
            view: View::framing(&doc.plate),
            doc,
            tools: Tools::default(),
            machine: Machine::default(),
            prefs: Preferences::default(),
            keys: Vec::new(),
        }
    }
}

impl SlicerApp {
    /// Builds the window, puts the viewport's GPU resources where egui keeps them, dresses
    /// the context in the Encrust tokens, and imports the model named on the command line.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_model: Option<&Path>) -> Self {
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            render::install(render_state);
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::hold_context(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        crate::updates::sweep();

        let prefs = crate::prefs::load();
        crate::i18n::choose(prefs.language());
        let mut app = Self {
            prefs,
            ..Self::default()
        };
        let renamed = app
            .machine
            .slicing
            .load_user_profiles(&mut app.machine.status);
        if !renamed.is_empty() {
            app.prefs.follow(&renamed);
            app.prefs.save();
        }
        let prefs = app.prefs.clone();
        prefs.apply(&mut app.window());
        app.tools
            .supports
            .start_automatic_unless_chosen(&app.doc.scene, &app.machine.slicing.catalogue);
        prefs.restore_panels(&cc.egui_ctx);
        project::mark_saved(&mut app.window());
        if let Some(path) = initial_model {
            open_by_what_it_is(&mut app.window(), Handed::Path(path.to_path_buf()));
        }
        app
    }

    /// Opens a model, read from a mesh or built back from a sliced file.
    fn open_model(&mut self, file: Handed) {
        open_by_what_it_is(&mut self.window(), file);
    }

    /// The machine, the resin, every tool value, the Preview divide and the side cards' widths
    /// are written the frame they change, so the next run opens on them however this one ends.
    /// Only on a settled frame: a slider under the pointer is one choice, not one per frame.
    fn remember_choices(&mut self, settled: bool, ctx: &egui::Context) {
        if !settled {
            return;
        }
        let now = Preferences::of(
            &self.machine.slicing,
            &self.machine.network,
            &self.machine.updates.prefs,
            &self.tools,
        )
        .with_recent(&self.machine.recent)
        .with_layout(&self.view.options)
        .with_general(&self.machine.general)
        .with_panels(self.view.gizmo.axes, ctx);
        if now != self.prefs {
            self.prefs = now;
            self.prefs.save();
        }
    }

    /// A file dropped on the window opens the way the dialog would open it: a mesh goes on
    /// to the plate, and a sliced file goes under the layer slider. A browser drops bytes,
    /// several at once for a model and its textures.
    fn take_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
        #[cfg(not(target_arch = "wasm32"))]
        for file in dropped {
            self.open_dropped(Handed::Path(file.path().to_path_buf()));
        }
        // A browser reads a dropped file by a promise, so it arrives on a later frame.
        #[cfg(target_arch = "wasm32")]
        crate::web::files::read_dropped(dropped);
    }

    /// A dropped file is a profile while Machine and resin is open over the plate, and
    /// opens as it would from the dialog otherwise.
    fn open_dropped(&mut self, file: Handed) {
        match self.machine.settings.machines {
            true => crate::panels::import_dropped(&mut self.window(), &file),
            false => open_by_what_it_is(&mut self.window(), file),
        }
    }

    /// What a browser's dialogs handed over since the last frame, each where it was asked
    /// for.
    fn take_arrived_files(&mut self) {
        for arrived in crate::files::arrived() {
            match arrived {
                Arrived::File(Wanted::Model, file) => self.open_model(file),
                Arrived::File(Wanted::ModelOrProject, file) => {
                    open_by_what_it_is(&mut self.window(), file);
                }
                Arrived::File(Wanted::Project, file) => {
                    project::open(&mut self.window(), &file);
                }
                Arrived::File(Wanted::PrinterProfile, file) => {
                    crate::profiles::import_printer(&mut self.window(), &file);
                }
                Arrived::File(Wanted::ResinProfile, file) => {
                    crate::profiles::import_material(&mut self.machine, &file);
                }
                Arrived::File(Wanted::Profile(kind), file) => {
                    crate::panels::import_profile(&mut self.window(), kind, &file);
                }
                Arrived::File(Wanted::ReliefPicture, file) => crate::relief::load_picture(
                    &mut self.tools.relief,
                    &mut self.machine.status,
                    &file,
                ),
                Arrived::Dropped(file) => self.open_dropped(file),
                Arrived::Failed(message) => self.machine.status = Status::Error(message),
            }
        }
    }

    /// A stack written for a printer goes straight on to it; one the user named is
    /// already where they asked for it, or in a browser, already handed over as a download.
    fn hand_written_file_to_the_network(&mut self) {
        let Some(written) = self.machine.slicing.take_written() else {
            return;
        };
        if written.to_send {
            self.machine
                .network
                .send(written.path, &mut self.machine.status);
        }
    }

    /// The preview shows the plate as it is, so entering the mode with a stale stack, or
    /// none, cuts one; so does opening Check, which reads its figures off the stack. Nothing
    /// to cut, or nothing to cut it for, and it stays empty: the transport says which.
    fn refresh_preview(&mut self) {
        let reads_the_stack = self.mode == Mode::Preview || self.tool == Tool::Check;
        if !reads_the_stack || self.machine.preview.is_building() {
            return;
        }
        // A stack cut before the supports land would be cut without them.
        let plate = self.doc.scene.active_plate();
        if self.doc.scene.supports_pending(plate) {
            return;
        }
        let fingerprint = stack_fingerprint(&self.doc.scene, self.machine.slicing.cutting());
        let wanted =
            self.machine.preview.layer_count() == 0 || self.machine.preview.is_stale(fingerprint);
        let Some(settings) = self.machine.slicing.raster_settings() else {
            return;
        };
        if !wanted {
            // Preview says what the print takes, so the stack is measured as it is written.
            self.machine
                .preview
                .measure(&settings, self.machine.slicing.fold());
            return;
        }
        if !self.doc.scene.has_printable(self.doc.scene.active_plate()) {
            return;
        }

        let started = self
            .machine
            .preview
            .build(&self.doc.scene, self.machine.slicing.cutting());
        if let Err(error) = started {
            self.machine.status = Status::failed(&error.context(tr!("app-cannot-build-preview")));
        }
    }

    /// Both modes cut the model at one height, so a switch hands it over: Preview parks on
    /// the layer nearest where Prepare was cut, and Prepare takes the layer Preview showed.
    fn carry_height(&mut self) {
        if self.mode == self.drawn_mode {
            return;
        }
        match self.mode {
            Mode::Preview => self
                .machine
                .preview
                .show_height(self.view.section.height_mm),
            Mode::Prepare => {
                if let Some(height_mm) = self.machine.preview.prepare_height() {
                    self.view.section.height_mm = height_mm;
                }
            }
        }
        self.drawn_mode = self.mode;
    }

    /// Does what the keys of this frame asked for. They were read off the raw input
    /// before egui's pass, in `shortcuts::take`.
    fn take_shortcuts(&mut self) {
        let fired = std::mem::take(&mut self.keys);
        if fired.is_empty() {
            return;
        }
        let mut window = self.window();
        for action in fired {
            shortcuts::act(&mut window, action);
        }
    }

    /// One frame of the prepare mode's transport: the cut climbing the model while the
    /// layer strip is playing. Answers whether it has to keep going.
    fn run_the_cut_up(&mut self, ctx: &egui::Context) -> bool {
        if self.mode != Mode::Prepare {
            self.view.section.playing = false;
            return false;
        }
        crate::panels::section::animate_section(
            ctx,
            &mut self.view.section,
            &self.doc,
            self.machine.slicing.layer_height_mm(),
        )
    }

    /// Every part of the window, borrowed at once, which is what a panel is drawn with.
    fn window(&mut self) -> Window<'_> {
        Window {
            mode: &mut self.mode,
            tool: &mut self.tool,
            doc: &mut self.doc,
            view: &mut self.view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        }
    }

    /// Takes the pockets of the last drainage check off a plate that has been hollowed
    /// again: the cavity they stood in is a new mesh, and nothing is checked until the
    /// Drain panel is asked to. See ADR 0198.
    /// Cuts the cavity again into a model a relief was pressed into, so a press keeps its
    /// hollow. Says whether a run was started.
    fn hollow_what_a_press_opened(&mut self) -> bool {
        let unhollowed = &mut self.tools.relief.unhollowed;
        if unhollowed.is_empty() || !self.tools.hollow.again(&self.doc.scene, unhollowed) {
            return false;
        }
        unhollowed.clear();
        self.tools.hollow.job.is_some()
    }

    fn forget_what_an_old_check_found(&mut self) {
        if !self.tools.hollow.take_hollowed() {
            return;
        }
        for object in self.doc.scene.objects_mut() {
            object.traps.clear();
        }
        self.tools.drain.stale();
    }

    /// Ends the frame's edits and puts them on the history. What follows from an edit by
    /// itself is done first, so it is part of that edit: a model settling onto its raft, and
    /// the plugs for its holes started, which `land_plugs` lays into the same edit.
    fn record(&mut self, frame: Frame, ctx: &egui::Context) {
        self.tools.supports.settle_on_raft(&mut self.doc.scene);
        let drift_mm = self.machine.slicing.material.compensation.fit_drift_mm();
        let asked_by = self.doc.history.next_edit();
        self.tools
            .drain
            .lay_plugs(&mut self.doc.scene, drift_mm, frame.settled, asked_by);
        self.doc.drilling.start(&self.doc.scene, frame.settled);
        self.doc.history.observe(
            &self.doc.scene,
            &ToolSettings::of(&self.tools, &self.machine.slicing),
            self.machine.slicing.chosen(),
            frame,
        );
        self.machine.settings.observe(frame);
        self.remember_choices(frame.settled && !frame.typing, ctx);
    }

    /// Lays the plugs a worker has cut into the edit that asked for them. Returns whether
    /// any is still being cut.
    fn land_plugs(&mut self) -> bool {
        let status = &mut self.machine.status;
        let plate_mm = Vec2::new(self.doc.plate.x_mm, self.doc.plate.y_mm);
        for ended in self.tools.drain.poll_plugs() {
            self.doc
                .history
                .land(&mut self.doc.scene, ended.asked_by(), |scene| {
                    DrainTool::land_plugs(scene, &ended, plate_mm, status);
                });
        }
        self.tools.drain.laying_plugs()
    }

    /// Rebuilds the columns and the painted patches of every object whose placement,
    /// points, paint or profile moved under them, so that what the viewport draws is what
    /// would be sliced. A profile still being dragged is grown to once it is let go.
    fn refresh_supports(&mut self, held: bool) {
        self.tools.supports.settle_on_raft(&mut self.doc.scene);
        let plate = self.tools.supports.table();
        let raft = self.tools.supports.raft;
        self.doc.surfaces.start(&mut self.doc.scene);
        for object in self.doc.scene.objects_mut() {
            let untouched = object.supports.is_empty()
                && object.supports.meshes().is_none()
                && object.supports.drawn().is_empty()
                && object.supports.painted().is_empty()
                && object.supports.blocked().is_empty();
            if untouched {
                continue;
            }
            let table = with_own(plate.clone(), &object.own, raft);
            let retuned = held && object.supports.retuned(object.transform, &table);
            if object.supports.outgrown(object.transform, &table) {
                if !retuned {
                    self.doc.growth.start(object, &table);
                }
                continue;
            }
            object.refresh_supports(&table);
        }
    }
}

/// Opens a file as whatever it turns out to be: a project onto the plate, a sliced file
/// to look at, or a model. What a drop and the one Open button both go through.
pub fn open_by_what_it_is(window: &mut Window, file: Handed) {
    if crate::files::has_extension(file.path(), &crate::files::PROJECTS) {
        project::open(window, &file);
    } else if core_pipeline::reads_sliced_file(file.path()) {
        use anyhow::Context as _;
        let slicing = &window.machine.slicing;
        let placement =
            crate::sliced::placement(&file, &slicing.catalogue, slicing.printer.as_ref())
                .with_context(|| {
                    tr!(
                        "error-cannot-open",
                        path = file.path().display().to_string()
                    )
                });
        let status = &mut window.machine.status;
        if let Some(placement) = status.report("", placement) {
            window.doc.imports.open_sliced(file, placement, status);
        }
    } else {
        window
            .doc
            .imports
            .open(file, &window.doc.plate, &mut window.machine.status);
    }
}

impl SlicerApp {
    /// Takes the files dropped on the window or handed to it, and notes those opened.
    fn take_files(&mut self, ctx: &egui::Context) {
        self.take_dropped_files(ctx);
        self.take_arrived_files();
        for (path, parts) in self.doc.imports.take_opened() {
            self.machine.recent.note(&path, crate::updates::now_s());
            crate::recent::picture::keep(&self.machine.recent, &path, parts);
        }
    }

    /// Drains every background job into the window, and answers whether one is still
    /// running: a channel wakes nothing, so the window keeps asking for frames meanwhile.
    fn poll_jobs(&mut self, ctx: &egui::Context) -> bool {
        let (doc, machine, tools) = (&mut self.doc, &mut self.machine, &mut self.tools);
        let (scene, status) = (&mut doc.scene, &mut machine.status);
        let mut working = doc
            .imports
            .poll(scene, &doc.plate, &mut self.view.camera, status);
        working |= doc.repairs.poll(scene, status);
        working |= doc.drilling.poll(scene, status);
        working |= doc.surfaces.poll(scene, status);
        working |= doc.growth.poll(scene, status);
        working |= machine.slicing.poll(scene, status);
        working |= machine.network.poll(status);
        working |= machine.preview.poll(status);
        working |= machine.updates.poll(status);
        working |= tools.supports.poll(scene, status);
        working |= tools.hollow.poll(scene, status);
        working |= tools.relief.poll(scene, status);
        working |= tools.boolean.poll(scene, status);
        working |= tools.text.poll(scene, status);
        working |= tools.cut.poll(scene, status);
        working |= tools.drain.poll(scene, status);
        working |= self.land_plugs();
        let (scene, status) = (&mut self.doc.scene, &mut self.machine.status);
        working |= self.tools.orient.poll(scene, &self.doc.plate, status);
        working | crate::panels::animate_preview(ctx, &mut self.machine.preview)
    }

    /// Drops what a tool kept for its own panel once another is in hand, and makes way
    /// for a slice.
    fn let_go_of_other_tools(&mut self) {
        if self.tool != Tool::Relief {
            self.tools.relief.forget_kept();
        }
        if self.tool != Tool::Drain {
            self.tools.drain.forget_read_model();
        }
        if self.tool != Tool::Cut {
            self.tools.cut.picking_part = false;
        }
        if self.machine.slicing.job.is_some() || self.machine.preview.is_slicing() {
            self.tools.hollow.make_way();
        }
    }

    /// Records this frame's edit, and asks for the frames a build started by it needs.
    fn finish_frame(&mut self, ctx: &egui::Context) {
        // Trees still growing are built once they land, which nothing else asks for while
        // the pointer is held still.
        let growing = self
            .doc
            .scene
            .objects()
            .iter()
            .any(SceneObject::supports_pending);
        // A gesture is one edit, so nothing is recorded until the button is up again and
        // the field being typed into has been left.
        let frame = Frame {
            settled: ctx.input(|input| !input.pointer.any_down()),
            typing: ctx.memory(|memory| memory.focused().is_some()),
        };
        self.record(frame, ctx);
        if growing || self.tools.drain.laying_plugs() || self.doc.drilling.is_running() {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for SlicerApp {
    /// The window reads its own keys here, before egui does: see `shortcuts::take`.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.keys.extend(shortcuts::take(ctx, raw_input));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = web_time::Instant::now();
        crate::i18n::choose(self.machine.general.language);
        crate::ui::theme::motion::reduce(ui.ctx(), self.machine.general.reduce_motion);
        #[cfg(target_os = "macos")]
        if !ui.input(|input| input.viewport().fullscreen.unwrap_or(false)) {
            crate::traffic_lights::centre(frame, crate::ui::theme::TOP_BAR_H);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = frame;
        self.take_files(ui.ctx());
        let working = self.poll_jobs(ui.ctx()) | self.hollow_what_a_press_opened();
        self.forget_what_an_old_check_found();
        self.let_go_of_other_tools();
        if working | self.run_the_cut_up(ui.ctx()) {
            ui.ctx().request_repaint();
        } else if self.machine.network.is_watching() {
            // A watched printer reports every few seconds; a second is soon enough to show it.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        self.machine.updates.tick();
        if self.machine.updates.take_close() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.hand_written_file_to_the_network();
        self.refresh_preview();
        self.carry_height();
        self.refresh_supports(ui.ctx().input(|input| input.pointer.any_down()));
        self.take_shortcuts();

        project::guard_close(ui, &mut self.window());
        self.window().show(ui);
        self.finish_frame(ui.ctx());
        self.view.ui_ms = started.elapsed().as_secs_f32() * 1000.0;
    }

    fn on_exit(&mut self) {
        self.machine.updates.on_exit();
        // Dropping the window resets its view's class under AppKit's Touch Bar observers,
        // which aborts the process: https://github.com/AccessKit/accesskit/issues/823
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

#[cfg(test)]
mod small_window;
#[cfg(all(test, feature = "snapshots"))]
mod snapshots;
#[cfg(test)]
mod states;

#[cfg(test)]
mod tests;
