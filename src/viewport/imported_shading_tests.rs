//! Optional end-to-end check for a local 3DM; no private model is checked in.
use super::*;
use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};

#[test]
#[ignore = "requires VIBOCEROS_3DM_FIXTURE and a graphics adapter"]
fn imported_surfaces_produce_shaded_pixels() {
    let path =
        std::env::var("VIBOCEROS_3DM_FIXTURE").expect("set VIBOCEROS_3DM_FIXTURE to a local 3DM");
    assert!(!path.contains('"'));
    let mut document = Document::default();
    viboceros_command::CommandRegistry::with_builtins()
        .execute(&mut document, &format!("Import3dm \"{path}\""))
        .unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let mut renderer = OffscreenRenderer::new(eframe::wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut checked = 0;
    for (index, object) in document.objects().enumerate() {
        if !matches!(
            object.geometry(),
            Geometry::Brep(_) | Geometry::NurbsSurface(_)
        ) {
            continue;
        }
        let display = display_cache::DisplayGeometry::new(
            object.geometry_snapshot().clone(),
            object.attributes().wire_density(),
            document.tolerance(),
        );
        let mesh = display
            .mesh()
            .unwrap_or_else(|| panic!("object {index}: missing shading mesh"));
        assert!(!mesh.triangles().is_empty(), "object {index}");
        let mut isolated = Document::default();
        isolated.add_geometry(object.geometry().clone()).unwrap();
        let mut view = Viewport::new(ViewKind::Perspective);
        view.last_rect = Some(rect);
        view.display_mode = DisplayMode::Shaded;
        view.zoom_extents(&isolated).unwrap();
        let mut scene = GpuSceneBuilder::new();
        view.add_gpu_mesh_faces(&mut scene, mesh, Color32::from_gray(180));
        let pixels = renderer.render(&scene.finish(&view, rect, false));
        assert!(
            pixels.iter().any(|p| p[3] != 0),
            "object {index}: no filled pixels"
        );
        if let Ok(directory) = std::env::var("VIBOCEROS_SHADING_IMAGES") {
            let mut bytes = format!("P6\n{SIZE} {SIZE}\n255\n").into_bytes();
            for p in pixels {
                bytes.extend_from_slice(if p[3] == 0 { &[225, 230, 238] } else { &p[..3] });
            }
            std::fs::write(
                std::path::Path::new(&directory).join(format!("object-{index}.ppm")),
                bytes,
            )
            .unwrap();
        }
        checked += 1;
    }
    assert!(checked > 0);
    eprintln!("{checked} imported surface/B-rep objects produced filled GPU pixels");
}
