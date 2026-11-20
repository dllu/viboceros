use super::*;

#[test]
fn growth_and_chunking_respect_device_limits_and_primitive_boundaries() {
    let limit = 256 * 1024 * 1024;
    assert_eq!(buffer_capacity(limit - 4, limit), limit);
    assert_eq!(buffer_capacity(192 * 1024 * 1024, limit), limit);
    assert_eq!(buffer_capacity(1000, 1000), 1000);
    for bytes in [120, 121, 1000, 1024 * 1024, limit] {
        let count = chunk_elements(bytes, std::mem::size_of::<TriangleVertex>(), 3);
        assert_eq!(count % 3, 0);
        assert!(count as u64 * std::mem::size_of::<TriangleVertex>() as u64 <= bytes);
    }
    assert_eq!(chunk_elements(119, 40, 3), 0);
}

#[test]
#[ignore = "requires a graphics adapter; run explicitly with --ignored --nocapture"]
fn chunked_upload_renders_every_primitive_and_survives_display_switches() {
    use readback::{OffscreenRenderer, SIZE};
    let mut renderer =
        OffscreenRenderer::with_buffer_limit(wgpu::TextureFormat::Rgba8Unorm, Some(1024 * 1024));
    let vertex = |position| TriangleVertex {
        position,
        normal: [0., 0., 1.],
        color: [1., 0., 0., 1.],
        shading: 1.,
    };
    let scene = |triangles, lines, points, overlay_line_start, overlay_point_start| ViewportScene {
        uniform: ViewUniform {
            view_projection: [
                [1., 0., 0., 0.],
                [0., 1., 0., 0.],
                [0., 0., 1., 0.],
                [0., 0., 0., 1.],
            ],
            viewport_size: [SIZE as f32; 2],
            clip_depth: [0., 1.],
            marker_scale: 1.,
            padding: [0.; 3],
        },
        geometry: Arc::new(ViewportGeometry {
            triangles,
            lines,
            points,
            overlay_line_start,
            overlay_point_start,
            transparent: false,
        }),
    };
    let triangle = vec![
        vertex([-0.8, -0.8, 0.5]),
        vertex([0.8, -0.8, 0.5]),
        vertex([0., 0.8, 0.5]),
    ];
    let line = LineInstance {
        start_width: [-0.5, 0., 0.9, 4.],
        end_padding: [0.5, 0., 0.9, 0.],
        color: [0., 1., 0., 1.],
    };
    let point = PointInstance {
        position_size: [0., 0.3, 0.9, 8.],
        color: [0., 0., 1., 1.],
    };
    let first = scene(triangle.clone(), vec![line], vec![point], 0, 0);
    let expected = renderer.render(&first);
    assert!(expected.iter().any(|p| p[3] > 0));
    assert_eq!(renderer.render_at(4, &first), expected);
    let uploaded = renderer.preparations();
    let mut camera = ViewportScene {
        uniform: first.uniform,
        geometry: Arc::clone(&first.geometry),
    };
    camera.uniform.view_projection[3][0] = 0.2;
    assert_ne!(renderer.render(&camera), expected);
    assert_eq!(renderer.preparations(), uploaded);
    camera.uniform = first.uniform;
    camera.uniform.marker_scale = 2.;
    let larger_markers = renderer.render(&camera);
    for channel in [1, 2] {
        let visible = |pixels: &[[u8; 4]]| pixels.iter().filter(|p| p[channel] == 255).count();
        assert!(visible(&larger_markers) > visible(&expected));
    }
    assert_eq!(renderer.preparations(), uploaded);
    // All three buffers exceed the actual device limit; visible primitives land
    // after a chunk boundary. Overlay ranges cross those boundaries as well.
    let mut triangles = vec![vertex([2., 2., 0.5]); 30_000];
    triangles.extend(triangle.clone());
    let mut lines = vec![
        LineInstance {
            start_width: [2., 2., 0.5, 1.],
            end_padding: [3., 2., 0.5, 0.],
            color: [1.; 4]
        };
        24_000
    ];
    lines.push(line);
    let mut points = vec![
        PointInstance {
            position_size: [2., 2., 0.5, 1.],
            color: [1.; 4]
        };
        36_000
    ];
    points.push(point);
    let large = scene(triangles, lines, points, 23_999, 35_999);
    for _ in 0..3 {
        assert_eq!(renderer.render(&large), expected);
        renderer.render(&scene(Vec::new(), Vec::new(), Vec::new(), 0, 0));
        assert_eq!(
            renderer.render(&scene(triangle.clone(), vec![line], vec![point], 0, 0)),
            expected
        );
    }
}
