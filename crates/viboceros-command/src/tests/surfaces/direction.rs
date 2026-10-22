//! surfaces direction command regressions.
use super::*;

#[test]
fn reverses_curve_direction_without_changing_identity_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 1,2 7,5").unwrap();
    registry.execute(&mut document, "Circle 10,0 3").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let Geometry::Line(original_line) = document.object(ids[0]).unwrap().geometry() else {
        panic!("expected source line")
    };
    let original_line = *original_line;
    let Geometry::Circle(original_circle) = document.object(ids[1]).unwrap().geometry() else {
        panic!("expected source circle")
    };
    let original_circle = *original_circle;
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Pair").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Rev").unwrap(),
        "Flipped 2 object(s)"
    );
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(
        document
            .group_by_name("Pair")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        ids.iter().copied().collect()
    );
    assert!(matches!(
        document.object(ids[0]).unwrap().geometry(),
        Geometry::Line(line) if line.start() == original_line.end() && line.end() == original_line.start()
    ));
    let Geometry::Circle(reversed_circle) = document.object(ids[1]).unwrap().geometry() else {
        panic!("expected reversed circle")
    };
    assert_eq!(
        reversed_circle.point_at_angle(0.0).unwrap(),
        original_circle.point_at_angle(0.0).unwrap()
    );
    assert!(
        document.tolerance().approx_eq(
            reversed_circle
                .normal()
                .unwrap()
                .as_vector()
                .dot(original_circle.normal().unwrap().as_vector())
                .unwrap(),
            -1.0
        )
    );
    assert_eq!(document.undo_label(), Some("Flip"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(ids[0]).unwrap().geometry(),
        &Geometry::Line(original_line)
    );
    assert_eq!(
        document.object(ids[1]).unwrap().geometry(),
        &Geometry::Circle(original_circle)
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert!(matches!(
        document.object(ids[0]).unwrap().geometry(),
        Geometry::Line(line) if *line == original_line.reversed()
    ));

    registry.execute(&mut document, "Point 99,99").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let point = document.objects().last().unwrap().clone();
    assert_eq!(
        registry.execute(&mut document, "Reverse").unwrap(),
        "Flipped 2 object(s)"
    );
    assert_eq!(document.object(point.id()), Some(&point));
    assert_eq!(document.selected_object_count(), 3);
    assert_eq!(
        document.object(ids[0]).unwrap().geometry(),
        &Geometry::Line(original_line)
    );
    assert_eq!(
        document.object(ids[1]).unwrap().geometry(),
        &Geometry::Circle(original_circle)
    );
}

#[test]
fn edits_surface_directions_exactly_without_changing_identity_attributes_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New DirectionTests")
        .unwrap();
    let source = rational_multi_span_surface();
    let expected_swapped = source.try_swapped_uv().unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Asymmetric rational surface");
    let id = document
        .add_geometry_with_attributes(Geometry::NurbsSurface(source.clone()), attributes.clone())
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "Group DirectionSet")
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "_Direction _SwapUV")
            .unwrap(),
        "Edited 1 object direction(s) with SwapUV"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.attributes(), &attributes);
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("DirectionSet")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    let Geometry::NurbsSurface(swapped) = object.geometry() else {
        panic!("expected a direction-edited NURBS surface")
    };
    assert_eq!(swapped, &expected_swapped);
    assert_eq!(
        (swapped.degree_u(), swapped.degree_v()),
        (source.degree_v(), source.degree_u())
    );
    assert_eq!(
        (
            swapped.control_point_count_u(),
            swapped.control_point_count_v()
        ),
        (
            source.control_point_count_v(),
            source.control_point_count_u()
        )
    );
    assert_eq!(swapped.knots_u(), source.knots_v());
    assert_eq!(swapped.knots_v(), source.knots_u());
    assert_eq!(document.undo_label(), Some("Dir"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(source.clone())
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected_swapped)
    );
}

