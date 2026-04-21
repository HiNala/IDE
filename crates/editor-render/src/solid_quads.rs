//! Translucent axis-aligned quads (selection + diff under text).

use bytemuck::{Pod, Zeroable};
use wgpu::{
    BlendState, Buffer, ColorTargetState, ColorWrites, Device, FragmentState, MultisampleState,
    PipelineCompilationOptions, PrimitiveState, Queue, RenderPass, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, TextureFormat, VertexAttribute,
    VertexBufferLayout, VertexState,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ColoredVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

const MAX_RECTS: usize = 4096;

pub struct SolidQuadLayer {
    pipeline: RenderPipeline,
    vertex_buffer: Buffer,
    vertex_count: u32,
    verts_scratch: Vec<ColoredVertex>,
}

impl SolidQuadLayer {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("solid-quad-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/solid_quad.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("solid-quad-pl"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("solid-quad-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[ColoredVertex::layout()],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("solid-quad-vb"),
            size: (MAX_RECTS * 6 * std::mem::size_of::<ColoredVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, vertex_buffer, vertex_count: 0, verts_scratch: Vec::new() }
    }

    /// Pixel rects `(left, top, right, bottom, premultiplied-ish RGBA)`.
    pub fn prepare(
        &mut self,
        _device: &Device,
        queue: &Queue,
        viewport_w: u32,
        viewport_h: u32,
        rects: &[(f32, f32, f32, f32, [f32; 4])],
    ) {
        let vw = viewport_w.max(1) as f32;
        let vh = viewport_h.max(1) as f32;
        self.verts_scratch.clear();
        self.verts_scratch.reserve(rects.len().saturating_mul(6));
        for &(left, top, right, bottom, c) in rects {
            if right <= left || bottom <= top {
                continue;
            }
            let l = left / vw * 2.0 - 1.0;
            let r = right / vw * 2.0 - 1.0;
            let t_ndc = 1.0 - (top / vh) * 2.0;
            let b_ndc = 1.0 - (bottom / vh) * 2.0;
            self.verts_scratch.push(ColoredVertex { pos: [l, t_ndc], color: c });
            self.verts_scratch.push(ColoredVertex { pos: [r, t_ndc], color: c });
            self.verts_scratch.push(ColoredVertex { pos: [l, b_ndc], color: c });
            self.verts_scratch.push(ColoredVertex { pos: [r, t_ndc], color: c });
            self.verts_scratch.push(ColoredVertex { pos: [r, b_ndc], color: c });
            self.verts_scratch.push(ColoredVertex { pos: [l, b_ndc], color: c });
        }
        self.vertex_count = self.verts_scratch.len() as u32;
        if self.verts_scratch.is_empty() {
            return;
        }
        if self.verts_scratch.len() > MAX_RECTS * 6 {
            self.verts_scratch.truncate(MAX_RECTS * 6);
            self.vertex_count = self.verts_scratch.len() as u32;
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.verts_scratch));
    }

    pub fn render<'a>(&'a self, pass: &mut RenderPass<'a>) {
        if self.vertex_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }
}

impl ColoredVertex {
    const ATTRS: [VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x4,
    ];

    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<ColoredVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}
