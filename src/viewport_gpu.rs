use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::{egui, egui_wgpu, wgpu};

#[cfg(test)]
pub(crate) mod readback;
#[cfg(test)]
mod tests;

const VIEWPORT_COUNT: usize = 4;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const INITIAL_BUFFER_SIZE: wgpu::BufferAddress = 4;
// Keep individual allocations bounded even on devices with very large limits.
const DISPLAY_BUFFER_CHUNK_BYTES: u64 = 64 * 1024 * 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ViewUniform {
    pub view_projection: [[f32; 4]; 4],
    pub viewport_size: [f32; 2],
    pub clip_depth: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct TriangleVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct LineInstance {
    pub start_width: [f32; 4],
    pub end_padding: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct PointInstance {
    pub position_size: [f32; 4],
    pub color: [f32; 4],
}

pub(crate) struct ViewportScene {
    pub uniform: ViewUniform,
    pub triangles: Vec<TriangleVertex>,
    pub lines: Vec<LineInstance>,
    pub points: Vec<PointInstance>,
    pub overlay_line_start: usize,
    pub overlay_point_start: usize,
    pub transparent: bool,
}

pub(crate) fn install(render_state: &egui_wgpu::RenderState) {
    let resources = ViewportRenderer::new(&render_state.device, render_state.target_format);
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resources);
}

pub(crate) fn paint(
    painter: &egui::Painter,
    rect: egui::Rect,
    viewport_index: usize,
    scene: Arc<ViewportScene>,
) {
    let callback = egui_wgpu::Callback::new_paint_callback(
        rect,
        ViewportCallback {
            viewport_index,
            scene,
        },
    );
    painter.add(callback);
}

struct ViewportCallback {
    viewport_index: usize,
    scene: Arc<ViewportScene>,
}

impl egui_wgpu::CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = callback_resources.get_mut::<ViewportRenderer>() {
            renderer.prepare_cached(self.viewport_index, &self.scene, device, queue);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = callback_resources.get::<ViewportRenderer>() {
            renderer.paint(self.viewport_index, render_pass);
        }
    }
}

struct ViewportRenderer {
    opaque_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    point_pipeline: wgpu::RenderPipeline,
    overlay_line_pipeline: wgpu::RenderPipeline,
    overlay_point_pipeline: wgpu::RenderPipeline,
    viewports: [PreparedViewport; VIEWPORT_COUNT],
}

