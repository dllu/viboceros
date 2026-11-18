//! Offscreen capture, with a temporary camera and display settings.
use super::*;
use crate::viewport::ViewKind;
use std::path::PathBuf;
use viboceros_geometry::Vector3;

#[derive(Deserialize)]
pub(super) struct Screenshot {
    path: PathBuf,
    view: Option<ViewRef>,
    camera: Option<Camera>,
    mode: String,
    size: [u32; 2],
    fit: bool,
    background: [u8; 4],
    #[serde(default = "show_edges")]
    show_edges: bool,
    #[serde(default)]
    lighting: Lighting,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Lighting {
    #[default]
    Viewport,
    Diffuse,
}

fn show_edges() -> bool {
    true
}

#[derive(Deserialize)]
struct Camera {
    eye: [f64; 3],
    target: [f64; 3],
    up: [f64; 3],
    vertical_fov_degrees: f64,
}

impl Session {
    pub(super) fn screenshot(&mut self, options: Screenshot) -> Result<()> {
        let mut viewport = if let Some(camera) = options.camera {
            if options.view.is_some() {
                return Err("specify a viewport or a custom camera".into());
            }
            let source = &self.app.viewports[self.app.active_viewport];
            let mut view = Viewport::new_for_layout(source, ViewKind::Perspective);
            view.capture_camera(
                Point3::try_from(camera.eye)?,
                Point3::try_from(camera.target)?,
                Vector3::try_from(camera.up)?,
                camera.vertical_fov_degrees,
            )?;
            view
        } else {
            self.capture_view(options.view.as_ref())?
        };
        viewport.display_mode = DisplayMode::parse(&options.mode).ok_or("unknown display mode")?;
        viewport.draw_surface_edges = options.show_edges;
        viewport.diffuse_lighting = matches!(options.lighting, Lighting::Diffuse);
        let [width, height] = options.size;
        let rect =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32));
        let scene = viewport.capture_scene(
            rect,
            &self.app.document,
            options.fit,
            self.app.zoom_extents_borders,
        )?;
        if self.renderer.is_none() {
            self.renderer = Some(OffscreenRenderer::try_new()?);
        }
        let rgba =
            self.renderer
                .as_mut()
                .unwrap()
                .capture(&scene, options.size, options.background)?;
        let file = io::BufWriter::new(std::fs::File::create(options.path)?);
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&rgba)?;
        writer.finish()?;
        Ok(())
    }
}
