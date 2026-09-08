use super::*;

#[test]
fn surface_filter_precedes_coincident_point_curve_and_mesh_hits() {
    let mut document = Document::default();
    let points = vec![
        point(-2., 0., 0.),
        point(2., 0., 0.),
        point(-2., 2., 0.),
        point(2., 2., 0.),
    ];
    let surface = document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                points.clone(),
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(points[0], points[1], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(points, vec![[0, 1, 2]], Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let dot = document
        .add_geometry(Geometry::Point(point(0., 0., 0.)))
        .unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        let view = Viewport::new(kind);
        let pointer = view.project(point(0., 0., 0.), rect).unwrap();
        assert_eq!(view.pick_object(pointer, rect, &document), Some(dot));
        assert_eq!(
            view.pick_object_matching(pointer, rect, &document, ObjectSelectionFilter::Surfaces),
            Some(surface)
        );
    }
    document.set_objects_locked([surface], true).unwrap();
    let view = Viewport::new(ViewKind::Top);
    let pointer = view.project(point(0., 0., 0.), rect).unwrap();
    assert_eq!(
        view.pick_object_matching(pointer, rect, &document, ObjectSelectionFilter::Surfaces),
        None
    );
}

#[test]
fn confirmation_disables_real_selection_events_without_enabling_point_drafting() {
    let mut document = Document::default();
    document
        .add_geometry(Geometry::Point(point(0., 0., 0.)))
        .unwrap();
    let line = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(point(-2., 0., 0.), point(2., 0., 0.), Tolerance::DEFAULT)
                .unwrap(),
        ))
        .unwrap();
    let context = egui::Context::default();
    let mut viewport = Viewport::new(ViewKind::Top);
    let mut frame = |events, object_filter| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = viewport.show(
                        ui,
                        &document,
                        ViewportInput {
                            object_filter,
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        output
    };
    let pointer = Pos2::new(400., 300.);
    let event = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    for filter in [
        Some(ObjectSelectionFilter::Beziers),
        Some(ObjectSelectionFilter::ToNurbs),
        None,
        Some(ObjectSelectionFilter::ToNurbs),
    ] {
        frame(vec![], filter);
        frame(
            vec![egui::Event::PointerMoved(pointer), event(pointer, true)],
            filter,
        );
        let output = frame(vec![event(pointer, false)], filter);
        assert_eq!(
            output.selection_click.map(|c| c.object_id),
            filter.map(|_| Some(line))
        );
        assert!(output.picked_point.is_none());
    }
    let end = Pos2::new(500., 400.);
    frame(vec![event(pointer, true)], None);
    frame(vec![egui::Event::PointerMoved(end)], None);
    let output = frame(vec![event(end, false)], None);
    assert!(output.selection_window.is_none());
    assert!(output.picked_point.is_none());
}

#[test]
fn real_pointer_events_use_the_mesh_filter_for_clicks_and_windows() {
    let mut document = Document::default();
    let mesh = document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![point(-2., 0., 0.), point(2., 0., 0.), point(0., 2., 0.)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .add_geometry(Geometry::Point(point(0., 0., 0.)))
        .unwrap();
    let context = egui::Context::default();
    let mut viewport = Viewport::new(ViewKind::Top);
    let mut frame = |events| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = viewport.show(
                        ui,
                        &document,
                        ViewportInput {
                            object_filter: Some(ObjectSelectionFilter::Mesh),
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        output
    };
    let event = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![]);
    let pointer = Pos2::new(400., 300.);
    frame(vec![
        egui::Event::PointerMoved(pointer),
        event(pointer, true),
    ]);
    let clicked = frame(vec![event(pointer, false)]);
    assert_eq!(clicked.selection_click.unwrap().object_id, Some(mesh));
    assert!(clicked.picked_point.is_none());
    for (start, end, crossing) in [
        (Pos2::new(280., 180.), Pos2::new(520., 340.), false),
        (Pos2::new(520., 340.), Pos2::new(280., 180.), true),
    ] {
        frame(vec![egui::Event::PointerMoved(start), event(start, true)]);
        frame(vec![egui::Event::PointerMoved(end)]);
        let selection = frame(vec![event(end, false)]).selection_window.unwrap();
        assert_eq!(selection.object_ids, [mesh]);
        assert_eq!(selection.crossing, crossing);
    }
}

#[test]
fn mesh_filter_is_applied_before_point_and_curve_hit_priority() {
    let mut d = Document::default();
    let mesh = d
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![point(0., 0., 0.), point(4., 0., 0.), point(0., 3., 0.)],
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ))
        .unwrap();
    d.add_geometry(Geometry::Line(
        LineSegment::try_new(point(0., 0., 0.), point(4., 0., 0.), Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap();
    let dot = d.add_geometry(Geometry::Point(point(2., 0., 0.))).unwrap();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        let view = Viewport::new(kind);
        let pointer = view.project(point(2., 0., 0.), rect).unwrap();
        assert_eq!(view.pick_object(pointer, rect, &d), Some(dot));
        assert_eq!(
            view.pick_object_matching(pointer, rect, &d, ObjectSelectionFilter::Mesh),
            Some(mesh)
        );
    }
    d.set_objects_locked([mesh], true).unwrap();
    let view = Viewport::default();
    assert_eq!(
        view.pick_object_matching(
            view.project(point(2., 0., 0.), rect).unwrap(),
            rect,
            &d,
            ObjectSelectionFilter::Mesh
        ),
        None
    );
}

#[test]
fn window_and_crossing_filters_exclude_nonmeshes_hidden_objects_and_locked_layers() {
    let mut d = Document::default();
    let mesh = Geometry::Mesh(
        TriangleMesh::try_new(
            vec![point(0., 0., 0.), point(4., 0., 0.), point(0., 3., 0.)],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap(),
    );
    let valid = d.add_geometry(mesh.clone()).unwrap();
    let hidden = d.add_geometry(mesh.clone()).unwrap();
    d.set_objects_visibility([hidden], false).unwrap();
    let layer = d.add_layer("locked", ColorRgb::BLACK).unwrap();
    d.add_geometry_with_attributes(mesh, ObjectAttributes::on_layer(layer))
        .unwrap();
    d.set_layer_locked(layer, true).unwrap();
    d.add_geometry(Geometry::Point(point(2., 0., 0.))).unwrap();
    let view = Viewport::default();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    for crossing in [false, true] {
        assert_eq!(
            view.objects_in_selection_matching(
                rect,
                rect,
                crossing,
                &d,
                ObjectSelectionFilter::Mesh
            ),
            [valid]
        );
    }
}