impl ViewportRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viboceros viewport shader"),
            source: wgpu::ShaderSource::Wgsl(VIEWPORT_SHADER.into()),
        });
        let uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("viboceros viewport uniform layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ViewUniform>() as _
                        ),
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viboceros viewport pipeline layout"),
            bind_group_layouts: &[Some(&uniform_layout)],
            immediate_size: 0,
        });
        let fragment_suffix = if target_format.is_srgb() {
            "linear"
        } else {
            "gamma"
        };
        let triangle_attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4
        ];
        let triangle_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TriangleVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &triangle_attributes,
        })];
        let opaque_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros opaque surface pipeline",
            "vs_triangle",
            if fragment_suffix == "linear" {
                "fs_triangle_linear"
            } else {
                "fs_triangle_gamma"
            },
            &triangle_buffers,
            PipelineDepth::Write,
            wgpu::DepthBiasState::default(),
        );
        let transparent_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros transparent surface pipeline",
            "vs_triangle",
            if fragment_suffix == "linear" {
                "fs_triangle_linear"
            } else {
                "fs_triangle_gamma"
            },
            &triangle_buffers,
            PipelineDepth::Read,
            wgpu::DepthBiasState::default(),
        );
        let line_attributes = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4
        ];
        let line_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineInstance>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &line_attributes,
        })];
        let overlay_bias = wgpu::DepthBiasState {
            constant: -2,
            slope_scale: -1.0,
            clamp: 0.0,
        };
        let line_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros depth-tested line pipeline",
            "vs_line",
            if fragment_suffix == "linear" {
                "fs_line_linear"
            } else {
                "fs_line_gamma"
            },
            &line_buffers,
            PipelineDepth::Read,
            overlay_bias,
        );
        let point_attributes = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4
        ];
        let point_buffers = [Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointInstance>() as _,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &point_attributes,
        })];
        let point_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros depth-tested point pipeline",
            "vs_point",
            if fragment_suffix == "linear" {
                "fs_point_linear"
            } else {
                "fs_point_gamma"
            },
            &point_buffers,
            PipelineDepth::Read,
            overlay_bias,
        );

        let overlay_line_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros deformation overlay line pipeline",
            "vs_line",
            if fragment_suffix == "linear" {
                "fs_line_linear"
            } else {
                "fs_line_gamma"
            },
            &line_buffers,
            PipelineDepth::Overlay,
            wgpu::DepthBiasState::default(),
        );
        let overlay_point_pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            target_format,
            "viboceros deformation overlay point pipeline",
            "vs_point",
            if fragment_suffix == "linear" {
                "fs_point_linear"
            } else {
                "fs_point_gamma"
            },
            &point_buffers,
            PipelineDepth::Overlay,
            wgpu::DepthBiasState::default(),
        );

        Self {
            opaque_pipeline,
            transparent_pipeline,
            line_pipeline,
            point_pipeline,
            overlay_line_pipeline,
            overlay_point_pipeline,
            viewports: std::array::from_fn(|_| PreparedViewport::new(device, &uniform_layout)),
        }
    }

    fn prepare_cached(
        &mut self,
        viewport_index: usize,
        scene: &Arc<ViewportScene>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let Some(viewport) = self.viewports.get(viewport_index) else {
            return;
        };
        if viewport
            .scene
            .as_ref()
            .is_some_and(|previous| Arc::ptr_eq(previous, scene))
        {
            return;
        }
        self.prepare(viewport_index, scene, device, queue);
        self.viewports[viewport_index].scene = Some(Arc::clone(scene));
    }

    fn prepare(
        &mut self,
        viewport_index: usize,
        scene: &ViewportScene,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let Some(viewport) = self.viewports.get_mut(viewport_index) else {
            return;
        };
        #[cfg(test)]
        {
            viewport.preparations += 1;
        }
        viewport.scene = None;
        queue.write_buffer(
            &viewport.uniform_buffer,
            0,
            bytemuck::bytes_of(&scene.uniform),
        );
        viewport
            .triangle_buffer
            .upload(device, queue, &scene.triangles);
        viewport.line_buffer.upload(device, queue, &scene.lines);
        viewport.point_buffer.upload(device, queue, &scene.points);
        viewport.overlay_line_start = scene.overlay_line_start.min(scene.lines.len());
        viewport.overlay_point_start = scene.overlay_point_start.min(scene.points.len());
        viewport.transparent = scene.transparent;
    }

    fn paint(&self, viewport_index: usize, render_pass: &mut wgpu::RenderPass<'static>) {
        let Some(viewport) = self.viewports.get(viewport_index) else {
            return;
        };
        render_pass.set_bind_group(0, &viewport.uniform_bind_group, &[]);
        if !viewport.triangle_buffer.chunks.is_empty() {
            render_pass.set_pipeline(if viewport.transparent {
                &self.transparent_pipeline
            } else {
                &self.opaque_pipeline
            });
            viewport
                .triangle_buffer
                .draw(render_pass, 0..usize::MAX, false);
        }
        render_pass.set_pipeline(&self.line_pipeline);
        viewport
            .line_buffer
            .draw(render_pass, 0..viewport.overlay_line_start, true);
        render_pass.set_pipeline(&self.point_pipeline);
        viewport
            .point_buffer
            .draw(render_pass, 0..viewport.overlay_point_start, true);
        render_pass.set_pipeline(&self.overlay_line_pipeline);
        viewport
            .line_buffer
            .draw(render_pass, viewport.overlay_line_start..usize::MAX, true);
        render_pass.set_pipeline(&self.overlay_point_pipeline);
        viewport
            .point_buffer
            .draw(render_pass, viewport.overlay_point_start..usize::MAX, true);
    }
}

struct PreparedViewport {
    #[cfg(test)]
    preparations: usize,
    scene: Option<Arc<ViewportScene>>,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    triangle_buffer: ChunkedBuffer,
    line_buffer: ChunkedBuffer,
    point_buffer: ChunkedBuffer,
    overlay_line_start: usize,
    overlay_point_start: usize,
    transparent: bool,
}

impl PreparedViewport {
    fn new(device: &wgpu::Device, uniform_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viboceros viewport uniform buffer"),
            size: std::mem::size_of::<ViewUniform>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viboceros viewport uniform bind group"),
            layout: uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        Self {
            #[cfg(test)]
            preparations: 0,
            scene: None,
            uniform_buffer,
            uniform_bind_group,
            triangle_buffer: ChunkedBuffer::new("viboceros triangle vertex buffer", 3),
            line_buffer: ChunkedBuffer::new("viboceros line instance buffer", 1),
            point_buffer: ChunkedBuffer::new("viboceros point instance buffer", 1),
            overlay_line_start: 0,
            overlay_point_start: 0,
            transparent: false,
        }
    }
}

