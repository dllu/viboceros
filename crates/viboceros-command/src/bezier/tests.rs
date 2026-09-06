use super::*;

fn curve() -> NurbsCurve {
    NurbsCurve::try_new_rational(
        3,
        [(0., 0., 1.), (1., 3., 0.7), (4., 2., 1.4), (5., 0., 1.)]
            .into_iter()
            .map(|(x, y, w)| {
                WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), w).unwrap()
            })
            .collect(),
        vec![-3., -2., -1., 0., 2., 3., 4., 5.],
    )
    .unwrap()
}

#[test]
fn fresh_unit_domain_output_keeps_empty_groups_and_is_fully_undoable() {
    for delete in [false, true] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let source_layer = doc.current_layer_id();
        let current = doc.add_layer("Current", ColorRgb::BLACK).unwrap();
        doc.set_current_layer(current).unwrap();
        let source = doc
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(curve()),
                ObjectAttributes::on_layer(source_layer)
                    .with_name("source")
                    .with_object_color(ColorRgb::new(11, 22, 33)),
            )
            .unwrap();
        let group = doc.add_group(Some("sources".into()), [source]).unwrap();
        let empty = doc.add_empty_group(Some("empty".into())).unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut doc,
                if delete {
                    "ConvertToBeziers DeleteInput=Yes"
                } else {
                    "ConvertToBeziers DeleteInput=No"
                },
            )
            .unwrap();
        let output = doc.objects().find(|o| o.id() != source).unwrap();
        let output_id = output.id();
        assert_eq!(output.attributes(), &ObjectAttributes::on_layer(current));
        assert!(output.group_ids().is_empty());
        assert!(!doc.is_selected(output_id));
        let Geometry::NurbsCurve(piece) = output.geometry() else {
            panic!()
        };
        assert_eq!(piece.domain(), 0.0..=1.0);
        assert_eq!(piece.knots(), [0., 0., 0., 0., 1., 1., 1., 1.]);
        for i in 0..=32 {
            let t = i as f64 / 32.;
            assert!(
                piece
                    .evaluate(t)
                    .unwrap()
                    .distance_to(curve().evaluate(2. * t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
        assert_eq!(doc.object(source).is_some(), !delete);
        assert_eq!(doc.is_selected(source), !delete);
        assert_eq!(
            doc.group(group).unwrap().members().len(),
            usize::from(!delete)
        );
        assert_eq!(doc.group(empty).unwrap().members().len(), 0);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 1);
        assert_eq!(
            doc.object(source).unwrap().geometry(),
            &Geometry::NurbsCurve(curve())
        );
        assert_eq!(
            doc.group(group).unwrap().members().collect::<Vec<_>>(),
            [source]
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(output_id).is_some());
        assert_eq!(doc.object(source).is_some(), !delete);
    }
}

#[test]
fn mixed_selection_ignores_points_and_source_document_order_wins() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let first = doc.add_geometry(Geometry::NurbsCurve(curve())).unwrap();
    let point = doc
        .add_geometry(Geometry::Point(Point3::try_new(50., 0., 0.).unwrap()))
        .unwrap();
    let second = doc
        .add_geometry(Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(20., 0., 0.).unwrap(),
                Point3::try_new(23., 0., 0.).unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    doc.select_objects_direct([second, point, first], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut doc, "ConvertToBeziers Yes").unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [point]);
    let outputs = doc
        .objects()
        .filter(|o| o.id() != point)
        .collect::<Vec<_>>();
    assert!(
        outputs[0]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .x()
            < 10.
    );
    assert_eq!(
        outputs[1]
            .geometry()
            .curve_ref()
            .unwrap()
            .start_point()
            .unwrap()
            .x(),
        20.
    );
}

#[test]
fn surface_patches_have_unit_uv_and_do_not_inherit_trims_or_reversed_orientation() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        [(0., 0.), (2., 0.), (4., 0.), (0., 3.), (2., 3.), (4., 3.)]
            .into_iter()
            .map(|(x, y)| Point3::try_new(x, y, 0.).unwrap())
            .collect(),
        vec![-2., -2., 1., 4., 4.],
        vec![10., 10., 18., 18.],
    )
    .unwrap();
    let brep = Brep::try_rectangular_surface_face_with_orientation(
        surface.clone(),
        -1.0..=3.0,
        11.0..=17.0,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut doc = Document::default();
    let source = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    doc.select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(&mut doc, "ConvertToBeziers Yes")
        .unwrap();
    assert_eq!(doc.objects().len(), 2);
    for (i, o) in doc.objects().enumerate() {
        let Geometry::NurbsSurface(s) = o.geometry() else {
            panic!("untrimmed surface expected")
        };
        assert_eq!(s.domain_u(), 0.0..=1.0);
        assert_eq!(s.domain_v(), 0.0..=1.0);
        assert!(
            s.evaluate(0.5, 0.5)
                .unwrap()
                .distance_to(surface.evaluate(-0.5 + i as f64 * 3., 14.).unwrap())
                .unwrap()
                < 1e-13
        );
    }
}

#[test]
fn invalid_options_and_unrepresentable_final_controls_leave_document_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let first = doc.add_geometry(Geometry::NurbsCurve(curve())).unwrap();
    let bad = NurbsCurve::try_new_rational(
        2,
        [1., -1., 3.]
            .into_iter()
            .enumerate()
            .map(|(i, w)| {
                WeightedPoint3::try_new(Point3::try_new(i as f64, 0., 0.).unwrap(), w).unwrap()
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let second = doc.add_geometry(Geometry::NurbsCurve(bad)).unwrap();
    doc.select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let history = doc.undo_label().map(str::to_owned);
    for command in [
        "ConvertToBeziers Yes",
        "ConvertToBeziers Maybe",
        "ConvertToBeziers DeleteInput=No extra",
        "ConvertToBeziers Other=Yes",
    ] {
        assert!(registry.execute(&mut doc, command).is_err());
        assert_eq!(
            doc.objects().map(|o| o.id()).collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(
            doc.selected_object_ids().collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(doc.undo_label(), history.as_deref());
    }
}

#[test]
fn selecting_only_ineligible_geometry_is_a_nonmutating_error() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    assert!(matches!(
        registry.execute(&mut doc, "ConvertToBeziers"),
        Err(CommandError::NoObjectsSelected)
    ));
    let p = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.select_objects_direct([p], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut doc, "ConvertToBeziers"),
        Err(CommandError::UnsupportedConvertToBeziersGeometry)
    ));
    assert!(doc.is_selected(p));
    assert_eq!(doc.objects().len(), 1);
}
