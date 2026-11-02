use super::*;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn array_instances_share_tessellation_faces_clipping_and_leave_the_document_unchanged() {
    let mut doc = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![point(0., 0., 0.), point(4., 0., 0.), point(0., 4., 0.)],
        vec![[0, 1, 2]],
        doc.tolerance(),
    )
    .unwrap();
    let source = doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    doc.select_all();
    doc.clear_history().unwrap();
    let before = format!("{doc:?}");
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let maps = [
        AffineTransform3::identity(),
        AffineTransform3::from_translation(Vector3::try_new(6., 0., 1000.).unwrap()),
        AffineTransform3::try_uniform_scale(point(0., 0., 0.), -1.).unwrap(),
    ];
    let sources = [source];
    let preview = ObjectPreview::Instances(super::super::object_preview::InstancePreview {
        sources: &sources,
        transforms: &maps,
    });
    view.refresh_clipping_with_preview(&doc, rect, Some(preview))
        .unwrap();
    let first = view.object_scene_with_object_preview(rect, &doc, None, &[], Some(preview));
    let second = view.object_scene_with_object_preview(rect, &doc, None, &[], Some(preview));
    assert!(Arc::ptr_eq(&first, &second));
    let geometry = {
        let cache = view.cached_scene.borrow();
        let objects = &cache.as_ref().unwrap().key.objects;
        assert_eq!(objects.len(), 4);
        assert!(objects.iter().all(|o| o.draw_faces));
        assert!(
            objects
                .iter()
                .all(|o| Rc::ptr_eq(&o.geometry, &objects[0].geometry))
        );
        Rc::clone(&objects[0].geometry)
    };
    view.object_scene_with_object_preview(rect, &doc, None, &[], None);
    let cache = view.cached_scene.borrow();
    let objects = &cache.as_ref().unwrap().key.objects;
    assert_eq!(objects.len(), 1);
    assert!(Rc::ptr_eq(&objects[0].geometry, &geometry));
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn gpu_curve_lines_match_after_exact_knot_translation() {
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let make = |origin| {
        NurbsCurve::try_new(
            2,
            vec![point(0., 0., 0.), point(1., 2., 0.), point(2., 0., 0.)],
            vec![
                origin,
                origin,
                origin,
                origin + 2.,
                origin + 2.,
                origin + 2.,
            ],
        )
        .unwrap()
    };
    let mut expected = GpuSceneBuilder::new();
    let mut actual = GpuSceneBuilder::new();
    view.add_gpu_nurbs_curve(&mut expected, rect, &make(0.), 1., Color32::WHITE);
    view.add_gpu_nurbs_curve(
        &mut actual,
        rect,
        &make(2.0_f64.powi(52)),
        1.,
        Color32::WHITE,
    );
    assert_eq!(actual.lines.len(), CURVE_SAMPLES_PER_SPAN);
    for (a, b) in actual.lines.iter().zip(&expected.lines) {
        assert_eq!(a.instance.start_width, b.instance.start_width);
        assert_eq!(a.instance.end_padding, b.instance.end_padding);
        assert_eq!(a.depths, b.depths);
    }
}

#[test]
fn collapsed_faces_do_not_change_healthy_corner_normals() {
    let source = TriangleMesh::try_new(
        vec![point(0., 0., 0.), point(1., 0., 0.), point(0., 1., -1.)],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mixed = TriangleMesh::try_from_face_records(
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0., 1., -1.),
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(0.5, 0., 0.),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([3, 4, 5])],
    )
    .unwrap();
    let normals = mesh_normals::mesh_corner_normals(&mixed);
    assert_eq!(normals[0], mesh_normals::mesh_corner_normals(&source)[0]);
    assert!(
        normals
            .iter()
            .flatten()
            .all(|n| n.iter().all(|v| v.is_finite()))
    );
}

#[test]
fn tessellation_shading_keeps_ninety_degree_edges_hard() {
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ],
        vec![[0, 1, 2], [0, 1, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let normals = mesh_normals::tessellation_corner_normals(&mesh);
    assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].z, 1.0));
    assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].y, 0.0));
    assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].y, -1.0));
    assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].z, 0.0));
}

#[test]
fn object_color_overrides_layer_color_only_for_object_source() {
    let object = ColorRgb::new(12, 34, 56);
    let layer = ColorRgb::new(78, 90, 123);
    let document = Document::default();
    let base = ObjectAttributes::on_layer(document.current_layer_id()).with_object_color(object);
    assert_eq!(
        resolved_display_color(&base, layer),
        Color32::from_rgb(12, 34, 56)
    );
    for source in [
        viboceros_document::ObjectColorSource::Layer,
        viboceros_document::ObjectColorSource::Material,
        viboceros_document::ObjectColorSource::Parent,
    ] {
        assert_eq!(
            resolved_display_color(&base.clone().with_color_source(source), layer),
            Color32::from_rgb(78, 90, 123)
        );
    }
}