/// Chunks preserve complete primitives and their ordering. Overlay boundaries
/// are applied inside each chunk, so depth-tested and overlay instances never mix.
struct ChunkedBuffer {
    chunks: Vec<(GrowingBuffer, usize)>,
    label: &'static str,
    primitive_elements: usize,
}

impl ChunkedBuffer {
    fn new(label: &'static str, primitive_elements: usize) -> Self {
        Self {
            chunks: Vec::new(),
            label,
            primitive_elements,
        }
    }

    fn upload<T: Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, values: &[T]) {
        let per_chunk = chunk_elements(
            device
                .limits()
                .max_buffer_size
                .min(DISPLAY_BUFFER_CHUNK_BYTES),
            std::mem::size_of::<T>(),
            self.primitive_elements,
        );
        // wgpu guarantees enough space for these tiny vertex records. This also
        // avoids making an invalid allocation if a future backend breaks that contract.
        if per_chunk == 0 {
            eprintln!(
                "Cannot display {}: device buffer limit cannot hold one primitive",
                self.label
            );
            self.chunks.clear();
            return;
        }
        let chunk_count = values.len().div_ceil(per_chunk);
        self.chunks.truncate(chunk_count);
        for (index, values) in values.chunks(per_chunk).enumerate() {
            if index == self.chunks.len() {
                self.chunks
                    .push((GrowingBuffer::new(device, self.label), 0));
            }
            let (buffer, count) = &mut self.chunks[index];
            buffer.upload(device, queue, bytemuck::cast_slice(values));
            *count = values.len();
        }
    }

    fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'static>,
        range: std::ops::Range<usize>,
        instanced: bool,
    ) {
        let mut offset = 0;
        for (buffer, count) in &self.chunks {
            let start = range.start.saturating_sub(offset).min(*count) as u32;
            let end = range.end.saturating_sub(offset).min(*count) as u32;
            if start < end {
                pass.set_vertex_buffer(0, buffer.buffer.slice(..));
                if instanced {
                    pass.draw(0..6, start..end);
                } else {
                    pass.draw(start..end, 0..1);
                }
            }
            offset += count;
        }
    }
}

fn chunk_elements(limit: u64, element_size: usize, primitive_elements: usize) -> usize {
    let count = (limit / element_size as u64).min(u32::MAX as u64) as usize;
    count / primitive_elements * primitive_elements
}

fn buffer_capacity(required: u64, limit: u64) -> u64 {
    debug_assert!(required <= limit);
    required
        .checked_next_power_of_two()
        .unwrap_or(required)
        .min(limit)
}

struct GrowingBuffer {
    buffer: wgpu::Buffer,
    capacity: wgpu::BufferAddress,
    label: &'static str,
}

