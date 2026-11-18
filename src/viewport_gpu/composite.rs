//! Keep each viewport's rendered model and depth independent of egui overlays.

use super::*;

pub(super) struct Composite {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
}

pub(super) struct ViewportImage {
    pub(super) size: [u32; 2],
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl Composite {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport image layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport image pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport image shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport image pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            layout,
            pipeline,
            format,
        }
    }

    pub(super) fn new_image(&self, device: &wgpu::Device, size: [u32; 2]) -> ViewportImage {
        let texture_view = |label, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let color = texture_view(
            "viewport color",
            self.format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture_view(
            "viewport depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport image"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&color),
            }],
        });
        ViewportImage {
            size,
            color,
            depth,
            bind_group,
        }
    }

    pub(super) fn paint(&self, image: &ViewportImage, pass: &mut wgpu::RenderPass<'static>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &image.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl ViewportImage {
    pub(super) fn begin(&self, encoder: &mut wgpu::CommandEncoder) -> wgpu::RenderPass<'static> {
        encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport model pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            })
            .forget_lifetime()
    }
}

const COMPOSITE_SHADER: &str = r#"
@group(0) @binding(0)
var image: texture_2d<f32>;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = array(vec2(0.0, 0.0), vec2(2.0, 0.0), vec2(0.0, 2.0))[index];
    return Vertex(vec4(2.0 * uv.x - 1.0, 1.0 - 2.0 * uv.y, 0.0, 1.0), uv);
}

@fragment
fn fs(vertex: Vertex) -> @location(0) vec4<f32> {
    let size = textureDimensions(image);
    let pixel = vec2<i32>(vertex.uv * vec2<f32>(size));
    return textureLoad(image, min(pixel, vec2<i32>(size) - 1), 0);
}
"#;
