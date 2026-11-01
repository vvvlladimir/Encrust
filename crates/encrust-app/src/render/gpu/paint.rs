//! Replaying what `prepare` decided: the passes, in the order the stencil needs them.

use super::pipelines::MATERIAL_ABOVE;
use super::settle;
use super::*;
use crate::render::occlusion::Occlusion;

impl ViewportResources {
    /// Draws the frame `prepare` recorded into the viewport's own target, into `viewport`
    /// of a window `size_px` pixels large.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: egui::epaint::ViewportInPixels,
    ) {
        // A picture at rest that has averaged enough is held as it is.
        if self
            .frame
            .sample
            .is_some_and(|sample| sample >= SETTLED_SAMPLES)
        {
            return;
        }
        self.bake_shadow(encoder);
        self.bake_key_shadow(encoder);
        self.bake_environment(encoder);
        self.measure_thickness(device, encoder, size_px, &viewport);
        let occlusion = Occlusion::of(&viewport, self.frame.staged, self.frame.occlusion);
        self.target.occlude(queue, &occlusion);
        self.place(queue, &viewport);
        let timestamps = self.timer.as_mut().and_then(|timer| timer.begin());
        let mut pass = self.target.begin(
            device,
            encoder,
            size_px,
            &viewport,
            timestamps,
            self.frame.clear,
        );
        self.paint(&mut pass);
        drop(pass);
        if self.frame.clear {
            self.paint_clear(encoder, &viewport);
        }
        let at = (size_px, &viewport);
        self.target
            .accumulate(device, encoder, at, self.frame.sample);
        if let Some(timer) = &mut self.timer {
            timer.end(encoder, self.planned_faces);
        }
    }

    /// Tells the shader where the viewport stands in the framebuffer, which only `draw` knows.
    pub fn place(&self, queue: &wgpu::Queue, viewport: &egui::epaint::ViewportInPixels) {
        let screen = [
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
        ];
        let at = std::mem::offset_of!(Globals, screen) as wgpu::BufferAddress;
        queue.write_buffer(&self.globals, at, bytemuck::bytes_of(&screen));
        // Each frame of a picture at rest is drawn a fraction of a pixel aside.
        if let Some(sample) = self.frame.sample {
            let size_px = [screen[2], screen[3]];
            let moved = settle::jittered(
                self.frame.view_projection,
                settle::jitter_px(sample),
                size_px,
            );
            let at = std::mem::offset_of!(Globals, view_projection) as wgpu::BufferAddress;
            queue.write_buffer(
                &self.globals,
                at,
                bytemuck::bytes_of(&moved.to_cols_array_2d()),
            );
        }
    }

    /// Lays what `draw` drew over egui's pass.
    pub fn present(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        self.target.present(render_pass);
    }

    /// Records the scene into a pass whose depth plane is [`DEPTH_FORMAT`].
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_bind_group(0, &self.globals_bind_group, &[]);
        if self.frame.staged {
            render_pass.set_pipeline(&self.backdrop);
            render_pass.draw(0..3, 0..1);
        } else {
            self.paint_plate(render_pass);
        }

        if self.frame.line_vertices > 0 {
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_vertex_buffer(0, self.lines.buffer.slice(..));
            render_pass.draw(0..self.frame.line_vertices, 0..1);
        }
        if self.frame.wide_lines > 0 {
            render_pass.set_pipeline(&self.wide_line_pipeline);
            render_pass.set_vertex_buffer(0, self.wide_lines.buffer.slice(..));
            render_pass.draw(0..6, 0..self.frame.wide_lines);
        }

        render_pass.set_vertex_buffer(1, self.instances.buffer.slice(..));

        // First of all, because these passes own the depth and stencil planes while they
        // run and hand them back empty.
        self.paint_cuts(render_pass);

        // The cut is capped rather than left open, and the cap is stencilled: what the
        // cut took away is counted first, the models are drawn, and the face is filled
        // wherever the count says the plane runs through a solid. See
        // `docs/decisions/0062`.
        if self.frame.cutting {
            let crossing = &self.capping.crossing;
            self.draw_meshes(
                render_pass,
                crossing,
                &self.frame.solids,
                self.frame.solid_base,
            );
        }

        let models = self.models_pipeline(render_pass);
        let (drawn, markers) = self.frame.models.split_at(self.frame.markers_from);
        // A clear resin is drawn after this pass, over a copy of what it leaves.
        if !self.frame.clear {
            self.draw_meshes(render_pass, models, drawn, 0);
        }

        if !self.frame.reliefs.is_empty() {
            for (index, (key, mirrored)) in self.frame.reliefs.iter().enumerate() {
                let Some(relief) = self.reliefs.get(key) else {
                    continue;
                };
                let instance = self.frame.relief_base + index as u32;
                render_pass.set_pipeline(self.relief_pipeline.of(*mirrored));
                render_pass.set_bind_group(1, &relief.bind_group, &[]);
                for (buffer, count) in &relief.pieces {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.draw(0..*count, instance..instance + 1);
                }
            }
        }

        // Before the caps, which lay down no depth for it to be hidden by.
        if let Some((_, atlas)) = self
            .atlas
            .as_ref()
            .filter(|_| self.frame.label_vertices > 0)
        {
            render_pass.set_pipeline(&self.label_pipeline);
            render_pass.set_bind_group(1, atlas, &[]);
            render_pass.set_vertex_buffer(0, self.label.buffer.slice(..));
            render_pass.draw(0..self.frame.label_vertices, 0..1);
        }

        // Before the caps, which take no depth: the model's floor lies in the plate, and the
        // shadow on it would win that tie over the cap by turns.
        self.paint_shadow(render_pass);

        if self.frame.cutting && self.frame.cap_vertices > 0 {
            render_pass.set_pipeline(&self.capping.cap);
            render_pass.set_stencil_reference(MATERIAL_ABOVE);
            render_pass.set_vertex_buffer(0, self.cap.buffer.slice(..));
            render_pass.draw(0..self.frame.cap_vertices, 0..1);
        }
        self.paint_bottom_cap(render_pass);

        // After the caps, which keep no depth: a blocker inside the material shows through
        // the cut, and the model's own faces still hide it everywhere else.
        let from = self.frame.markers_from as u32;
        self.draw_meshes(render_pass, &self.marker_pipelines.far, markers, from);
        self.draw_meshes(render_pass, &self.marker_pipelines.near, markers, from);
        // Last, over everything they stand in front of, and drawing no depth of their own.
        if self.frame.clear {
            self.draw_meshes(render_pass, &self.clear.inner, drawn, 0);
        }
    }

    /// The pipeline the models are drawn by this frame, with the planes it reads bound: seen
    /// through, glowing where they are thin, or plain.
    fn models_pipeline(&self, render_pass: &mut wgpu::RenderPass<'static>) -> &Facing {
        let planes = self.clear.planes().filter(|_| self.frame.thick);
        match (self.frame.xray, planes) {
            (true, _) => &self.xray_pipeline,
            (false, Some(planes)) => {
                render_pass.set_bind_group(1, planes, &[]);
                &self.clear.glowing
            }
            (false, None) => &self.model_pipeline,
        }
    }

    /// Draws the inside of every cut: the far wall of each body, kept where the object it
    /// is cut into stands in front of it. Four passes over a pair and the stencil back to
    /// zero; see ADR 0201.
    fn paint_cuts(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        for (index, (body, solid)) in self.frame.cuts.iter().enumerate() {
            let pair = self.frame.cut_base + 2 * index as u32;
            let cutting = &self.cutting;
            self.draw_mesh(render_pass, &cutting.candidate, body, pair);
            self.draw_mesh(render_pass, &cutting.counting, solid, pair + 1);
            render_pass.set_stencil_reference(MATERIAL_ABOVE);
            self.draw_mesh(render_pass, &cutting.surface, body, pair);
            self.draw_mesh(render_pass, &cutting.wipe, body, pair);
            render_pass.set_pipeline(&cutting.reset);
            render_pass.draw(0..3, 0..1);
        }
    }

    /// Caps the cut from below the same way, once the top cap's count is cleared: the
    /// bottom cap is only ever seen from under it, where its count is the one in front.
    fn paint_bottom_cap(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        if !self.frame.cutting_bottom || self.frame.bottom_cap_vertices == 0 {
            return;
        }
        render_pass.set_pipeline(&self.cutting.reset);
        render_pass.draw(0..3, 0..1);
        let crossing = &self.capping.bottom_crossing;
        self.draw_meshes(
            render_pass,
            crossing,
            &self.frame.solids,
            self.frame.solid_base,
        );
        render_pass.set_pipeline(&self.capping.cap);
        render_pass.set_stencil_reference(MATERIAL_ABOVE);
        render_pass.set_vertex_buffer(0, self.cap.buffer.slice(..));
        let from = self.frame.cap_vertices;
        render_pass.draw(from..from + self.frame.bottom_cap_vertices, 0..1);
    }

    /// Draws one mesh per entry, each with the instance that many slots past `base`.
    pub(super) fn draw_meshes(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        pipelines: &Facing,
        drawn: &[Drawn],
        base: u32,
    ) {
        for (index, draw) in drawn.iter().enumerate() {
            self.draw_mesh(render_pass, pipelines, draw, base + index as u32);
        }
    }

    /// Draws the pieces of one mesh that lie inside the range the draw covers.
    pub(super) fn draw_mesh(
        &self,
        render_pass: &mut wgpu::RenderPass<'static>,
        pipelines: &Facing,
        draw: &Drawn,
        instance: u32,
    ) {
        let Some(mesh) = self.meshes.get(&draw.key) else {
            return;
        };
        render_pass.set_pipeline(pipelines.of(draw.mirrored));
        for piece in &mesh.pieces {
            if piece.faces.start < draw.faces.start || piece.faces.end > draw.faces.end {
                continue;
            }
            render_pass.set_vertex_buffer(0, piece.vertices.slice(..));
            render_pass.set_index_buffer(piece.indices.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..piece.index_count, 0, instance..instance + 1);
        }
    }
}
