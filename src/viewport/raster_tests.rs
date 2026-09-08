use super::*;
use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
use eframe::wgpu;

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