impl GrowingBuffer {
    fn new(device: &wgpu::Device, label: &'static str) -> Self {
        Self {
            buffer: create_vertex_buffer(device, label, INITIAL_BUFFER_SIZE),
            capacity: INITIAL_BUFFER_SIZE,
            label,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let required = bytes.len() as wgpu::BufferAddress;
        if required > self.capacity {
            self.capacity = buffer_capacity(required, device.limits().max_buffer_size);
            self.buffer = create_vertex_buffer(device, self.label, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytes);
    }
}

fn create_vertex_buffer(
    device: &wgpu::Device,
    label: &'static str,
    size: wgpu::BufferAddress,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

enum PipelineDepth {
    Write,
    Read,
    Overlay,
}

#[allow(clippy::too_many_arguments)]
fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    label: &'static str,
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    depth: PipelineDepth,
    bias: wgpu::DepthBiasState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(matches!(depth, PipelineDepth::Write)),
            depth_compare: Some(if matches!(depth, PipelineDepth::Overlay) {
                wgpu::CompareFunction::Always
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: wgpu::StencilState::default(),
            bias,
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

const VIEWPORT_SHADER: &str = r#"
struct ViewUniform {
    view_projection: mat4x4<f32>,
    viewport_size: vec2<f32>,
    clip_depth: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> view: ViewUniform;

fn discard_clipped(depth: f32) {
    if depth < view.clip_depth.x || depth > view.clip_depth.y {
        discard;
    }
}

fn srgb_to_linear(value: vec3<f32>) -> vec3<f32> {
    let low = value / 12.92;
    let high = pow((value + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, value <= vec3<f32>(0.04045));
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let low = value * 12.92;
    let high = 1.055 * pow(value, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(high, low, value <= vec3<f32>(0.0031308));
}

struct TriangleInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct TriangleOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_triangle(input: TriangleInput) -> TriangleOutput {
    var output: TriangleOutput;
    output.position = view.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

fn shaded_triangle(input: TriangleOutput) -> vec4<f32> {
    let normal = normalize(input.normal + vec3<f32>(0.0, 0.0, 1.0e-20));
    let light = normalize(vec3<f32>(-0.35, -0.45, 0.82));
    let illumination = clamp(abs(dot(normal, light)), 0.0, 1.0);
    let whiten = 0.35 + 0.55 * illumination;
    return vec4<f32>(mix(input.color.rgb, vec3<f32>(1.0), whiten), input.color.a);
}

@fragment
fn fs_triangle_linear(input: TriangleOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    let color = shaded_triangle(input);
    return vec4<f32>(srgb_to_linear(color.rgb) * color.a, color.a);
}

@fragment
fn fs_triangle_gamma(input: TriangleOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    let color = shaded_triangle(input);
    return vec4<f32>(color.rgb * color.a, color.a);
}

struct LineInput {
    @location(0) start_width: vec4<f32>,
    @location(1) end_padding: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct FlatOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(input: LineInput, @builtin(vertex_index) vertex_index: u32) -> FlatOutput {
    let endpoint_indices = array<u32, 6>(0u, 0u, 1u, 0u, 1u, 1u);
    let sides = array<f32, 6>(-1.0, 1.0, 1.0, -1.0, 1.0, -1.0);
    let start_clip = view.view_projection * vec4<f32>(input.start_width.xyz, 1.0);
    let end_clip = view.view_projection * vec4<f32>(input.end_padding.xyz, 1.0);
    let start_ndc = start_clip.xy / start_clip.w;
    let end_ndc = end_clip.xy / end_clip.w;
    let delta_pixels = vec2<f32>(
        (end_ndc.x - start_ndc.x) * view.viewport_size.x * 0.5,
        -(end_ndc.y - start_ndc.y) * view.viewport_size.y * 0.5,
    );
    let safe_length = max(length(delta_pixels), 1.0e-6);
    let perpendicular_pixels = vec2<f32>(-delta_pixels.y, delta_pixels.x) / safe_length;
    let half_width = max(input.start_width.w, 1.0) * 0.5;
    let offset_pixels = perpendicular_pixels * half_width * sides[vertex_index];
    let offset_ndc = vec2<f32>(
        offset_pixels.x * 2.0 / view.viewport_size.x,
        -offset_pixels.y * 2.0 / view.viewport_size.y,
    );
    var clip = select(start_clip, end_clip, endpoint_indices[vertex_index] == 1u);
    clip.x += offset_ndc.x * clip.w;
    clip.y += offset_ndc.y * clip.w;
    var output: FlatOutput;
    output.position = clip;
    output.color = input.color;
    return output;
}

fn flat_linear_color(input: FlatOutput) -> vec4<f32> {
    return vec4<f32>(srgb_to_linear(input.color.rgb), input.color.a);
}

@fragment
fn fs_line_linear(input: FlatOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    let color = flat_linear_color(input);
    return vec4<f32>(color.rgb * color.a, color.a);
}

@fragment
fn fs_line_gamma(input: FlatOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    let color = flat_linear_color(input);
    return vec4<f32>(linear_to_srgb(color.rgb) * color.a, color.a);
}

struct PointInput {
    @location(0) position_size: vec4<f32>,
    @location(1) color: vec4<f32>,
};

struct PointOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_position: vec2<f32>,
};

@vertex
fn vs_point(input: PointInput, @builtin(vertex_index) vertex_index: u32) -> PointOutput {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vertex_index];
    var clip = view.view_projection * vec4<f32>(input.position_size.xyz, 1.0);
    let radius = max(input.position_size.w, 1.0);
    clip.x += corner.x * radius * 2.0 / view.viewport_size.x * clip.w;
    clip.y -= corner.y * radius * 2.0 / view.viewport_size.y * clip.w;
    var output: PointOutput;
    output.position = clip;
    output.color = input.color;
    output.local_position = corner;
    return output;
}

@fragment
fn fs_point_linear(input: PointOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    if dot(input.local_position, input.local_position) > 1.0 {
        discard;
    }
    let linear = srgb_to_linear(input.color.rgb);
    return vec4<f32>(linear * input.color.a, input.color.a);
}

@fragment
fn fs_point_gamma(input: PointOutput) -> @location(0) vec4<f32> {
    discard_clipped(input.position.z);
    if dot(input.local_position, input.local_position) > 1.0 {
        discard;
    }
    return vec4<f32>(input.color.rgb * input.color.a, input.color.a);
}
"#;
