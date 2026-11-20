//! Render the viewport pipelines to an RGBA image without a window or surface.

use super::*;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[cfg(test)]
pub(crate) const SIZE: u32 = 256;
#[cfg(test)]
static GPU_TEST_CONTEXT: Mutex<()> = Mutex::new(());
const READBACK_TIMEOUT: Duration = Duration::from_secs(30);

type RenderError = Box<dyn std::error::Error>;

pub(crate) struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: ViewportRenderer,
    multisampled_renderer: Option<ViewportRenderer>,
    supports_multisampling: bool,
    format: wgpu::TextureFormat,
    // Serialize test contexts until all graphics resources have been dropped.
    #[cfg(test)]
    serial: Option<MutexGuard<'static, ()>>,
}

impl OffscreenRenderer {
    pub(crate) fn try_new() -> Result<Self, RenderError> {
        Self::create(wgpu::TextureFormat::Rgba8Unorm, None)
    }

    fn create(format: wgpu::TextureFormat, limit: Option<u64>) -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))?;
        let flags = |format| adapter.get_texture_format_features(format).flags;
        let supports_multisampling = flags(format).contains(
            wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4
                | wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE,
        ) && flags(DEPTH_FORMAT)
            .contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4);
        let mut descriptor = wgpu::DeviceDescriptor::default();
        if let Some(limit) = limit {
            descriptor.required_limits.max_buffer_size = limit;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))?;
        let renderer = ViewportRenderer::new(&device, format);
        Ok(Self {
            device,
            queue,
            renderer,
            multisampled_renderer: None,
            supports_multisampling,
            format,
            #[cfg(test)]
            serial: None,
        })
    }

    pub(crate) fn capture(
        &mut self,
        scene: &ViewportScene,
        size: [u32; 2],
        background: [u8; 4],
        sample_count: u32,
    ) -> Result<Vec<u8>, RenderError> {
        let [r, g, b, a] = background.map(|channel| f64::from(channel) / 255.0);
        let clear = wgpu::Color {
            r: r * a,
            g: g * a,
            b: b * a,
            a,
        };
        let mut rgba = self.read_pixels(0, scene, size, clear, sample_count)?;
        if a < 1.0 {
            // The viewport compositor stores premultiplied color. PNG uses straight alpha.
            for pixel in rgba.chunks_exact_mut(4) {
                let alpha = u16::from(pixel[3]);
                if (1..255).contains(&alpha) {
                    for channel in &mut pixel[..3] {
                        *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                    }
                }
            }
        }
        Ok(rgba)
    }

    fn read_pixels(
        &mut self,
        index: usize,
        scene: &ViewportScene,
        [width, height]: [u32; 2],
        background: wgpu::Color,
        sample_count: u32,
    ) -> Result<Vec<u8>, RenderError> {
        if !matches!(sample_count, 1 | 4) {
            return Err("capture MSAA samples must be 1 or 4".into());
        }
        if sample_count == 4 && !self.supports_multisampling {
            return Err("the capture GPU does not support 4× MSAA for the viewport".into());
        }
        let limits = self.device.limits();
        if width == 0 || height == 0 || width.max(height) > limits.max_texture_dimension_2d {
            return Err(format!(
                "capture dimensions must be between 1 and {} pixels",
                limits.max_texture_dimension_2d
            )
            .into());
        }
        let row_bytes = width * 4;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row_bytes = row_bytes.div_ceil(alignment) * alignment;
        let buffer_size = u64::from(padded_row_bytes) * u64::from(height);
        if buffer_size > limits.max_buffer_size {
            return Err("capture exceeds the GPU readback buffer limit".into());
        }
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen viewport color"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = texture.create_view(&Default::default());
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen viewport readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let renderer = if sample_count == 4 {
            self.multisampled_renderer.get_or_insert_with(|| {
                ViewportRenderer::with_sample_count(&self.device, self.format, sample_count)
            })
        } else {
            &mut self.renderer
        };
        renderer.prepare_image(
            index,
            scene,
            [width, height],
            &self.device,
            &self.queue,
            &mut encoder,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("offscreen viewport pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(background),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            if let Some(image) = &renderer.viewports[index].image {
                renderer.composite.paint(image, &mut pass);
            }
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(READBACK_TIMEOUT),
        })?;
        receiver.recv_timeout(READBACK_TIMEOUT)??;
        let pixels = slice.get_mapped_range()?;
        let mut rgba = Vec::with_capacity(row_bytes as usize * height as usize);
        for row in pixels.chunks_exact(padded_row_bytes as usize) {
            rgba.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(pixels);
        readback.unmap();
        Ok(rgba)
    }
}

#[cfg(test)]
impl OffscreenRenderer {
    pub(crate) fn new(format: wgpu::TextureFormat) -> Self {
        Self::with_buffer_limit(format, None)
    }

    pub(crate) fn with_buffer_limit(format: wgpu::TextureFormat, limit: Option<u64>) -> Self {
        let serial = GPU_TEST_CONTEXT.lock().expect("GPU test context lock");
        let mut renderer = Self::create(format, limit).expect("create offscreen test device");
        renderer.serial = Some(serial);
        renderer
    }

    pub(crate) fn render(&mut self, scene: &ViewportScene) -> Vec<[u8; 4]> {
        self.render_at(0, scene)
    }

    pub(crate) fn render_at(&mut self, index: usize, scene: &ViewportScene) -> Vec<[u8; 4]> {
        assert_eq!(scene.uniform.viewport_size, [SIZE as f32; 2]);
        self.read_pixels(index, scene, [SIZE; 2], wgpu::Color::TRANSPARENT, 1)
            .expect("render offscreen test pixels")
            .chunks_exact(4)
            .map(|pixel| pixel.try_into().unwrap())
            .collect()
    }

    pub(crate) fn render_cached(&mut self, scene: &Arc<ViewportScene>) -> Vec<[u8; 4]> {
        self.render(scene)
    }

    pub(crate) fn preparations(&self) -> usize {
        self.renderer.viewports[0].preparations
    }
}
