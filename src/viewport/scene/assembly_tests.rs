//! Opt-in regression against an external assembly; no user model is committed.
use super::*;
use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};

#[test]
#[ignore = "set VIBOCEROS_ASSEMBLY_3DM to an assembly; requires a graphics adapter"]
fn large_native_assembly_repeated_display_switches() {
    let path = std::env::var("VIBOCEROS_ASSEMBLY_3DM").expect("VIBOCEROS_ASSEMBLY_3DM");
    let mut document = Document::default();
    viboceros_command::CommandRegistry::with_builtins()
        .execute(&mut document, &format!("Open \"{path}\""))
        .unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let mut view = Viewport::new(ViewKind::Perspective);
    view.last_rect = Some(rect);
    view.zoom_extents(&document, ZoomExtentsBorders::default())
        .unwrap();
    let mut renderer = OffscreenRenderer::new(eframe::wgpu::TextureFormat::Rgba8Unorm);
    for pass in 0..3 {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            view.display_mode = mode;
            let scene = view.object_scene(rect, &document);
            eprintln!(
                "assembly pass {pass} mode {mode:?}: {} objects; {} triangle vertices ({} bytes), {} lines",
                document.objects().len(),
                scene.triangles.len(),
                scene.triangles.len() * std::mem::size_of::<GpuTriangleVertex>(),
                scene.lines.len()
            );
            let pixels = renderer.render_cached(&scene);
            assert!(
                pixels.iter().any(|p| p[3] != 0),
                "assembly must remain visible"
            );
        }
    }
}
