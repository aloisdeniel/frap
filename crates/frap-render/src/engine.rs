// SPDX-License-Identifier: GPL-3.0-or-later
//! `trait Engine` and the GPU cache (§19.2). One uber-shader per engine (WP 1.7).

use std::collections::HashMap;

use frap_render_scene::{Film, MeshKey, RMesh, RenderError, RenderScene};
use wgpu::util::DeviceExt;

use crate::gpu::Gpu;

/// Renders after which a mesh that no render used is dropped from the cache.
const MESH_TTL: u64 = 8;

/// Vertex layout of [`GpuMesh::vertices`]: position then normal, `f32 × 3` each.
pub(crate) const VERTEX_STRIDE: u64 = 24;

/// A mesh uploaded to the GPU (corner-split, so one vertex per corner).
pub(crate) struct GpuMesh {
    /// Interleaved `[position, normal]` vertices.
    pub(crate) vertices: wgpu::Buffer,
    /// `u32` triangle indices.
    pub(crate) indices: wgpu::Buffer,
    /// Number of indices (a multiple of 3).
    pub(crate) index_count: u32,
    last_used: u64,
}

/// GPU resources kept between renders: meshes by `MeshKey` (textures by `ImageKey` arrive with
/// the texture colour type, M2/M3).
#[derive(Default)]
pub struct GpuCache {
    meshes: HashMap<MeshKey, GpuMesh>,
    /// Render counter (for eviction).
    epoch: u64,
    uploads: u64,
}

impl GpuCache {
    /// Starts a render: meshes no render used during the last 8 renders are dropped.
    pub fn begin_render(&mut self) {
        self.epoch += 1;
        let epoch = self.epoch;
        self.meshes
            .retain(|_, m| epoch.saturating_sub(m.last_used) <= MESH_TTL);
    }

    /// Uploads `mesh` under `key` unless it is already cached (a `MeshKey` is an evaluated
    /// fingerprint, so the same key always carries the same data). `None` for meshes without
    /// triangles.
    pub(crate) fn mesh(&mut self, gpu: &Gpu, key: MeshKey, mesh: &RMesh) -> Option<&GpuMesh> {
        if mesh.indices.len() < 3 || mesh.positions.is_empty() {
            return None;
        }
        let epoch = self.epoch;
        let uploads = &mut self.uploads;
        let m = self.meshes.entry(key).or_insert_with(|| {
            *uploads += 1;
            upload(gpu, mesh)
        });
        m.last_used = epoch;
        Some(m)
    }

    /// A cached mesh.
    pub(crate) fn get(&self, key: MeshKey) -> Option<&GpuMesh> {
        self.meshes.get(&key)
    }

    /// Number of meshes resident.
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Number of mesh uploads since creation (cache-hit tests).
    pub fn upload_count(&self) -> u64 {
        self.uploads
    }
}

fn upload(gpu: &Gpu, mesh: &RMesh) -> GpuMesh {
    let mut data: Vec<f32> = Vec::with_capacity(mesh.positions.len() * 6);
    for (i, p) in mesh.positions.iter().enumerate() {
        let n = mesh.normals.get(i).copied().unwrap_or([0.0, 0.0, 0.0]);
        data.extend_from_slice(p);
        data.extend_from_slice(&n);
    }
    // Indices past the vertex count would read out of bounds (robust access hides it on some
    // backends, not all): clamp them to a degenerate triangle on vertex 0.
    let nv = mesh.positions.len() as u32;
    let whole = mesh.indices.len() / 3 * 3;
    let indices: Vec<u32> = mesh.indices[..whole]
        .iter()
        .map(|&i| if i < nv { i } else { 0 })
        .collect();
    let vertices = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frap mesh vertices"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX,
        });
    let index_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frap mesh indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
    GpuMesh {
        vertices,
        indices: index_buf,
        index_count: indices.len() as u32,
        last_used: 0,
    }
}

/// A render engine (Workbench, EEVEE-like).
pub trait Engine {
    /// Renders one scene to scene-linear pixels.
    fn render(
        &mut self,
        gpu: &Gpu,
        cache: &mut GpuCache,
        scene: &RenderScene,
    ) -> Result<Film, RenderError>;
}
