use super::*;
use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
use eframe::wgpu;

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_depth_and_ghosted_compositing_ignore_object_insertion_order() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let center = (SIZE / 2 * SIZE + SIZE / 2) as usize;
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut viewport = Viewport::new(kind);
            let (right, up, forward) = match kind {
                ViewKind::Top => (NaVector3::x(), NaVector3::y(), -NaVector3::z()),
                ViewKind::Front => (NaVector3::x(), NaVector3::z(), NaVector3::y()),
                ViewKind::Right => (NaVector3::y(), NaVector3::z(), -NaVector3::x()),
                ViewKind::Perspective => viewport.perspective_basis(),
            };
            let origin = if kind.is_parallel() {
                NaVector3::zeros()
            } else {
                -forward * viewport.perspective_camera_distance
            };
            let point = |x: Real, y: Real, depth: Real| {
                let v = origin + right * x + up * y + forward * depth;
                Point3::try_new(v.x, v.y, v.z).unwrap()
            };
            let mesh = |depth| {
                TriangleMesh::try_new(
                    vec![
                        point(-3.0, -2.0, depth),
                        point(3.0, -2.0, depth),
                        point(0.0, 3.0, depth),
                    ],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap()
            };
            let front = mesh(10.0);
            let back = mesh(20.0);
            for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
                viewport.display_mode = mode;
                let mut render = |scene: GpuSceneBuilder| {
                    let uniform = viewport.gpu_view_uniform(rect, scene.depth_range());
                    renderer.render(&scene.finish(uniform, mode == DisplayMode::Ghosted))[center]
                };
                let mut baseline = GpuSceneBuilder::new();
                viewport.add_gpu_mesh_faces(&mut baseline, &front, Color32::RED);
                let baseline = render(baseline);
                assert!(baseline[0] > baseline[2]);
                let mut ordered_pixel = None;
                for reverse in [false, true] {
                    let mut scene = GpuSceneBuilder::new();
                    let mut faces = [(&front, Color32::RED), (&back, Color32::BLUE)];
                    if reverse {
                        faces.reverse();
                    }
                    for (mesh, color) in faces {
                        viewport.add_gpu_mesh_faces(&mut scene, mesh, color);
                    }
                    let pixel = render(scene);
                    if let Some(expected) = ordered_pixel {
                        assert_eq!(pixel, expected, "{kind:?} {format:?} {mode:?}");
                    }
                    ordered_pixel = Some(pixel);
                    if mode == DisplayMode::Shaded {
                        assert_eq!(pixel, baseline, "back face must be occluded in {kind:?}");
                    } else {
                        // Two layers of alpha 35/255 combined with source-over.
                        assert!((i16::from(pixel[3]) - 65).abs() <= 1);
                        assert!(
                            pixel[0] > pixel[2],
                            "near red face must composite after far blue face"
                        );
                        assert_ne!(pixel, baseline);
                    }
                }
                for depth in [5.0, 20.0] {
                    let mut scene = GpuSceneBuilder::new();
                    viewport.add_gpu_mesh_faces(&mut scene, &front, Color32::RED);
                    viewport.add_gpu_line(
                        &mut scene,
                        rect,
                        point(-1.0, 0.0, depth),
                        point(1.0, 0.0, depth),
                        3.0,
                        Color32::BLACK,
                    );
                    let pixel = render(scene);
                    if mode == DisplayMode::Shaded && depth > 10.0 {
                        assert_eq!(pixel, baseline, "back wire must be occluded in {kind:?}");
                    } else {
                        assert_eq!(pixel, [0, 0, 0, 255], "visible wire in {kind:?} {mode:?}");
                    }
                }
            }
        }
    }
}

/// Independent ray/triangle intersection, not a projection or clipping helper.
/// Return ray distance and barycentric weights; leave edge exclusion to callers.
fn ray_triangle(
    origin: NaVector3<Real>,
    direction: NaVector3<Real>,
    points: [Point3; 3],
) -> Option<[Real; 4]> {
    let [a, b, c] = points.map(|p| NaVector3::from(p.to_array()));
    let ab = b - a;
    let ac = c - a;
    let p = direction.cross(&ac);
    let determinant = ab.dot(&p);
    if determinant.abs() < 1e-12 {
        return None;
    }
    let offset = origin - a;
    let u = offset.dot(&p) / determinant;
    let q = offset.cross(&ab);
    let v = direction.dot(&q) / determinant;
    Some([ac.dot(&q) / determinant, 1.0 - u - v, u, v])
}

