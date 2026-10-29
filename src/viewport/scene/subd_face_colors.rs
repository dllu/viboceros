use super::*;
use viboceros_geometry::{SubD, SubDFaceProperties};
fn p(x: f64, y: f64) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn source() -> SubD {
    SubD::try_from_polygons(
        vec![
            p(0., 0.),
            p(2., 0.),
            p(4., 0.),
            p(0., 3.),
            p(2., 3.),
            p(4., 3.),
        ],
        vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]],
    )
    .unwrap()
}
fn rect() -> Rect {
    Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))
}

#[test]
fn per_face_colors_reach_smooth_and_flat_gpu_faces_and_invalidate_cached_scenes() {
    let red = [173, 29, 61, 64];
    let green = [17, 191, 43, 0];
    for flat in [false, true] {
        for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
            let mut doc = Document::default();
            doc.set_subd_control_net(flat);
            let colored = source()
                .try_with_face_properties(&[(
                    0,
                    SubDFaceProperties {
                        color: Some(red),
                        material_channel: 17,
                    },
                )])
                .unwrap();
            let id = doc.add_geometry(Geometry::SubD(colored.clone())).unwrap();
            let mut view = Viewport::new(ViewKind::Top);
            view.display_mode = mode;
            view.last_rect = Some(rect());
            view.zoom_extents(&doc, ZoomExtentsBorders::default())
                .unwrap();
            let alpha = if mode == DisplayMode::Ghosted {
                35
            } else {
                255 - red[3]
            };
            let expected = color_to_gpu(Color32::from_rgba_unmultiplied(
                red[0], red[1], red[2], alpha,
            ));
            let scene = view.object_scene(rect(), &doc);
            assert!(
                scene.triangles.iter().any(|v| v.color == expected),
                "{flat} {mode:?}"
            );
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            assert_eq!(
                view.object_scene(rect(), &doc)
                    .triangles
                    .iter()
                    .map(|v| v.color)
                    .collect::<Vec<_>>(),
                scene.triangles.iter().map(|v| v.color).collect::<Vec<_>>()
            );
            let changed = colored
                .try_with_face_properties(&[(
                    0,
                    SubDFaceProperties {
                        color: Some(green),
                        material_channel: 17,
                    },
                )])
                .unwrap();
            doc.replace_object_geometries([(id, Geometry::SubD(changed))])
                .unwrap();
            let fresh = view.object_scene(rect(), &doc);
            assert!(!fresh.triangles.iter().any(|v| v.color == expected));
            let expected = color_to_gpu(Color32::from_rgba_unmultiplied(
                green[0],
                green[1],
                green[2],
                if mode == DisplayMode::Ghosted {
                    35
                } else {
                    255
                },
            ));
            assert!(fresh.triangles.iter().any(|v| v.color == expected));
            doc.set_objects_locked([id], true).unwrap();
            assert!(
                !view
                    .object_scene(rect(), &doc)
                    .triangles
                    .iter()
                    .any(|v| v.color == expected)
            );
        }
    }
}

#[test]
fn merged_native_faces_use_id_sorted_ordinals_for_smooth_and_flat_colors() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let data: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tools/rhino_oracle/observations/subd_merge_properties.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let row = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "strip-retain-neighbor")
        .unwrap();
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        STANDARD
            .decode(row["value"]["steps"][0]["after_3dm"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let model = viboceros_io::read_3dm_file(file.path(), Tolerance::DEFAULT).unwrap();
    let subd = model
        .objects
        .into_iter()
        .find_map(|o| match o.geometry {
            viboceros_io::ThreeDmGeometry::SubD(s) => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(subd.face_ids(), &[1, 3]);
    for flat in [false, true] {
        let display = DisplayGeometry::in_appearance(
            Geometry::SubD(subd.clone()).into(),
            1,
            Tolerance::DEFAULT,
            flat,
        );
        let colors = display.triangle_colors().unwrap();
        let mesh = display.mesh().unwrap();
        assert_eq!(colors.len(), mesh.triangles().len());
        let mut seen = [false; 2];
        for (i, &color) in colors.iter().enumerate() {
            let x = mesh
                .triangle_points(i)
                .unwrap()
                .iter()
                .map(|p| p.x())
                .sum::<f64>()
                / 3.;
            if x < 2. {
                assert_eq!(color, None, "flat={flat} x={x}");
                seen[0] = true;
            }
            if x > 5. {
                assert_eq!(color, Some([61, 97, 233, 176]), "flat={flat} x={x}");
                seen[1] = true;
            }
        }
        assert_eq!(seen, [true, true]);
    }
}
