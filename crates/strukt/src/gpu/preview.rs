//! The 3D preview: one mesh, rendered offscreen and handed to egui as an image.
//!
//! Offscreen rather than a paint callback into egui's own pass, for one reason: that pass has no
//! depth attachment, and a solid drawn without a depth buffer shows its far side through its near
//! side. Rendering to our own texture costs one blit and buys correct occlusion.
//!
//! The vertices come straight from [`gizmo_nfs::NfsMeshPart`] with the file's own placement
//! applied ([`gizmo_nfs::placement`]) and its own frame kept (Z-up, 1 unit = 1 m), so what the
//! viewport shows is what the file says — not a re-interpretation of it.

use crate::gpu::math::M4;
use eframe::egui_wgpu::RenderState;
use gizmo_nfs::placement::{part_centroid, place_dir, place_point, should_place};
use gizmo_nfs::NfsMeshPart;
use wgpu::util::DeviceExt as _;

/// One vertex: position and normal, both in the file's frame.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

/// The uniform block the shader reads.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    clip_from_world: M4,
    /// `xyz` = the key light's direction, `w` unused.
    key_dir: [f32; 4],
}

/// What is currently uploaded, so a redraw of the same selection costs nothing.
struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    /// Bounds in the file's frame, for framing the camera and for the overlay.
    pub bounds: ([f32; 3], [f32; 3]),
    /// What is in the buffer, so an unchanged selection is not re-uploaded.
    key: MeshKey,
}

/// Identifies what a mesh was built from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeshKey {
    /// One solid, by the offset of its chunk.
    Solid(usize),
    /// The showroom configuration — what `select_stock_car` picks.
    StockCar,
}

/// The preview renderer, held by the app for the lifetime of the window.
pub struct Preview {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    colour: wgpu::Texture,
    depth: wgpu::TextureView,
    size: [u32; 2],
    /// The egui handle for the colour texture.
    texture_id: egui::TextureId,
    mesh: Option<Mesh>,
}

impl Preview {
    /// Build the pipeline. `None` when eframe is running without a wgpu backend, in which case
    /// the tab says so rather than the app refusing to start.
    pub fn new(state: &RenderState) -> Option<Self> {
        let device = &state.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("strukt.preview"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("strukt.preview.bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("strukt.preview.uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("strukt.preview.bg"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("strukt.preview.layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("strukt.preview.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            // Double-sided: these meshes are authored for a renderer that culls differently, and a
            // missing face reads as a hole in the part — worse than a slightly wrong silhouette.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let size = [16, 16];
        let (colour, depth) = targets(device, size);
        let view = colour.create_view(&wgpu::TextureViewDescriptor::default());
        let texture_id = state.renderer.write().register_native_texture(
            device,
            &view,
            wgpu::FilterMode::Linear,
        );
        Some(Self { pipeline, uniforms, bind_group, colour, depth, size, texture_id, mesh: None })
    }

    /// What is currently uploaded.
    #[must_use]
    pub fn mesh_key(&self) -> Option<MeshKey> {
        self.mesh.as_ref().map(|m| m.key)
    }

    /// Bounds of the uploaded mesh, in the file's frame.
    #[must_use]
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.mesh.as_ref().map(|m| m.bounds)
    }

    /// Triangles currently uploaded.
    #[must_use]
    pub fn triangles(&self) -> u32 {
        self.mesh.as_ref().map_or(0, |m| m.index_count / 3)
    }

    /// Upload a set of parts as the mesh to show.
    pub fn upload(&mut self, state: &RenderState, key: MeshKey, parts: &[&NfsMeshPart]) {
        let (mut vertices, mut indices) = (Vec::new(), Vec::<u32>::new());
        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for part in parts {
            let apply = should_place(&part.transform, &part_centroid(part));
            let base = vertices.len() as u32;
            for (i, p) in part.positions.iter().enumerate() {
                let position = place_point(&part.transform, *p, apply);
                let normal = part
                    .normals
                    .get(i)
                    .map(|n| place_dir(&part.transform, *n, apply))
                    .unwrap_or([0.0, 0.0, 1.0]);
                for k in 0..3 {
                    lo[k] = lo[k].min(position[k]);
                    hi[k] = hi[k].max(position[k]);
                }
                vertices.push(Vertex { position, normal });
            }
            indices.extend(part.indices.iter().map(|i| base + i));
        }
        if vertices.is_empty() || indices.is_empty() {
            self.mesh = None;
            return;
        }
        let device = &state.device;
        let mesh = Mesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("strukt.preview.vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("strukt.preview.indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
            bounds: (lo, hi),
            key,
        };
        self.mesh = Some(mesh);
    }

    /// Render one frame at `size` physical pixels and return the egui texture to draw.
    pub fn render(&mut self, state: &RenderState, size: [u32; 2], clip_from_world: M4) -> egui::TextureId {
        let size = [size[0].max(1), size[1].max(1)];
        if size != self.size {
            self.resize(state, size);
        }
        let queue = &state.queue;
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                clip_from_world,
                // The design's warm key from above-front-left; the fill is baked into the shader.
                key_dir: [0.45, 0.35, 0.82, 0.0],
            }),
        );

        let view = self.colour.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("strukt.preview") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("strukt.preview.pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // The page colour, so the viewport sits in the interface rather than on it.
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.925, g: 0.921, b: 0.921, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(mesh) = &self.mesh {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
        self.texture_id
    }

    /// Rebuild the render targets and re-register the texture with egui.
    fn resize(&mut self, state: &RenderState, size: [u32; 2]) {
        let (colour, depth) = targets(&state.device, size);
        let view = colour.create_view(&wgpu::TextureViewDescriptor::default());
        state.renderer.write().update_egui_texture_from_wgpu_texture(
            &state.device,
            &view,
            wgpu::FilterMode::Linear,
            self.texture_id,
        );
        self.colour = colour;
        self.depth = depth;
        self.size = size;
    }
}

/// The colour target's format. Rgba8 unorm-srgb matches what egui expects to sample.
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn targets(device: &wgpu::Device, size: [u32; 2]) -> (wgpu::Texture, wgpu::TextureView) {
    let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
    let colour = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("strukt.preview.colour"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TARGET,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("strukt.preview.depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    (colour, depth)
}

/// Lambert with a warm key and a cool fill — the same two-light rig the rest of the project uses,
/// so a part looks here the way it looks in the engine.
const SHADER: &str = r#"
struct Uniforms {
    clip_from_world: mat4x4<f32>,
    key_dir: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> VsOut {
    var out: VsOut;
    out.clip = u.clip_from_world * vec4<f32>(position, 1.0);
    out.normal = normal;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Two-sided: these parts are authored for a renderer that culls differently, so a face turned
    // away is lit as if it were turned towards us rather than left black.
    let n = normalize(in.normal);
    let key = abs(dot(n, normalize(u.key_dir.xyz)));
    let fill = abs(dot(n, normalize(vec3<f32>(-0.4, -0.6, 0.3))));
    let lit = 0.18 + 0.72 * key + 0.22 * fill;
    let base = vec3<f32>(0.62, 0.63, 0.66);
    return vec4<f32>(base * lit, 1.0);
}
"#;