#[test]
fn independent_ray_reference_handles_winding_misses_and_unnormalized_directions() {
    let points =
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].map(|p| Point3::try_from(p).unwrap());
    let origin = NaVector3::new(0.25, 0.25, 1.0);
    let direction = NaVector3::new(0.0, 0.0, -2.0);
    assert_eq!(
        ray_triangle(origin, direction, points),
        Some([0.5, 0.5, 0.25, 0.25])
    );
    assert_eq!(
        ray_triangle(origin, direction, [points[2], points[1], points[0]]),
        Some([0.5, 0.25, 0.25, 0.5])
    );
    assert_eq!(
        ray_triangle(origin, -direction, points),
        Some([-0.5, 0.5, 0.25, 0.25])
    );
    assert_eq!(
        ray_triangle(NaVector3::new(2.0, 2.0, 1.0), direction, points),
        Some([0.5, -3.0, 2.0, 2.0])
    );
    assert!(ray_triangle(origin, NaVector3::x(), points).is_none());
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_camera_crossing_faces_match_independent_ray_coverage() {
    let mut viewport = Viewport::new(ViewKind::Perspective);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let (right, up, forward) = viewport.perspective_basis();
    let camera = viewport.target - forward * viewport.perspective_camera_distance;
    let focal = viewport.perspective_focal_length_pixels(rect);
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for depths in [
            [10.0; 3],
            [-10.0, 10.0, 10.0],
            [-10.0, -10.0, 10.0],
            [-10.0; 3],
        ] {
            let points = [(-3.0, -2.0), (3.0, -2.0), (0.0, 3.0)];
            let points: [Point3; 3] = std::array::from_fn(|i| {
                let v = camera + right * points[i].0 + up * points[i].1 + forward * depths[i];
                Point3::try_new(v.x, v.y, v.z).unwrap()
            });
            for indices in [[0, 1, 2], [2, 1, 0]] {
                let points = indices.map(|i| points[i]);
                let mesh =
                    TriangleMesh::try_new(points.to_vec(), vec![[0, 1, 2]], Tolerance::DEFAULT)
                        .unwrap();
                for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
                    viewport.display_mode = mode;
                    let mut scene = GpuSceneBuilder::new();
                    viewport.add_gpu_mesh_faces(&mut scene, &mesh, Color32::GRAY);
                    let uniform = viewport.gpu_view_uniform(rect, scene.depth_range());
                    let pixels =
                        renderer.render(&scene.finish(uniform, mode == DisplayMode::Ghosted));
                    let mut covered = 0;
                    for y in (0..SIZE).step_by(4) {
                        for x in (0..SIZE).step_by(4) {
                            let direction = forward
                                + right * ((Real::from(x) + 0.5 - Real::from(SIZE) / 2.0) / focal)
                                - up * ((Real::from(y) + 0.5 - Real::from(SIZE) / 2.0) / focal);
                            let Some([distance, a, b, c]) = ray_triangle(camera, direction, points)
                            else {
                                continue;
                            };
                            let barycentric_minimum = a.min(b).min(c);
                            // Do not specify rasterizer edge tie-breaking or subpixel coverage.
                            if distance > 0.0 && barycentric_minimum.abs() < 0.02 {
                                continue;
                            }
                            let expected = distance > 0.0 && barycentric_minimum > 0.0;
                            let actual = pixels[(y * SIZE + x) as usize][3] > 0;
                            assert_eq!(
                                actual, expected,
                                "{format:?} {mode:?} depths={depths:?} indices={indices:?} pixel=({x},{y})"
                            );
                            covered += usize::from(actual);
                        }
                    }
                    assert_eq!(covered > 0, depths.iter().any(|depth| *depth > 0.0));
                }
            }
        }
    }
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_camera_crossing_wires_rasterize_in_both_endpoint_orders() {
    let viewport = Viewport::new(ViewKind::Perspective);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let (right, _, forward) = viewport.perspective_basis();
    let camera = viewport.target - forward * viewport.perspective_camera_distance;
    let point = |depth: Real, x: Real| {
        let v = camera + forward * depth + right * x;
        Point3::try_new(v.x, v.y, v.z).unwrap()
    };
    let start = point(-10.0, 2.0);
    let end = point(20.0, -2.0);
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let mut renderer = OffscreenRenderer::new(format);
        for points in [[start, end], [end, start]] {
            let mut scene = GpuSceneBuilder::new();
            viewport.add_gpu_line(&mut scene, rect, points[0], points[1], 3.0, Color32::WHITE);
            let uniform = viewport.gpu_view_uniform(rect, scene.depth_range());
            let pixels = renderer.render(&scene.finish(uniform, false));
            for depth in [2.0, 8.0, 15.0] {
                let x = 2.0 - 4.0 * (depth + 10.0) / 30.0;
                let pointer = viewport.project(point(depth, x), rect).unwrap();
                assert!(rect.shrink(4.0).contains(pointer));
                let x = pointer.x as u32;
                let y = pointer.y as u32;
                assert!(
                    (y - 1..=y + 1).any(|y| pixels[(y * SIZE + x) as usize][3] > 0),
                    "{format:?}, depth={depth}, points={points:?}"
                );
                assert_eq!(pixels[((y + 5) * SIZE + x) as usize][3], 0);
            }
        }
        let mut scene = GpuSceneBuilder::new();
        viewport.add_gpu_line(
            &mut scene,
            rect,
            start,
            point(-20.0, -2.0),
            3.0,
            Color32::WHITE,
        );
        let uniform = viewport.gpu_view_uniform(rect, scene.depth_range());
        let pixels = renderer.render(&scene.finish(uniform, false));
        assert!(pixels.iter().all(|pixel| pixel[3] == 0));
    }
}