#[test]
fn edits_multiple_surface_axes_and_exchanges_periodicity() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let rational = rational_multi_span_surface();
    let periodic = periodic_cubic_u_surface();
    let rational_id = document
        .add_geometry(Geometry::NurbsSurface(rational.clone()))
        .unwrap();
    let periodic_id = document
        .add_geometry(Geometry::NurbsSurface(periodic.clone()))
        .unwrap();
    registry.execute(&mut document, "SelAll").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Dir Mode _FlipU").unwrap(),
        "Edited 2 object direction(s) with UReverse"
    );
    assert_eq!(
        document.object(rational_id).unwrap().geometry(),
        &Geometry::NurbsSurface(rational.try_reversed_u().unwrap())
    );
    let Geometry::NurbsSurface(reversed_periodic) =
        document.object(periodic_id).unwrap().geometry()
    else {
        panic!("expected reversed periodic surface")
    };
    assert_eq!(reversed_periodic, &periodic.try_reversed_u().unwrap());
    assert!(reversed_periodic.is_periodic_u());
    assert!(!reversed_periodic.is_periodic_v());

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry.execute(&mut document, "Dir Mode=FlipV").unwrap(),
        "Edited 2 object direction(s) with VReverse"
    );
    assert_eq!(
        document.object(rational_id).unwrap().geometry(),
        &Geometry::NurbsSurface(rational.try_reversed_v().unwrap())
    );
    assert_eq!(
        document.object(periodic_id).unwrap().geometry(),
        &Geometry::NurbsSurface(periodic.try_reversed_v().unwrap())
    );

    registry.execute(&mut document, "Undo").unwrap();
    document.clear_selection();
    document
        .select_object(periodic_id, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Dir SwapUV").unwrap();
    let Geometry::NurbsSurface(swapped_periodic) = document.object(periodic_id).unwrap().geometry()
    else {
        panic!("expected transposed periodic surface")
    };
    assert_eq!(swapped_periodic, &periodic.try_swapped_uv().unwrap());
    assert!(!swapped_periodic.is_periodic_u());
    assert!(swapped_periodic.is_periodic_v());
}

#[test]
fn direction_flip_reuses_curve_reversal_and_rejects_invalid_edits_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut curve_document = Document::default();
    registry
        .execute(&mut curve_document, "Line 1,2,3 7,5,4")
        .unwrap();
    let line_id = curve_document.objects().next().unwrap().id();
    let Geometry::Line(source_line) = curve_document.object(line_id).unwrap().geometry() else {
        panic!("expected source line")
    };
    let source_line = *source_line;
    registry.execute(&mut curve_document, "SelAll").unwrap();
    assert_eq!(
        registry
            .execute(&mut curve_document, "Dir Mode=FlipNormal")
            .unwrap(),
        "Edited 1 object direction(s) with Flip"
    );
    assert_eq!(
        curve_document.object(line_id).unwrap().geometry(),
        &Geometry::Line(source_line.reversed())
    );

    let mut empty = Document::default();
    for command in [
        "Dir",
        "Dir Unknown",
        "Dir Flip extra",
        "Dir Mode",
        "Dir Mode=Flip Mode=Flip",
    ] {
        assert!(matches!(
            registry.execute(&mut empty, command),
            Err(CommandError::Usage(DIRECTION_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut empty, "Dir Flip"),
        Err(CommandError::NoObjectsSelected)
    ));

    let mut mixed = Document::default();
    let surface = rational_multi_span_surface();
    let surface_id = mixed
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    let curve_id = mixed.add_geometry(Geometry::Line(source_line)).unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    let history = mixed.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut mixed, "Dir UReverse"),
        Err(CommandError::UnsupportedDirectionEditGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(mixed.undo_label(), history.as_deref());

    mixed.clear_selection();
    mixed
        .select_object(surface_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut mixed, "Dir Flip"),
        Err(CommandError::UnsupportedDirectionEditGeometry)
    ));
    assert_eq!(
        mixed.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert!(mixed.object(curve_id).is_some());
    assert_eq!(mixed.undo_label(), history.as_deref());
}
