use super::*;
use crate::viewport_gpu::readback::{OffscreenRenderer, SIZE};
use eframe::wgpu;

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn gpu_camera_relative_geometry_preserves_large_translation_pixels() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
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
            let mut baseline = None;
            for translation in [
                NaVector3::zeros(),
                NaVector3::new(1073741824.0, -2147483648.0, 4294967296.0),
            ] {
                let mut viewport = Viewport::new(kind);
                viewport.target = translation;
                viewport.orbit_yaw = 0.0;
                viewport.orbit_pitch = 0.0;
                viewport.display_mode = DisplayMode::Shaded;
                let (right, up, forward) = match kind {
                    ViewKind::Top => (NaVector3::x(), NaVector3::y(), -NaVector3::z()),
                    ViewKind::Front => (NaVector3::x(), NaVector3::z(), NaVector3::y()),
                    ViewKind::Right => (NaVector3::y(), NaVector3::z(), -NaVector3::x()),
                    ViewKind::Perspective => viewport.perspective_basis(),
                };
                let point = |x: Real, y: Real, z: Real| {
                    let v = translation + right * x + up * y + forward * z;
                    Point3::try_new(v.x, v.y, v.z).unwrap()
                };
                let mesh = TriangleMesh::try_new(
                    vec![
                        point(-2.0, -2.0, 0.0),
                        point(2.0, -2.0, 0.0),
                        point(0.0, 2.0, 0.0),
                    ],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap();
                let mut scene = GpuSceneBuilder::new();
                viewport.add_gpu_mesh_faces(&mut scene, &mesh, Color32::RED);
                viewport.add_gpu_line(
                    &mut scene,
                    rect,
                    point(-1.0, 3.0, -1.0),
                    point(1.0, 3.0, -1.0),
                    3.0,
                    Color32::WHITE,
                );
                viewport.add_gpu_point(
                    &mut scene,
                    rect,
                    point(-3.0, 0.0, -1.0),
                    3.0,
                    Color32::GREEN,
                );
                assert_eq!(
                    (scene.triangles.len(), scene.lines.len(), scene.points.len()),
                    (1, 1, 1)
                );
                let uniform = viewport.gpu_view_uniform(rect, scene.depth_range());
                let pixels = renderer.render(&scene.finish(uniform, false));
                if let Some(expected) = &baseline {
                    let differences = pixels.iter().zip(expected).filter(|(a, b)| a != b).count();
                    assert_eq!(
                        differences, 0,
                        "{kind:?} {format:?}: translated image differs"
                    );
                } else {
                    assert!(
                        pixels
                            .iter()
                            .any(|pixel| pixel[0] > pixel[1] && pixel[3] == 255)
                    );
                    assert!(pixels.contains(&[0, 255, 0, 255]));
                    assert!(pixels.contains(&[255; 4]));
                    baseline = Some(pixels);
                }
            }
        }
    }
}

#[test]
fn face_click_selection_uses_depth_not_insertion_order() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
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
        let geometry = |depth: Real, representation| {
            let points = [(-3.0, -2.0), (3.0, -2.0), (3.0, 3.0), (-3.0, 3.0)].map(|(x, y)| {
                let v = origin + right * x + up * y + forward * depth;
                Point3::try_new(v.x, v.y, v.z).unwrap()
            });
            if representation == 0 {
                Geometry::Mesh(
                    TriangleMesh::try_new(
                        points.to_vec(),
                        vec![[0, 1, 2], [0, 2, 3]],
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                )
            } else {
                let surface = NurbsSurface::try_bilinear(points).unwrap();
                if representation == 1 {
                    Geometry::NurbsSurface(surface)
                } else {
                    Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap())
                }
            }
        };
        for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
            viewport.display_mode = mode;
            for representation in 0..3 {
                for front_first in [false, true] {
                    let mut document = Document::default();
                    let depths = if front_first {
                        [10.0, 20.0]
                    } else {
                        [20.0, 10.0]
                    };
                    let ids = depths.map(|depth| {
                        document
                            .add_geometry(geometry(depth, representation))
                            .unwrap()
                    });
                    let expected = ids[usize::from(!front_first)];
                    assert_eq!(
                        viewport.pick_object(rect.center(), rect, &document),
                        Some(expected),
                        "{kind:?} {mode:?} front_first={front_first} representation={representation}"
                    );
                }
            }
        }
    }
}

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

#[test]
fn sloped_and_camera_crossing_face_picks_match_independent_rays() {
    let mut viewport = Viewport::new(ViewKind::Perspective);
    viewport.display_mode = DisplayMode::Shaded;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(SIZE as f32));
    let (right, up, forward) = viewport.perspective_basis();
    let camera = -forward * viewport.perspective_camera_distance;
    let focal = viewport.perspective_focal_length_pixels(rect);
    for first_depths in [[2.0, 20.0, 20.0], [-10.0, 20.0, 20.0]] {
        let triangles = [first_depths, [8.0; 3]].map(|depths| {
            let xy = [(-3.0, -2.0), (3.0, -2.0), (0.0, 3.0)];
            std::array::from_fn(|i| {
                let v = camera + right * xy[i].0 + up * xy[i].1 + forward * depths[i];
                Point3::try_new(v.x, v.y, v.z).unwrap()
            })
        });
        for order in [[0, 1], [1, 0]] {
            let mut document = Document::default();
            let ids = order.map(|i| {
                document
                    .add_geometry(Geometry::Mesh(
                        TriangleMesh::try_new(
                            triangles[i].to_vec(),
                            vec![[0, 1, 2]],
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                    ))
                    .unwrap()
            });
            let mut wins = [0; 2];
            for y in (0..SIZE).step_by(8) {
                for x in (0..SIZE).step_by(8) {
                    let pointer = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let direction = forward
                        + right * ((Real::from(pointer.x) - Real::from(SIZE) / 2.0) / focal)
                        - up * ((Real::from(pointer.y) - Real::from(SIZE) / 2.0) / focal);
                    let hits = triangles.map(|triangle| ray_triangle(camera, direction, triangle));
                    // Keep well inside each covered face and away from edge capture.
                    if hits
                        .iter()
                        .flatten()
                        .any(|[t, a, b, c]| *t > 0.0 && a.min(*b).min(*c).abs() < 0.03)
                    {
                        continue;
                    }
                    let depths = hits.map(|hit| {
                        hit.filter(|[t, a, b, c]| *t > 0.0 && a.min(*b).min(*c) > 0.0)
                            .map(|hit| hit[0])
                            .unwrap_or(Real::INFINITY)
                    });
                    let winner = usize::from(depths[1] < depths[0]);
                    if !depths[winner].is_finite() || (depths[0] - depths[1]).abs() < 1e-5 {
                        continue;
                    }
                    let expected = ids[order.iter().position(|i| *i == winner).unwrap()];
                    assert_eq!(
                        viewport.pick_object(pointer, rect, &document),
                        Some(expected),
                        "depths={first_depths:?}, order={order:?}, pointer={pointer:?}"
                    );
                    wins[winner] += 1;
                }
            }
            assert!(
                wins.iter().all(|count| *count > 0),
                "each face must win somewhere: {wins:?}"
            );
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
