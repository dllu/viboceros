use super::*;
use viboceros_document::SelectionMode;
use viboceros_geometry::PolyCurve3;

fn composite() -> PolyCurve3 {
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    PolyCurve3::try_with_segment_domains(
        vec![
            NurbsCurve::try_clamped_uniform(
                2,
                vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)],
            )
            .unwrap(),
            NurbsCurve::try_clamped_uniform(1, vec![point(3.0, 0.0), point(4.0, 3.0)]).unwrap(),
        ],
        vec![-10.0, -3.0, 12.0],
    )
    .unwrap()
}

#[test]
fn explode_preserves_segment_domains_attributes_groups_and_undo() {
    let curve = composite();
    let mut document = Document::default();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("composite")
        .with_object_color(ColorRgb::new(12, 34, 56));
    let source = document
        .add_geometry_with_attributes(Geometry::PolyCurve(curve.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("assembly".into()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Explode").unwrap();
    assert!(document.object(source).is_none());
    assert_eq!(document.selected_objects().count(), 2);
    assert_eq!(document.group(group).unwrap().members().count(), 2);
    let mut segments = document
        .objects()
        .map(|object| {
            assert_eq!(object.attributes(), &attributes);
            let Geometry::NurbsCurve(segment) = object.geometry() else {
                panic!("expected NURBS segment")
            };
            segment
        })
        .collect::<Vec<_>>();
    segments.sort_by(|a, b| a.domain().start().total_cmp(b.domain().start()));
    for (index, segment) in segments.iter().enumerate() {
        assert_eq!(segment.domain(), curve.segment_domain(index).unwrap());
        for i in 0..=10 {
            let t = segment.parameter_at(i as f64 / 10.0).unwrap();
            assert!(
                segment
                    .evaluate(t)
                    .unwrap()
                    .distance_to(curve.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-12
            );
        }
    }
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::PolyCurve(curve)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.group(group).unwrap().members().count(), 2);
}

#[test]
fn curve_selection_division_reversal_and_conversion_accept_polycurves() {
    let curve = composite();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::PolyCurve(curve.clone()))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "SelCrv").unwrap();
    assert_eq!(document.selected_objects().count(), 1);
    registry.execute(&mut document, "Length").unwrap();
    registry
        .execute(&mut document, "Divide 4 MarkEnds")
        .unwrap();
    assert_eq!(
        document
            .objects()
            .filter(|object| matches!(object.geometry(), Geometry::Point(_)))
            .count(),
        5
    );
    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Flip").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::PolyCurve(curve.reversed().unwrap())
    );
    registry.execute(&mut document, "Undo").unwrap();
    registry
        .execute(&mut document, "ToNURBS DeleteInputObjects=Yes")
        .unwrap();
    let Geometry::NurbsCurve(result) = document.object(source).unwrap().geometry() else {
        panic!("conversion did not produce NURBS")
    };
    for i in 0..=50 {
        let t = curve.parameter_at(i as f64 / 50.0).unwrap();
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(curve.evaluate(t).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn close_curve_appends_to_open_composites_and_keeps_closed_ones() {
    let curve = composite();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::PolyCurve(curve.clone()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "CloseCrv").unwrap();
    let Geometry::PolyCurve(appended) = document.object(source).unwrap().geometry() else {
        panic!("expected closed polycurve")
    };
    assert!(appended.is_closed().unwrap());
    assert_eq!(appended.segments().len(), 3);
    assert_eq!(&appended.segments()[..2], curve.segments());
    assert_eq!(appended.parameters(), &[-10.0, -3.0, 12.0, 17.0]);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::PolyCurve(curve.clone())
    );
    let mut segments = curve.segments().to_vec();
    segments.push(
        NurbsCurve::try_clamped_uniform(
            1,
            vec![
                Point3::try_new(4.0, 3.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            ],
        )
        .unwrap(),
    );
    let closed = Geometry::PolyCurve(PolyCurve3::try_new(segments).unwrap());
    document
        .replace_object_geometries([(source, closed.clone())])
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    registry.execute(&mut document, "CloseCrv").unwrap();
    assert_eq!(document.object(source).unwrap().geometry(), &closed);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn extraction_failure_with_mixed_selection_is_atomic() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::PolyCurve(composite()))
        .unwrap();
    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(5.0, 6.0, 7.0).unwrap()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    document.select_object(point, SelectionMode::Add).unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut document, "ExtractControlPolygon")
            .is_err()
    );
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.selected_objects().count(), 2);
    assert_eq!(document.undo_label(), history.as_deref());
}
