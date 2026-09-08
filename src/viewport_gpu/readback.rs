//! Opt-in integration-test support using the production pipelines without a window.

use super::*;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

pub(crate) const SIZE: u32 = 256;
static GPU_TEST_CONTEXT: Mutex<()> = Mutex::new(());

pub(crate) struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: ViewportRenderer,
    format: wgpu::TextureFormat,
    // Drop this last, after all graphics resources. Concurrent context tests
    // have triggered a native crash; keep this harness's lifetimes disjoint.
    _serial: MutexGuard<'static, ()>,
}

impl OffscreenRenderer {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        let serial = GPU_TEST_CONTEXT.lock().expect("GPU test context lock");
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("the opt-in GPU test requires an available graphics adapter");
        eprintln!("Offscreen viewport adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))
            .expect("create offscreen test device");
        let renderer = ViewportRenderer::new(&device, format);
        Self {
            device,
            queue,
            renderer,
            format,
            _serial: serial,
        }
    }

    pub(crate) fn render(&mut self, scene: &ViewportScene) -> Vec<[u8; 4]> {
        assert_eq!(scene.uniform.viewport_size, [SIZE as f32; 2]);
        self.renderer.prepare(0, scene, &self.device, &self.queue);
        let extent = wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport test color"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport test depth"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = texture.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport test readback"),
            size: u64::from(SIZE * SIZE * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("viewport test pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.paint(0, &mut pass);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 4),
                    rows_per_image: Some(SIZE),
                },
            },
            extent,
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap();
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .expect("complete offscreen render");
        receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("readback callback")
            .expect("map pixels");
        let pixels = readback
            .slice(..)
            .get_mapped_range()
            .expect("read mapped pixels");
        let result = pixels
            .chunks_exact(4)
            .map(|pixel| pixel.try_into().unwrap())
            .collect();
        drop(pixels);
        readback.unmap();
        result
    }
}