#[test]
fn special_preview_draws_only_candidates_and_highlights_pending_picks() {
    let mut document = Document::default();
    let visible = document
        .add_geometry(Geometry::Point(point(-2., 0., 0.)))
        .unwrap();
    let hidden = document
        .add_geometry(Geometry::Point(point(0., 0., 0.)))
        .unwrap();
    let locked = document
        .add_geometry(Geometry::Point(point(2., 0., 0.)))
        .unwrap();
    document.set_objects_visibility([hidden], false).unwrap();
    document.set_objects_locked([locked], true).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let normal = view.object_scene(rect, &document);
    assert_eq!(normal.points.len(), 2);
    let shown = view.object_scene_with_preview(
        rect,
        &document,
        Some(ObjectSelectionFilter::HiddenObjects),
        &[hidden],
    );
    assert_eq!(shown.points.len(), 1);
    assert_eq!(shown.points[0].color, color_to_gpu(SELECTED_COLOR));
    let unlocked = view.object_scene_with_preview(
        rect,
        &document,
        Some(ObjectSelectionFilter::LockedObjects),
        &[],
    );
    assert_eq!(unlocked.points.len(), 1);
    assert_ne!(unlocked.points[0].color, color_to_gpu(LOCKED_COLOR));
    assert!(document.object(visible).unwrap().attributes().is_visible());
}

#[test]
fn choice_highlight_is_drawn_after_coincident_unselected_points() {
    let mut document = Document::default();
    let point = point(0.0, 0.0, 0.0);
    let first = document.add_geometry(Geometry::Point(point)).unwrap();
    let second = document.add_geometry(Geometry::Point(point)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let scene = view.object_scene_with_preview(rect, &document, None, &[first]);
    assert_eq!(scene.points.len(), 2);
    assert_ne!(scene.points[0].color, color_to_gpu(PICK_PREVIEW_COLOR));
    assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
    assert_eq!(document.selected_object_count(), 0);
    let scene = view.object_scene_with_preview(rect, &document, None, &[second]);
    assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
    document
        .select_objects([first, second], SelectionMode::Replace)
        .unwrap();
    let scene = view.object_scene_with_preview(rect, &document, None, &[first]);
    assert_eq!(scene.points[0].color, color_to_gpu(SELECTED_COLOR));
    assert_eq!(scene.points[1].color, color_to_gpu(PICK_PREVIEW_COLOR));
}

#[test]
fn point_cloud_member_colors_reach_the_gpu_scene() {
    let mut document = Document::default();
    let cloud = PointCloud3::try_with_colors(
        vec![point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)],
        Some(vec![[200, 20, 30, 0], [40, 50, 220, 128]]),
    )
    .unwrap();
    let id = document.add_geometry(Geometry::PointCloud(cloud)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let scene = view.object_scene(rect, &document);
    assert_eq!(scene.points.len(), 2);
    let expected = [
        [200.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0],
        [40.0 / 255.0, 50.0 / 255.0, 220.0 / 255.0, 127.0 / 255.0],
    ];
    for color in expected {
        assert!(
            scene.points.iter().any(|point| {
                point
                    .color
                    .iter()
                    .zip(color)
                    .all(|(actual, expected)| (actual - expected).abs() <= 1.1 / 255.0)
            }),
            "expected {color:?}, got {:?}",
            scene
                .points
                .iter()
                .map(|point| point.color)
                .collect::<Vec<_>>()
        );
    }
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let selected = view.object_scene(rect, &document);
    assert!(
        selected
            .points
            .iter()
            .all(|point| point.color == color_to_gpu(SELECTED_COLOR))
    );
}

#[test]
fn hidden_cloud_members_do_not_reach_gpu_scene() {
    let mut document = Document::default();
    let cloud = PointCloud3::try_new(vec![point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)])
        .unwrap()
        .with_hidden(vec![true, false])
        .unwrap();
    document.add_geometry(Geometry::PointCloud(cloud)).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let scene = view.object_scene(rect, &document);
    assert_eq!(scene.points.len(), 1);
}

#[test]
fn mesh_vertex_colors_reach_the_gpu_scene() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            point(-1.0, -1.0, 0.0),
            point(1.0, -1.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![
        [200, 20, 30, 0],
        [40, 50, 220, 128],
        [10, 240, 80, 0],
    ]))
    .unwrap();
    let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    view.display_mode = DisplayMode::Shaded;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
    let scene = view.object_scene(rect, &document);
    assert_eq!(scene.triangles.len(), 3);
    let expected = [
        [200.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0],
        [40.0 / 255.0, 50.0 / 255.0, 220.0 / 255.0, 127.0 / 255.0],
        [10.0 / 255.0, 240.0 / 255.0, 80.0 / 255.0, 1.0],
    ];
    for (actual, expected) in scene.triangles.iter().zip(expected) {
        assert!(
            actual
                .color
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() <= 1.1 / 255.0)
        );
    }
    document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let selected = view.object_scene(rect, &document);
    // Rhino highlights selected wires while retaining mesh vertex colors on faces.
    for (actual, expected) in selected.triangles.iter().zip(&scene.triangles) {
        assert_eq!(actual.color, expected.color);
    }
    assert!(
        selected
            .lines
            .iter()
            .all(|line| line.color == color_to_gpu(SELECTED_COLOR))
    );
}

#[test]
fn stored_mesh_normals_reach_shaded_and_ghosted_gpu_scenes() {
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![point(-1., -1., 0.), point(1., -1., 0.), point(0., 1., 0.)],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap()
    .try_with_vertex_normals(Some(vec![
        Vector3::try_new(2., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 1e-38, 0.).unwrap(),
    ]))
    .unwrap();
    document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let mut view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
        view.display_mode = mode;
        let scene = view.object_scene(rect, &document);
        assert_eq!(scene.triangles.len(), 3);
        for (vertex, normal) in
            scene
                .triangles
                .iter()
                .zip([[1., 0., 0.], [0., 0., 1.], [0., 1., 0.]])
        {
            assert_eq!(vertex.normal, normal);
        }
    }
}
