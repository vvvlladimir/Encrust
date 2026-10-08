//! The buffers under the viewport: one that grows as a frame needs it, and a mesh cut
//! into pieces the card will take (ADR 0070).

use std::ops::Range;

use bytemuck::Pod;
use core_geometry::Mesh;

use crate::render::vertex::{ModelVertex, flat_shaded_vertices};

use super::*;

const INITIAL_CAPACITY_BYTES: u64 = 4096;

/// One buffer of a cached mesh, and the faces of that mesh it holds. A draw asking for
/// part of a mesh takes the pieces that fall inside it, which is why `pieces` breaks at
/// the ends of that part as well as at the card's ceiling.
pub(super) struct Piece {
    pub(super) buffer: wgpu::Buffer,
    pub(super) vertices: u32,
    pub(super) faces: Range<usize>,
}

/// A vertex buffer that grows to fit whatever the frame needs and never shrinks.
pub(super) struct DynamicBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) capacity_bytes: u64,
    pub(super) label: &'static str,
}

impl DynamicBuffer {
    pub(super) fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            buffer: allocate(device, label, INITIAL_CAPACITY_BYTES),
            capacity_bytes: INITIAL_CAPACITY_BYTES,
            label,
        }
    }

    pub(super) fn write<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) {
        let bytes = bytemuck::cast_slice(data);
        if bytes.is_empty() {
            return;
        }
        if bytes.len() as u64 > self.capacity_bytes {
            self.capacity_bytes = (bytes.len() as u64).next_power_of_two();
            self.buffer = allocate(device, self.label, self.capacity_bytes);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }
}

fn allocate(device: &wgpu::Device, label: &'static str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A mesh flat-shaded into as few buffers as the card's own ceiling allows.
///
/// Expanded a piece at a time as well as uploaded a piece at a time: a cavity of eleven
/// million triangles is eight hundred megabytes of vertices, and holding all of them to
/// hand one buffer over is the same wall from the other side.
pub(super) fn pieces(device: &wgpu::Device, mesh: &Mesh, splits: &[usize]) -> Vec<Piece> {
    let ceiling = device.limits().max_buffer_size as usize / size_of::<ModelVertex>();
    let faces_per_piece = (ceiling / 3).max(1);

    let mut first = 0;
    let mut pieces = Vec::new();
    while first < mesh.faces.len() {
        let ceiling = (first + faces_per_piece).min(mesh.faces.len());
        let last = splits
            .iter()
            .copied()
            .filter(|split| (first + 1..ceiling).contains(split))
            .min()
            .unwrap_or(ceiling);
        let vertices = flat_shaded_vertices(mesh, first..last);
        if !vertices.is_empty() {
            pieces.push(Piece {
                buffer: upload(device, "viewport_mesh", &vertices),
                vertices: vertices.len() as u32,
                faces: first..last,
            });
        }
        first = last;
    }
    pieces
}

/// Creates a buffer holding exactly `data`. Used for meshes, which never change once
/// imported; anything that changes every frame goes through a [`DynamicBuffer`].
pub(super) fn upload<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    data: &[T],
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage: wgpu::BufferUsages::VERTEX,
    })
}
