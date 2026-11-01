//! The render view's key light shadow: a depth map seen from the key over the mat, baked when
//! what stands on it changes and read wherever the key falls. See docs/design/viewport.md.

use std::hash::{DefaultHasher, Hash, Hasher};

use core_geometry::glam::camera::rh::proj::directx::orthographic;
use core_geometry::glam::camera::rh::view::look_to_mat4;
use core_geometry::{Mat4, Vec3};

use super::can_print::CanPrintTexture;
use super::environment::Environment;
use super::shadow::{Bake, hash_casters};
use super::*;

/// Texels along each side of the map: an A2 mat is 0.3 mm a texel, under the softening.
const KEY_TEXELS: u32 = 2048;
pub(super) const KEY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Millimetres above the plate the map reaches when nothing taller stands on it: past the
/// tallest prop, a can.
const LEAST_TOP_MM: f32 = 130.0;
/// Millimetres the map reaches past the mat's edges, so a prop's shadow falls on the table.
const PAST_MAT_MM: f32 = 60.0;

/// The map, the comparing sampler it is read through, and what it was last baked from.
pub(super) struct KeyShadow {
    pub(super) cast: Facing,
    /// The map once the render view first asks for it, and a texel standing in until then
    /// and while the map is being drawn.
    map: Option<wgpu::TextureView>,
    stand_in: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bake: Bake,
    /// Whether the map was made and not yet drawn into, which leaves it all in shadow.
    fresh: bool,
}

impl KeyShadow {
    pub(super) fn new(device: &wgpu::Device, cast: Facing) -> Self {
        Self {
            cast,
            map: None,
            stand_in: map(device, 1),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("viewport_key_shadow"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            }),
            bake: Bake::default(),
            fresh: false,
        }
    }

    /// The globals, with the key's map, the studio's and the can's print where the shader reads
    /// them: stand-ins for a pass that draws into either map, `casting`, or before there is one.
    pub(super) fn globals_group(
        &self,
        device: &wgpu::Device,
        (layout, globals): (&wgpu::BindGroupLayout, &wgpu::Buffer),
        (environment, can_print): (&Environment, &CanPrintTexture),
        casting: bool,
    ) -> wgpu::BindGroup {
        let map = match (&self.map, casting) {
            (Some(map), false) => map,
            _ => &self.stand_in,
        };
        let studio = match casting {
            true => &environment.stand_in,
            false => &environment.view,
        };
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_globals"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(map),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(studio),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&environment.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&can_print.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&can_print.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(map),
                },
            ],
        })
    }
}

