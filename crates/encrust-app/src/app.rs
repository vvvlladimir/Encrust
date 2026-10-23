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
    fn remember_choices(&mut self, settled: bool) {
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
        .with_general(&self.machine.general);
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
    /// none, cuts one; so does opening Check or Export, which read their figures off the
    /// stack. Nothing to cut, or nothing to cut it for, and it stays empty: the transport
    /// says which.
    fn refresh_preview(&mut self) {
        let reads_the_stack =
            self.mode == Mode::Preview || matches!(self.tool, Tool::Check | Tool::Export);
        if !reads_the_stack || self.machine.preview.is_building() {
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
    fn record(&mut self, frame: Frame) {
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
        self.remember_choices(frame.settled && !frame.typing);
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
    /// would be sliced.
    fn refresh_supports(&mut self) {
        self.tools.supports.settle_on_raft(&mut self.doc.scene);
        let plate = self.tools.supports.table();
        let raft = self.tools.supports.raft;
        for object in self.doc.scene.objects_mut() {
            if object.keep_support_surface() {
                object.supports.surface_changed();
            }
            let untouched = object.supports.is_empty()
                && object.supports.meshes().is_none()
                && object.supports.drawn().is_empty()
                && object.supports.painted().is_empty()
                && object.supports.blocked().is_empty();
            if untouched {
                continue;
            }
            let model = std::sync::Arc::clone(&object.mesh);
            let (surface, bvh) = object.support_surface();
            let (surface, bvh) = (std::sync::Arc::clone(surface), std::sync::Arc::clone(bvh));
            let table = with_own(plate.clone(), &object.own, raft);
            object
                .supports
                .refresh_on(&model, &surface, &bvh, object.transform, &table);
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

impl eframe::App for SlicerApp {
    /// The window reads its own keys here, before egui does: see `shortcuts::take`.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.keys.extend(shortcuts::take(ctx, raw_input));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = web_time::Instant::now();
        crate::i18n::choose(self.machine.general.language);
        #[cfg(target_os = "macos")]
        if !ui.input(|input| input.viewport().fullscreen.unwrap_or(false)) {
            crate::traffic_lights::centre(frame, crate::ui::theme::TOP_BAR_H);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = frame;
        self.take_dropped_files(ui.ctx());
        self.take_arrived_files();
        for (path, parts) in self.doc.imports.take_opened() {
            self.machine.recent.note(&path, crate::updates::now_s());
            crate::recent::picture::keep(&self.machine.recent, &path, parts);
        }

        // A background job reports over a channel, which wakes nothing on its own, so the
        // window has to keep asking for frames while one is running.
        let working = self.doc.imports.poll(
            &mut self.doc.scene,
            &self.doc.plate,
            &mut self.view.camera,
            &mut self.machine.status,
        ) | self
            .doc
            .repairs
            .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .doc
                .drilling
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .machine
                .slicing
                .poll(&self.doc.scene, &mut self.machine.status)
            | self.machine.network.poll(&mut self.machine.status)
            | self.machine.preview.poll(&mut self.machine.status)
            | self.machine.updates.poll(&mut self.machine.status)
            | self
                .tools
                .supports
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .hollow
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .relief
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .boolean
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .text
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .cut
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .drain
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self.land_plugs()
            | self.tools.orient.poll(
                &mut self.doc.scene,
                &self.doc.plate,
                &mut self.machine.status,
            )
            | crate::panels::animate_preview(ui.ctx(), &mut self.machine.preview);
        let working = working | self.hollow_what_a_press_opened();
        self.forget_what_an_old_check_found();
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
        self.refresh_supports();
        self.take_shortcuts();

        project::guard_close(ui, &mut self.window());
        self.window().show(ui);
        // An edit made in this frame is built in the next one, which nothing else asks
        // for while the pointer is held still.
        let stale = |object: &crate::scene::SceneObject| {
            object.supports.meshes().is_none()
                && !(object.supports.is_empty() && object.supports.drawn().is_empty())
        };
        if self.doc.scene.objects().iter().any(stale) {
            ui.ctx().request_repaint();
        }

        // A gesture is one edit, so nothing is recorded until the button is up again and
        // the field being typed into has been left.
        let frame = Frame {
            settled: ui.ctx().input(|input| !input.pointer.any_down()),
            typing: ui.ctx().memory(|memory| memory.focused().is_some()),
        };
        self.record(frame);
        // A cut started by this frame's edit is polled on the frames that follow.
        if self.tools.drain.laying_plugs() || self.doc.drilling.is_running() {
            ui.ctx().request_repaint();
        }
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
mod tests {
    use super::*;

    #[test]
    fn the_window_opens_on_the_prepare_mode_with_nothing_selected() {
        let app = SlicerApp::default();
        assert_eq!(app.mode, Mode::Prepare);
        assert_eq!(app.tool, Tool::Position);
        assert!(
            app.view.options.grid,
            "the plate grid is on until it is turned off"
        );
    }

    /// BUG: the gizmo of a picked model took every press on the stage, the layer strip's
    /// included, because it listened wherever the stage did.
    #[test]
    fn the_layer_strip_takes_a_press_while_a_model_is_picked() {
        use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
        use egui_kittest::kittest::Queryable as _;

        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(0.0, 10.0, 0.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut app = SlicerApp::default();
        let id = app.doc.scene.insert(crate::scene::Imported::new(
            "tetra".to_owned(),
            std::sync::Arc::new(mesh.clone()),
            Transform::default(),
            crate::scene::ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        app.doc.scene.select(Some(id));

        // The faces a context is given reach it a frame later, so the first frame only
        // dresses it.
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        harness.get_by_label("Model and layer side by side").click();
        harness.run_steps(3);

        assert_eq!(harness.state().mode, Mode::Preview);
    }

    /// A screen reader finds every tool on the rail by its name, and Enter on the one in
    /// focus picks it, which is the whole of driving the rail without a pointer.
    #[test]
    fn every_tool_on_the_rail_is_named_and_picked_from_the_keyboard() {
        use egui_kittest::kittest::Queryable as _;

        let mut app = SlicerApp::default();
        app.view.options.started = true;
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        for tool in Tool::on_rail() {
            let button = egui::accesskit::Role::Button;
            harness.get_by_role_and_label(button, &tool.label()).focus();
            harness.run_steps(2);
            harness.key_press(egui::Key::Enter);
            harness.run_steps(2);
            assert_eq!(harness.state().tool, tool, "Enter on {}", tool.label());
        }
    }

    #[test]
    fn taking_every_support_off_a_rafted_model_is_undone_in_one_step() {
        let mut app = super::states::app(true);
        app.tools.supports.raft.enabled = true;
        let resting = Frame {
            settled: true,
            typing: false,
        };
        app.record(resting);
        let object = &mut app.doc.scene.objects_mut()[0];
        object.transform.translation.z = 10.0;
        let transform = object.transform;
        object
            .supports
            .add(core_geometry::Vec3::new(5.0, 5.0, 10.0), transform, 0);
        app.record(resting);
        let stood_mm = app.doc.scene.objects()[0].transform.translation.z;
        assert!(stood_mm > 10.0, "the raft stood it up");

        app.doc.scene.objects_mut()[0].supports.clear();
        app.record(resting);
        app.record(resting);
        assert!(
            app.doc
                .history
                .undo(&mut app.doc.scene, &mut app.tools, &mut app.machine.slicing)
        );
        app.record(resting);
        app.record(resting);

        let object = &app.doc.scene.objects()[0];
        assert_eq!(object.supports.point_count(), 1, "the support is back");
        assert_eq!(object.transform.translation.z, stood_mm, "on its raft");
        assert!(
            app.doc.history.can_redo(),
            "and nothing was recorded over it"
        );
    }

    #[test]
    fn the_opening_camera_looks_at_the_plate() {
        let app = SlicerApp::default();
        assert_eq!(app.view.camera.target, app.doc.plate.center());
        assert!(app.doc.scene.is_empty());
    }

    #[test]
    fn a_file_that_cannot_be_read_leaves_the_scene_alone() {
        let mut app = SlicerApp::default();
        app.open_model(Handed::Path("does-not-exist.stl".into()));

        // The read is on a worker thread, so the failure lands on a later frame.
        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        assert!(app.doc.scene.is_empty());
        assert!(app.machine.status.is_error());
    }

    #[test]
    fn a_sliced_file_opens_as_a_model_where_its_pixels_stood() {
        // A 0.8 x 0.4 mm block lit at pixels 4..12 by 2..6 of 0.1 mm, three layers of 0.05.
        let path = crate::preview::tests::written_goo("app-startup", 3);
        let mut app = SlicerApp::default();
        app.open_model(Handed::Path(path));

        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        assert!(!app.machine.status.is_error(), "{:?}", app.machine.status);
        let object = &app.doc.scene.objects()[0];
        assert_eq!(object.name, "encrust-preview-app-startup.goo");
        let bounds = object.world_bounds().expect("a block has bounds");
        assert!(
            (bounds.mins.x - 0.4).abs() < 0.11 && (bounds.maxs.z - 0.15).abs() < 0.11,
            "the block stands where its pixels and layers were: {bounds:?}"
        );
    }

    #[test]
    fn a_file_no_reader_claims_is_opened_as_a_model() {
        assert!(!core_pipeline::reads_sliced_file(Path::new("cube.stl")));
        assert!(core_pipeline::reads_sliced_file(Path::new("plate.ctb")));
    }

    #[test]
    fn a_model_opened_from_the_command_line_reaches_the_plate() {
        let fixture =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl");
        let mut app = SlicerApp::default();
        app.open_model(Handed::Path(fixture));

        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        assert_eq!(app.doc.scene.objects().len(), 1);
        assert!(!app.machine.status.is_error(), "a sound cube opens");
    }

    #[test]
    fn the_view_key_runs_through_the_three_views_and_back() {
        let mut app = SlicerApp::default();
        let turn = |app: &mut SlicerApp, step| {
            shortcuts::act(&mut app.window(), shortcuts::Action::View(step));
            (app.mode, app.view.options.mask_only)
        };
        assert_eq!(turn(&mut app, 1), (Mode::Preview, false), "side by side");
        assert_eq!(turn(&mut app, 1), (Mode::Preview, true), "the layer alone");
        assert_eq!(
            turn(&mut app, 1),
            (Mode::Prepare, false),
            "round to the model"
        );
        assert_eq!(
            turn(&mut app, -1),
            (Mode::Preview, true),
            "and back the other way"
        );
    }

    /// Only the layer strip switches the view; a tool opens in the inspector beside it.
    #[test]
    fn picking_a_tool_leaves_the_view_alone() {
        let mut app = SlicerApp {
            mode: Mode::Preview,
            ..SlicerApp::default()
        };
        shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
        assert_eq!(app.tool, Tool::Hollow);
        assert_eq!(app.mode, Mode::Preview);
    }

    #[test]
    fn picking_the_open_tool_again_folds_the_inspector() {
        let mut app = SlicerApp::default();
        let pick = |app: &mut SlicerApp, tool| {
            shortcuts::act(&mut app.window(), shortcuts::Action::Pick(tool));
        };
        pick(&mut app, Tool::Hollow);
        assert!(app.view.options.inspector);
        pick(&mut app, Tool::Hollow);
        assert!(!app.view.options.inspector, "a second press folds it");
        pick(&mut app, Tool::Hollow);
        assert!(app.view.options.inspector, "a third unfolds it");
        pick(&mut app, Tool::Hollow);
        pick(&mut app, Tool::Cut);
        assert!(app.view.options.inspector, "another tool always opens");
        assert_eq!(app.tool, Tool::Cut);
    }

    /// The sheet is modal: a key pressed under it moves nothing on the plate.
    #[test]
    fn the_sheet_holds_the_keyboard_until_it_is_closed() {
        let mut app = SlicerApp::default();
        shortcuts::act(&mut app.window(), shortcuts::Action::Sheet);
        assert!(app.view.options.sheet);

        shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
        assert_eq!(
            app.tool,
            Tool::Position,
            "the tool is not reached under the sheet"
        );
        assert!(app.view.options.sheet);

        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(!app.view.options.sheet, "Escape closes it");
    }

    /// The report sheet is modal too, and Escape closes it: `shortcuts::take` reads the
    /// key before egui's pass, so the sheet would never see it otherwise.
    #[test]
    fn the_report_sheet_holds_the_keyboard_until_it_is_closed() {
        let mut app = SlicerApp::default();
        app.machine.report.open = true;

        shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
        assert_eq!(app.tool, Tool::Position, "the tool is not reached under it");
        assert!(app.machine.report.open);

        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(!app.machine.report.open, "Escape closes it");
    }

    /// The start page is left once and for good: a tool picked and put back does not
    /// bring it back over an empty plate.
    #[test]
    fn the_start_page_is_left_for_good() {
        let mut app = SlicerApp::default();
        assert!(crate::panels::still_starting(&mut app.window()));
        app.tool = Tool::Measure;
        assert!(!crate::panels::still_starting(&mut app.window()));
        app.tool = Tool::default();
        assert!(!crate::panels::still_starting(&mut app.window()));
    }

    /// A plate with nothing to print on says so at the stage's foot. From there two zones and
    /// a library pick reach a machine and a resin, each put under the plate unasked (ADR 0299).
    #[test]
    fn a_plate_with_nothing_to_print_on_is_set_up_from_its_notice() {
        use egui::accesskit::Role;
        use egui_kittest::kittest::Queryable as _;

        let dir = std::env::temp_dir().join(format!("encrust-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = SlicerApp::default();
        app.view.options.started = true;
        app.machine.slicing.catalogue = printer_profiles::Catalogue::with_root(&dir)
            .expect("a missing directory reads as empty");
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        assert!(
            !harness.state().machine.settings.machines,
            "nothing opens unasked"
        );
        harness.get_by_label("No printer or resin yet");

        for step in [
            "Set up",
            "Add a machine",
            "Elegoo",
            "Saturn 4 Ultra",
            "Add a resin",
        ] {
            harness.get_by_role_and_label(Role::Button, step).click();
            harness.run_steps(3);
        }
        let slicing = &harness.state().machine.slicing;
        assert_eq!(slicing.printer_id.as_deref(), Some("elegoo-saturn-4-ultra"));
        assert!(slicing.resin_id.is_some(), "the new resin is the plate's");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file dropped on Machine and resin is a profile rather than a model: a machine while
    /// the library is in view, a resin once its machine's resins are, and either goes under
    /// the plate that had none (ADR 0299).
    #[test]
    fn a_profile_dropped_on_machine_and_resin_is_imported_and_printed_on() {
        let dir = std::env::temp_dir().join(format!("encrust-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = SlicerApp::default();
        app.machine.slicing.catalogue = printer_profiles::Catalogue::with_root(&dir)
            .expect("a missing directory reads as empty");
        let printer = app
            .machine
            .slicing
            .catalogue
            .export(printer_profiles::Kind::Printer, "elegoo-saturn-4-ultra")
            .expect("a shipped machine exports");
        let printer_file =
            std::env::temp_dir().join(format!("encrust-drop-{}.toml", std::process::id()));
        std::fs::write(&printer_file, printer).expect("the temp directory is writable");
        crate::panels::open_machines(&mut app.machine);

        app.open_dropped(Handed::Path(printer_file.clone()));
        assert!(
            app.machine.slicing.printer_id.is_some(),
            "the dropped machine is printed on"
        );
        assert!(app.doc.scene.is_empty(), "nothing went on the plate");

        let resin = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../printer-profiles/tests/fixtures/saturn-4-ultra.cfgx");
        app.open_dropped(Handed::Path(resin));
        assert!(
            app.machine.slicing.resin_id.is_some(),
            "the dropped resin is printed with"
        );

        let _ = std::fs::remove_file(&printer_file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A machine the user removed is still in the library, and a run must not open on it.
    #[test]
    fn a_remembered_machine_that_is_no_longer_the_users_is_not_restored() {
        let read: crate::prefs::Preferences =
            serde_json::from_str(r#"{"printer":"elegoo-saturn-4-ultra","resin":"gone"}"#)
                .expect("every field of the file is optional");
        let mut app = SlicerApp::default();
        read.apply(&mut app.window());
        assert!(app.machine.slicing.printer_id.is_none());
        assert!(app.machine.slicing.resin_id.is_none());
    }

    /// Notices of different widths stand on one middle, the room between the side cards.
    #[test]
    fn every_notice_at_the_stages_foot_is_centred_on_one_line() {
        use egui_kittest::kittest::Queryable as _;

        let mut app = SlicerApp::default();
        app.view.options.started = true;
        app.machine.status = Status::Info("Opened /a/rather/long/path/to/Stand.obj".to_owned());
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        let unready = harness.get_by_label("No printer or resin yet").rect();
        let message = harness
            .get_by_label("Opened /a/rather/long/path/to/Stand.obj")
            .rect();
        let set_up = harness.get_by_label("Set up").rect();
        let unready_middle = (unready.left() + set_up.right()) / 2.0;
        assert!(
            (unready_middle - message.center().x).abs() < 16.0,
            "the notice centred at {unready_middle}, the message at {}",
            message.center().x
        );
    }

    /// A message with its figures longer than the room wraps inside it rather than running
    /// under the view tools beside it.
    #[test]
    fn a_long_message_wraps_inside_the_room_between_the_side_cards() {
        use crate::status::{Change, Reading};
        use egui_kittest::kittest::Queryable as _;

        let mut app = SlicerApp::default();
        app.view.options.started = true;
        let figure = |text: &str| Reading {
            text: text.to_owned(),
            change: Change::Better,
        };
        let message = "1 already stands the way it prints best; 1 stands on the plate";
        app.machine.status = Status::Readings(
            message.to_owned(),
            vec![figure("0.04 ml of supports"), figure("4.8 N of peel")],
        );
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 800.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        let tools = harness.get_by_label("Plate grid").rect();
        let last = harness.get_by_label("4.8 N of peel").rect();
        assert!(
            last.right() < tools.left(),
            "the message ends at {}, the view tools start at {}",
            last.right(),
            tools.left()
        );
    }

    /// A message under the pointer is being read, so it outlasts its moment, and goes once
    /// the pointer has left it for as long again.
    #[test]
    fn a_message_under_the_pointer_stays_until_it_is_left() {
        use egui_kittest::kittest::Queryable as _;

        let mut app = SlicerApp::default();
        app.view.options.started = true;
        app.machine.status = Status::Info("Hollowed 1 model(s), saving 70.9 ml".to_owned());
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .with_step_dt(0.5)
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(2);
        let at = harness
            .get_by_label("Hollowed 1 model(s), saving 70.9 ml")
            .rect()
            .center();
        harness.hover_at(at);
        harness.run_steps(16);
        assert!(
            harness
                .query_by_label("Hollowed 1 model(s), saving 70.9 ml")
                .is_some(),
            "eight seconds under the pointer"
        );

        harness.hover_at(egui::pos2(640.0, 300.0));
        harness.run_steps(12);
        assert!(
            harness
                .query_by_label("Hollowed 1 model(s), saving 70.9 ml")
                .is_none()
        );
    }

    /// A notice is as thin as its lines: the figures sit right under the message.
    #[test]
    fn a_message_and_its_figures_stand_close_together() {
        use crate::status::{Change, Reading};
        use egui_kittest::kittest::Queryable as _;

        let mut app = SlicerApp::default();
        app.view.options.started = true;
        let reading = Reading {
            text: "0.06 ml of supports".to_owned(),
            change: Change::Same,
        };
        app.machine.status =
            Status::Readings("Everything already stands".to_owned(), vec![reading]);
        let mut dressed = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut SlicerApp| {
                    if dressed {
                        app.window().show(ui);
                    } else {
                        crate::ui::theme::apply(ui.ctx());
                        dressed = true;
                    }
                },
                app,
            );
        harness.run_steps(3);
        let title = harness.get_by_label("Everything already stands").rect();
        let figures = harness.get_by_label("0.06 ml of supports").rect();
        let gap = figures.top() - title.bottom();
        assert!(
            (0.0..=4.0).contains(&gap),
            "{gap} points between the two lines"
        );
    }

    /// The Settings window holds the keyboard, and Escape takes down the question over it
    /// before the window itself.
    #[test]
    fn escape_closes_the_question_and_then_the_settings_window() {
        let mut app = SlicerApp::default();
        shortcuts::act(&mut app.window(), shortcuts::Action::Settings);
        assert!(app.machine.settings.open);

        let deleting =
            crate::settings::Deleting::Profile(printer_profiles::Kind::Support, "x".into());
        app.machine.settings.confirm = Some(deleting);
        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(app.machine.settings.confirm.is_none());
        assert!(app.machine.settings.open, "the window is still there");

        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(!app.machine.settings.open);
    }

    /// The Machine and resin window holds the keyboard, and Escape takes down the question
    /// over it before the window itself.
    #[test]
    fn escape_closes_the_question_and_then_the_machine_and_resin_window() {
        let mut app = SlicerApp::default();
        crate::panels::open_machines(&mut app.machine);
        assert!(app.machine.settings.machines);

        app.machine.settings.confirm = Some(crate::settings::Deleting::Resin("grey".to_owned()));
        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(app.machine.settings.confirm.is_none());
        assert!(app.machine.settings.machines, "the window is still there");

        shortcuts::act(&mut app.window(), shortcuts::Action::SelectAll);
        assert!(
            app.machine.settings.machines,
            "a key meant for the plate is held"
        );

        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(!app.machine.settings.machines);
    }

    #[test]
    fn a_switch_back_to_prepare_without_a_stack_keeps_its_own_height() {
        let mut app = SlicerApp::default();
        app.view.section.height_mm = Some(3.5);
        app.mode = Mode::Preview;
        app.carry_height();
        app.mode = Mode::Prepare;
        app.carry_height();
        assert_eq!(app.view.section.height_mm, Some(3.5));
    }

    /// Check and Export read their figures off the stack, so opening either cuts it
    /// without the user having to look at a layer first.
    #[test]
    fn opening_check_or_export_cuts_the_stack_in_the_model_view() {
        for tool in [Tool::Check, Tool::Export] {
            let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/cube.stl");
            let mut app = SlicerApp::default();
            let profile = app
                .machine
                .slicing
                .catalogue
                .printer("elegoo-saturn-4-ultra")
                .expect("the printer ships")
                .profile
                .clone();
            let id = Some("elegoo-saturn-4-ultra".to_owned());
            crate::profiles::apply_printer(&mut app.window(), profile, id);
            app.open_model(Handed::Path(fixture));
            while app.doc.imports.poll(
                &mut app.doc.scene,
                &app.doc.plate,
                &mut app.view.camera,
                &mut app.machine.status,
            ) {
                std::thread::yield_now();
            }

            app.refresh_preview();
            assert!(
                !app.machine.preview.is_building(),
                "the model view alone cuts nothing"
            );
            app.tool = tool;
            app.refresh_preview();
            assert!(
                app.machine.preview.is_building(),
                "{} cuts the stack",
                tool.label()
            );
            assert_eq!(app.mode, Mode::Prepare, "and the view stays on the model");
        }
    }

    #[test]
    fn the_preview_is_not_built_without_a_printer_profile() {
        let mut app = SlicerApp {
            mode: Mode::Preview,
            ..SlicerApp::default()
        };
        app.refresh_preview();
        assert!(!app.machine.preview.is_building());
        assert!(
            !app.machine.status.is_error(),
            "an empty plate is not a failure"
        );
    }

    const IMPORTED_ON: &str = "elegoo-saturn-4-ultra";

    /// The app over a profile directory of the test's own, which every import lands in.
    fn with_profile_dir(name: &str) -> SlicerApp {
        let dir =
            std::env::temp_dir().join(format!("encrust-profiles-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = SlicerApp::default();
        app.machine.slicing.catalogue = printer_profiles::Catalogue::with_root(&dir)
            .expect("a missing directory reads as empty");
        app
    }

    fn handed<T>(name: &str, text: Result<String, T>) -> Handed
    where
        T: std::fmt::Debug,
    {
        let text = text.expect("the profile serialises");
        Handed::bytes(name, std::sync::Arc::from(text.into_bytes()))
    }

    #[test]
    fn a_machine_from_the_file_menu_is_kept_and_put_under_the_plate() {
        let mut app = with_profile_dir("printer");
        let catalogue = &app.machine.slicing.catalogue;
        let file = handed(
            "saturn.toml",
            catalogue.export(printer_profiles::Kind::Printer, IMPORTED_ON),
        );

        crate::profiles::import_printer(&mut app.window(), &file);
        let id = app
            .machine
            .slicing
            .printer_id
            .clone()
            .expect("a machine in hand");
        let entry = app.machine.slicing.catalogue.printer(&id).expect("kept");
        assert!(
            crate::settings::installed(&entry.source),
            "one of the user's machines, with an id to remember"
        );
    }

    #[test]
    fn a_resin_from_the_file_menu_goes_on_the_machine_in_hand() {
        let mut app = with_profile_dir("resin");
        let profile = app
            .machine
            .slicing
            .catalogue
            .printer(IMPORTED_ON)
            .expect("ships")
            .profile
            .clone();
        crate::profiles::apply_printer(&mut app.window(), profile, Some(IMPORTED_ON.to_owned()));
        let resin = printer_profiles::MaterialProfile {
            name: "Bench grey".to_owned(),
            ..printer_profiles::MaterialProfile::default()
        };
        let file = handed(
            "grey.toml",
            resin.to_toml_string(std::path::Path::new("grey")),
        );

        crate::profiles::import_material(&mut app.machine, &file);
        let id = app
            .machine
            .slicing
            .resin_id
            .clone()
            .expect("a resin in hand");
        let kept = &app
            .machine
            .slicing
            .catalogue
            .resin(&id)
            .expect("kept")
            .profile;
        assert!(kept.is_tuned_for(IMPORTED_ON));
    }

    #[test]
    fn a_sliced_file_handed_in_as_a_resin_goes_on_the_machine_in_hand() {
        let mut app = with_profile_dir("sliced-resin");
        app.machine.slicing.printer_id = Some(IMPORTED_ON.to_owned());
        let path = crate::preview::tests::written_goo("sliced-resin", 3);

        crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
        let id = app
            .machine
            .slicing
            .resin_id
            .clone()
            .expect("the file's resin is in hand");
        let kept = &app
            .machine
            .slicing
            .catalogue
            .resin(&id)
            .expect("kept")
            .profile;
        assert!(kept.is_tuned_for(IMPORTED_ON));
        assert_eq!(kept.for_printer(IMPORTED_ON).layer_height_mm, 0.05);
    }

    #[test]
    fn a_sliced_file_for_no_known_machine_waits_for_one_in_hand() {
        let mut app = with_profile_dir("sliced-resin-unknown");
        app.machine.slicing.printer_id = None;
        let path = crate::preview::tests::written_goo("sliced-resin-unknown", 3);

        crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
        assert!(matches!(app.machine.status, Status::Error(_)));
        assert_eq!(app.machine.slicing.catalogue.resins().count(), 0);
    }

    #[test]
    fn a_resin_another_program_exported_goes_on_the_machine_it_names() {
        let mut app = with_profile_dir("foreign-resin");
        app.machine.slicing.printer_id = None;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../printer-profiles/tests/fixtures/saturn-4-ultra.cfgx");

        crate::profiles::import_material(&mut app.machine, &Handed::Path(path));
        let id = app
            .machine
            .slicing
            .resin_id
            .clone()
            .expect("the exported resin is in hand");
        let kept = &app
            .machine
            .slicing
            .catalogue
            .resin(&id)
            .expect("kept")
            .profile;
        assert!(kept.is_tuned_for(IMPORTED_ON));
        assert_eq!(kept.for_printer(IMPORTED_ON).light_pwm, 211);
    }
}
