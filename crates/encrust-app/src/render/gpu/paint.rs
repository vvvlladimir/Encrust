//! Replaying what `prepare` decided: the passes, in the order the stencil needs them.

use super::pipelines::MATERIAL_ABOVE;
use super::*;

impl ViewportResources {
    /// Draws the frame `prepare` recorded into the viewport's own target, into `viewport`
    /// of a window `size_px` pixels large.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: egui::epaint::ViewportInPixels,
    ) {
        self.bake_shadow(encoder);
        let mut pass = self.target.begin(device, encoder, size_px, viewport);
        self.paint(&mut pass);
    }

    /// Lays what `draw` drew over egui's pass.
    pub fn present(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        self.target.present(render_pass);
    }

    /// Records the scene into a pass whose depth plane is [`DEPTH_FORMAT`].
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_bind_group(0, &self.globals_bind_group, &[]);
        self.paint_plate(render_pass);

        if self.frame.line_vertices > 0 {
            render_pass.set_pipeline(&self.line_pipeline);
            render_pass.set_vertex_buffer(0, self.lines.buffer.slice(..));
            render_pass.draw(0..self.frame.line_vertices, 0..1);
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

        let models = match self.frame.xray {
            true => &self.xray_pipeline,
            false => &self.model_pipeline,
        };
        self.draw_meshes(render_pass, models, &self.frame.models, 0);

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

        if self.frame.cutting && self.frame.cap_vertices > 0 {
            render_pass.set_pipeline(&self.capping.cap);
            render_pass.set_stencil_reference(MATERIAL_ABOVE);
            render_pass.set_vertex_buffer(0, self.cap.buffer.slice(..));
            render_pass.draw(0..self.frame.cap_vertices, 0..1);
        }

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

        // Last, so the models' depth keeps it off whatever stands in front of the plate.
        self.paint_shadow(render_pass);
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

    /// Draws one mesh per entry, each with the instance that many slots past `base`.
    fn draw_meshes(
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
    fn draw_mesh(
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
            render_pass.set_vertex_buffer(0, piece.buffer.slice(..));
            render_pass.draw(0..piece.vertices, instance..instance + 1);
        }
    }
}