fn map(device: &wgpu::Device, texels: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_key_shadow"),
            size: wgpu::Extent3d {
                width: texels,
                height: texels,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: KEY_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// The key's view of the box over the mat `desk`, up to `top_mm`: an orthographic one along
/// `towards`, the way the key light travels, holding the whole box in its clip space.
pub(super) fn key_view_projection(desk: &Desk, top_mm: f32, towards: Vec3) -> Mat4 {
    let [low_x, low_y, high_x, high_y] = desk.rect;
    let (low_x, low_y) = (low_x - PAST_MAT_MM, low_y - PAST_MAT_MM);
    let (high_x, high_y) = (high_x + PAST_MAT_MM, high_y + PAST_MAT_MM);
    let towards = towards.normalize_or(Vec3::NEG_Z);
    let up = if towards.cross(Vec3::Z).length_squared() < 1e-6 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let middle = Vec3::new((low_x + high_x) / 2.0, (low_y + high_y) / 2.0, top_mm / 2.0);
    let view = look_to_mat4(middle, towards, up);
    let top = top_mm.max(LEAST_TOP_MM);
    let mut low = Vec3::splat(f32::MAX);
    let mut high = Vec3::splat(f32::MIN);
    for x in [low_x, high_x] {
        for y in [low_y, high_y] {
            for z in [-10.0, top] {
                let seen = view.transform_point3(Vec3::new(x, y, z));
                low = low.min(seen);
                high = high.max(seen);
            }
        }
    }
    let projection = orthographic(low.x, high.x, low.y, high.y, -high.z, -low.z);
    projection * view
}

/// What the map is cast by, as one number: the set, whether the resin is clear, and every
/// model's and marker's mesh, faces and placement.
fn cast_by(frame: &FrameInput<'_>, look: &Look) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytemuck::bytes_of(&look.desk.rect).hash(&mut hasher);
    look.attenuation_mm.to_bits().hash(&mut hasher);
    hash_casters(frame, &mut hasher);
    hasher.finish()
}

impl ViewportResources {
    /// Makes the map the first time the render view asks for it, and asks for a bake when
    /// what casts it changed.
    pub(super) fn want_key_shadow(&mut self, device: &wgpu::Device, frame: &FrameInput<'_>) {
        let Some(look) = frame.look.filter(|_| self.reads_planes) else {
            return;
        };
        if self.key.map.is_none() {
            self.key.map = Some(map(device, KEY_TEXELS));
            self.key.fresh = true;
            self.rebind_globals(device);
        }
        self.key.bake.want(Some(cast_by(frame, &look)), frame.light);
    }

    /// Binds the globals again after a texture they read was made.
    pub(super) fn rebind_globals(&mut self, device: &wgpu::Device) {
        let globals = (&self.globals_layout, &self.globals);
        let textures = (&self.environment, &self.can_print);
        self.globals_bind_group = self.key.globals_group(device, globals, textures, false);
    }

    /// Draws every model and marker into the map from the key, if what casts it changed; a
    /// map just made is cleared to all lit until then.
    pub fn bake_key_shadow(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let due = self.key.bake.due();
        let Some(map) = self.key.map.as_ref().filter(|_| due || self.key.fresh) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport_key_shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: map,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.casting_bind_group, &[]);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        let mut pass = pass.forget_lifetime();
        self.key.fresh = false;
        if !due {
            return;
        }
        // A clear resin lets the key through; its contact shadow is what it leaves.
        let (models, markers) = self.frame.models.split_at(self.frame.markers_from);
        if !self.frame.clear {
            self.draw_meshes(&mut pass, &self.key.cast, models, 0);
        }
        let from = self.frame.markers_from as u32;
        self.draw_meshes(&mut pass, &self.key.cast, markers, from);
        drop(pass);
        self.key.bake.baked = self.key.bake.wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_sees_the_whole_set_inside_its_map() {
        let desk = Desk {
            rect: [-100.0, -50.0, 500.0, 400.0],
            mat_mm: [600.0, 450.0],
        };
        let towards = Vec3::new(0.4, 0.55, -0.85);
        let seen = key_view_projection(&desk, 200.0, towards);
        for x in [-100.0, 500.0] {
            for y in [-50.0, 400.0] {
                for z in [0.0, 200.0] {
                    let clip = seen.project_point3(Vec3::new(x, y, z));
                    let inside = clip.x.abs() <= 1.0 + 1e-4
                        && clip.y.abs() <= 1.0 + 1e-4
                        && (-1e-4..=1.0 + 1e-4).contains(&clip.z);
                    assert!(inside, "({x}, {y}, {z}) lands at {clip}, off the map");
                }
            }
        }
    }

    #[test]
    fn a_set_of_no_size_still_has_a_view() {
        let seen = key_view_projection(&Desk::default(), 0.0, Vec3::new(0.4, 0.55, -0.85));
        assert!(seen.is_finite(), "expected a finite matrix, got {seen}");
    }

    #[test]
    fn the_key_straight_down_still_has_a_view() {
        let desk = Desk {
            rect: [0.0, 0.0, 100.0, 100.0],
            mat_mm: [100.0, 100.0],
        };
        let seen = key_view_projection(&desk, 50.0, Vec3::NEG_Z);
        assert!(seen.is_finite(), "expected a finite matrix, got {seen}");
    }
}
