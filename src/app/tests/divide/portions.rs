use super::*;

#[test]
fn divide_subcurve_viewport_previews_and_splits_curves_and_surface_edges() {
    for edge in [false, true] {
        let mut app = test_app();
        app.osnap = false;
        let shape = if edge {
            "Plane 0,0,0 10,2,0"
        } else {
            "Line 0,0,0 10,0,0"
        };
        enter(&mut app, shape);
        let source = app.document.objects().next().unwrap().clone();
        enter(&mut app, "SelNone");
        app.document.clear_history().unwrap();
        let context = egui::Context::default();
        top_view(&mut app, &context);

        enter(&mut app, "Divide");
        enter(&mut app, "SubCrv");
        click_world(&mut app, &context, point(5., 0., 0.));
        let reference = app
            .divide_subcurve_editor()
            .unwrap()
            .reference
            .as_ref()
            .unwrap();
        assert_eq!(reference.object_id(), source.id());
        assert_eq!(reference.source().1.is_some(), edge);
        enter(&mut app, "NoSnap");
        assert_eq!(app.one_shot_snap_label(), Some("NoSnap"));
        click_world(&mut app, &context, point(2., 0., 0.));
        assert_eq!(app.one_shot_snap_label(), None);
        assert!(
            app.divide_subcurve_editor()
                .unwrap()
                .hint()
                .starts_with("Select end location"),
            "start pick on surface edge={edge}",
        );
        for _ in 0..3 {
            viewport_frame(&mut app, &context, vec![]);
        }
        let end = point(8., 0., 0.);
        let pixel = screen_point(&app, end);
        viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
        let preview = app
            .subcurve_draft_preview()
            .unwrap_or_else(|| panic!("preview between picks on surface edge={edge}"));
        assert_points(&preview.geometry.endpoints, &[point(2., 0., 0.), end]);
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());
        click_world(&mut app, &context, end);
        assert!(app.divide_subcurve_editor().is_none());
        assert_eq!(app.document.selected_object_count(), 0);

        enter(&mut app, "");
        enter(&mut app, "3 Split=Yes");
        let ends = [2., 4., 4., 6., 6., 8.].map(|x| point(x, 0., 0.));
        assert_points(&app.divide_preview_points().unwrap(), &ends);
        enter(&mut app, "");
        assert_eq!(app.document.objects().len(), 4);
        assert_eq!(app.document.object(source.id()), Some(&source));
        assert_eq!(app.document.undo_label(), Some("Divide"));
        let actual = app
            .document
            .objects()
            .filter(|o| o.id() != source.id())
            .flat_map(|o| {
                let curve = o.geometry().curve_ref().unwrap();
                [curve.start_point().unwrap(), curve.end_point().unwrap()]
            })
            .collect::<Vec<_>>();
        assert_points(&actual, &ends);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().next(), Some(&source));
        assert_eq!(app.document.objects().len(), 1);
    }
}

#[test]
fn divide_subcurve_mixed_sources_split_in_pick_order_and_retain_the_parent() {
    let mut app = test_app();
    for input in ["Line 0,0 10,0", "Line 0,5 10,5", "SelNone"] {
        enter(&mut app, input);
    }
    let originals = app.document.objects().cloned().collect::<Vec<_>>();
    let parent = originals[0].id();
    let whole = originals[1].id();
    app.document.clear_history().unwrap();
    for input in [
        "_Divide _SubCrv".into(),
        "_-SelID".into(),
        parent.to_string(),
        "2,0,0".into(),
        "r6,0,0".into(),
        format!("SelID={whole}"),
        "SubCrv".into(),
        format!("SelID {parent}"),
        "8,0,0".into(),
        "2,0,0".into(),
        "".into(),
        "3 Split=Yes".into(),
    ] {
        enter(&mut app, &input);
    }
    let expected = [
        [2., 4., 0.],
        [4., 6., 0.],
        [6., 8., 0.],
        [0., 10. / 3., 5.],
        [10. / 3., 20. / 3., 5.],
        [20. / 3., 10., 5.],
        [8., 6., 0.],
        [6., 4., 0.],
        [4., 2., 0.],
    ];
    let ends = expected
        .map(|[a, b, y]| [point(a, y, 0.), point(b, y, 0.)])
        .concat();
    assert_points(&app.divide_preview_points().unwrap(), &ends);
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());

    enter(&mut app, "_Enter");
    assert_eq!(app.document.object(parent), Some(&originals[0]));
    assert!(app.document.object(whole).is_none());
    assert_eq!(app.document.objects().len(), 10);
    let pieces = app.document.objects().filter(|o| o.id() != parent);
    for (object, [a, b, y]) in pieces.zip(expected) {
        let curve = object.geometry().curve_ref().unwrap();
        assert_points(
            &[curve.start_point().unwrap(), curve.end_point().unwrap()],
            &[point(a, y, 0.), point(b, y, 0.)],
        );
        let domain = if a > b { -a..=-b } else { a..=b };
        assert!((curve.domain().start() - domain.start()).abs() < 1e-10);
        assert!((curve.domain().end() - domain.end()).abs() < 1e-10);
    }
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        originals
    );
}

#[test]
fn divide_subcurve_numeric_length_can_wrap_a_closed_curve() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0 2");
    let source = app.document.objects().next().unwrap().clone();
    enter(&mut app, "SelNone");
    app.document.clear_history().unwrap();
    for input in [
        "Divide SubCrv".into(),
        format!("SelID={}", source.id()),
        "0,-2,0".into(),
        "3".into(),
        "2,0,0".into(),
        "".into(),
        "3 MarkEnds=Yes GroupOutput=Yes".into(),
    ] {
        enter(&mut app, &input);
    }
    let expected =
        [0., 0.5, 1., 1.5].map(|angle: f64| point(2. * angle.sin(), -2. * angle.cos(), 0.));
    assert_points(&app.divide_preview_points().unwrap(), &expected);
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.object(source.id()), Some(&source));
    assert_eq!(app.document.groups().count(), 1);
}

#[test]
fn divide_subcurve_cancel_returns_to_selection_and_stale_source_hides_preview() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0 10,0");
    let parent = app.document.objects().next().unwrap().id();
    enter(&mut app, "SelNone");
    app.document.clear_history().unwrap();
    for input in ["Divide", "SubCrv", &format!("SelID {parent}"), "1,0,0"] {
        enter(&mut app, input);
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.divide_subcurve_editor().is_none());
    assert!(app.object_prompt.is_some());
    assert!(!app.document.can_undo());

    for input in [
        "SubCrv",
        &format!("SelID {parent}"),
        "2,0,0",
        "8,0,0",
        "",
        "3",
    ] {
        enter(&mut app, input);
    }
    assert!(app.divide_preview_points().is_some());
    let replacement = viboceros_geometry::LineSegment::try_new(
        point(0., 0., 0.),
        point(12., 0., 0.),
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .replace_object_geometries([(parent, Geometry::Line(replacement))])
        .unwrap();
    assert!(app.divide_preview_points().is_none());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 1);
}
