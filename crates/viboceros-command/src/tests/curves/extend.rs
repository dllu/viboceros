//! curves extend command regressions.
use super::*;

fn assert_extended_curve_matches(actual: &Geometry, expected: &NurbsCurve) {
    let Geometry::NurbsCurve(actual) = actual else {
        panic!("extension output must retain its exact NURBS representation")
    };
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(
        actual.control_points().len(),
        expected.control_points().len()
    );
    assert_eq!(actual.knots().len(), expected.knots().len());
    for (a, b) in actual.knots().iter().zip(expected.knots()) {
        assert!((a - b).abs() < 1e-12);
    }
    for (a, b) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
        assert!((a.weight() - b.weight()).abs() < 1e-12);
    }
    for i in 0..=32 {
        let fraction = i as Real / 32.;
        let a = actual
            .evaluate(actual.parameter_at(fraction).unwrap())
            .unwrap();
        let b = expected
            .evaluate(expected.parameter_at(fraction).unwrap())
            .unwrap();
        assert!(a.distance_to(b).unwrap() < 1e-12);
    }
}

#[test]
fn extend_by_length_preserves_curve_identity_attributes_groups_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Extensions")
        .unwrap();
    registry
        .execute(&mut document, "Line 0,0,0 10,0,0")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SetObjectName Natural")
        .unwrap();
    registry.execute(&mut document, "Group Extended").unwrap();
    let before = document.object(id).unwrap().clone();
    let source = before
        .geometry()
        .nurbs_curve_representation()
        .unwrap()
        .unwrap();
    let expected = source
        .try_extended_by_length(CurveExtensionSide::Both, 2.0, document.tolerance())
        .unwrap();
    let expected_smooth = expected.clone();
    let expected_arc = source
        .try_merged_circularly_by_length(CurveExtensionSide::Both, 2.0, document.tolerance())
        .unwrap();
    let expected_line = source
        .try_extended_by_length(CurveExtensionSide::Both, 2.0, document.tolerance())
        .unwrap();
    let expected_joined = source
        .try_joined_linearly_by_length(CurveExtensionSide::Both, 2.0, document.tolerance())
        .unwrap();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=Both Type=Natural Join=Merge"
            )
            .unwrap(),
        "Naturally extended the selected curve Both by length 2, merging the extension"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::NurbsCurve(expected.clone()));
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));
    assert!(
        document
            .group_by_name("Extended")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("Extend"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsCurve(expected)
    );

    registry.execute(&mut document, "Undo").unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=Both Type=Smooth Join=Merge"
            )
            .unwrap(),
        "Smoothly extended the selected curve Both by length 2, merging the extension"
    );
    let object = document.object(id).unwrap();
    assert_extended_curve_matches(object.geometry(), &expected_smooth);
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));

    registry.execute(&mut document, "Undo").unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=Both Type=Arc Join=Merge"
            )
            .unwrap(),
        "Circularly extended the selected curve Both by length 2, merging the extension"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::NurbsCurve(expected_arc));
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));

    registry.execute(&mut document, "Undo").unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=Both Type=Line Join=Merge"
            )
            .unwrap(),
        "Linearly extended the selected curve Both by length 2, merging the extension"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::NurbsCurve(expected_line));
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));

    registry.execute(&mut document, "Undo").unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=Both Type=Line Join=Yes"
            )
            .unwrap(),
        "Linearly extended the selected curve Both by length 2, joining the extension"
    );
    let object = document.object(id).unwrap();
    assert_extended_curve_matches(object.geometry(), &expected_joined);
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));
}

#[test]
fn extend_join_no_creates_ungrouped_attribute_preserving_curve_segments() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New SeparateCurveExtensions")
        .unwrap();
    let source = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 2.0, 0.0).unwrap(),
            Point3::try_new(3.0, 1.0, 0.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let source_id = document
        .add_geometry_with_attributes(
            Geometry::NurbsCurve(source.clone()),
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("SeparateExtension"),
        )
        .unwrap();
    document
        .add_group(Some("OnlyTheSource".to_owned()), [source_id])
        .unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    let source_before = document.object(source_id).unwrap().clone();
    let expected_lines = source
        .try_separate_linear_extensions_by_length(
            CurveExtensionSide::Both,
            2.0,
            document.tolerance(),
        )
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Extend Length=2 Side=Both Type=Line Join=No")
            .unwrap(),
        "Created 2 separate linear curve extensions at Both by length 2"
    );
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    let outputs = document
        .objects()
        .filter(|object| object.id() != source_id)
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for (output, expected) in outputs.iter().zip(expected_lines) {
        assert_eq!(output.geometry(), &Geometry::NurbsCurve(expected));
        assert_eq!(output.attributes(), source_before.attributes());
        assert!(!document.is_selected(output.id()));
    }
    assert!(!document.is_selected(source_id));
    assert_eq!(
        document
            .group_by_name("OnlyTheSource")
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source_id]
    );
    assert_eq!(document.undo_label(), Some("Extend"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();

    let expected_natural = source
        .try_separate_natural_extensions_by_length(
            CurveExtensionSide::End,
            2.0,
            document.tolerance(),
        )
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=End Type=Natural Join=No"
            )
            .unwrap(),
        "Created 1 separate natural curve extension at End by length 2"
    );
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    let output = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    assert_extended_curve_matches(output.geometry(), &expected_natural);
    assert_eq!(output.attributes(), source_before.attributes());
    assert!(!document.is_selected(source_id));
    assert!(!document.is_selected(output.id()));
    assert_eq!(
        document
            .group_by_name("OnlyTheSource")
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source_id]
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    let expected_joined = source
        .try_joined_naturally_by_length(CurveExtensionSide::End, 2.0, document.tolerance())
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "Extend Length=2 Side=End Type=Natural Join=Yes"
            )
            .unwrap(),
        "Naturally extended the selected curve End by length 2, joining the extension"
    );
    assert_eq!(document.objects().count(), 1);
    let object = document.object(source_id).unwrap();
    assert_extended_curve_matches(object.geometry(), &expected_joined);
    assert_eq!(object.attributes(), source_before.attributes());
    assert!(!document.is_selected(source_id));
    assert_eq!(
        document
            .group_by_name("OnlyTheSource")
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source_id]
    );
}

#[test]
fn extend_to_selected_curve_boundary_uses_the_picked_source_end() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 5,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    registry.execute(&mut document, "Line 10,-5 10,5").unwrap();
    let boundary_id = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap()
        .id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName BoundaryExtendSource")
        .unwrap();
    registry
        .execute(&mut document, "Group BoundaryExtendGroup")
        .unwrap();
    document
        .select_objects([source_id, boundary_id], SelectionMode::Replace)
        .unwrap();
    let source_before = document.object(source_id).unwrap().clone();
    let boundary_before = document.object(boundary_id).unwrap().clone();

    assert_eq!(
        registry
            .execute(&mut document, "Extend 4.9,0 Type=Line Join=Merge",)
            .unwrap(),
        "Linearly extended the selected curve End to the nearest boundary, merging the extension"
    );
    let Geometry::NurbsCurve(extended) = document.object(source_id).unwrap().geometry() else {
        panic!("boundary Extend must replace the source with exact NURBS geometry")
    };
    assert!(
        document
            .tolerance()
            .approx_eq(*extended.domain().start(), 0.0)
    );
    assert!(
        document
            .tolerance()
            .approx_eq(*extended.domain().end(), 10.0)
    );
    assert!(
        extended
            .evaluate(*extended.domain().end())
            .unwrap()
            .is_near(
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                document.tolerance()
            )
    );
    assert_eq!(
        document.object(source_id).unwrap().attributes(),
        source_before.attributes()
    );
    assert_eq!(document.object(boundary_id).unwrap(), &boundary_before);
    assert!(
        document
            .group_by_name("BoundaryExtendGroup")
            .unwrap()
            .members()
            .any(|member| member == source_id)
    );
    assert_eq!(document.selected_object_count(), 0);
    assert_eq!(document.undo_label(), Some("Extend"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    assert_eq!(document.object(boundary_id).unwrap(), &boundary_before);
    document
        .select_objects([source_id, boundary_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "Extend 5,0 Type=Line Join=No")
            .unwrap(),
        "Created 1 separate linear curve extension from the selected End to the nearest boundary"
    );
    assert_eq!(document.object(source_id).unwrap(), &source_before);
    let extension = document
        .objects()
        .find(|object| object.id() != source_id && object.id() != boundary_id)
        .unwrap();
    assert_eq!(extension.attributes(), source_before.attributes());
    assert!(
        !document
            .group_by_name("BoundaryExtendGroup")
            .unwrap()
            .members()
            .any(|member| member == extension.id())
    );
    let Geometry::NurbsCurve(extension) = extension.geometry() else {
        panic!("Join=No must create a NURBS extension curve")
    };
    assert!(extension.evaluate(0.0).unwrap().is_near(
        Point3::try_new(5.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(extension.evaluate(1.0).unwrap().is_near(
        Point3::try_new(10.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
}

#[test]
fn extend_to_selected_surface_boundary_preserves_the_boundary() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 5,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(10.0, -5.0, -5.0).unwrap(),
        Point3::try_new(10.0, 5.0, -5.0).unwrap(),
        Point3::try_new(10.0, 5.0, 5.0).unwrap(),
        Point3::try_new(10.0, -5.0, 5.0).unwrap(),
    ])
    .unwrap();
    let boundary_id = document
        .add_geometry(Geometry::NurbsSurface(surface.clone()))
        .unwrap();
    document
        .select_objects([source_id, boundary_id], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "Extend 5,0 Type=Line Join=Merge")
        .unwrap();
    let Geometry::NurbsCurve(extended) = document.object(source_id).unwrap().geometry() else {
        panic!("surface-boundary Extend must replace the source with a NURBS curve")
    };
    assert!(
        extended
            .evaluate(*extended.domain().end())
            .unwrap()
            .is_near(
                Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                document.tolerance()
            )
    );
    assert_eq!(
        document.object(boundary_id).unwrap().geometry(),
        &Geometry::NurbsSurface(surface)
    );
    assert_eq!(document.selected_object_count(), 0);
}

#[test]
fn boundary_extend_rejects_missing_or_unreachable_boundaries_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 5,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Extend 5,0"),
        Err(CommandError::ExtendRequiresSourceAndBoundaryObjects { actual: 1 })
    ));

    registry.execute(&mut document, "Line 2,-5 2,5").unwrap();
    let boundary_id = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap()
        .id();
    document
        .select_objects([source_id, boundary_id], SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut document, "Extend 5,0 Type=Line"),
        Err(CommandError::Geometry(
            GeometryError::CurveExtensionBoundaryNotFound
        ))
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert!(matches!(
        registry.execute(&mut document, "Extend 5,0 Side=End"),
        Err(CommandError::Usage(EXTEND_CURVE_USAGE))
    ));
}

#[test]
fn extend_accepts_an_exact_domain_and_defaults_length_to_the_end() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Extend Domain=-5,15")
            .unwrap(),
        "Naturally extended the selected curve to include domain -5..15, merging the extension"
    );
    let Geometry::NurbsCurve(curve) = document.object(id).unwrap().geometry() else {
        panic!("Extend must create exact NURBS geometry")
    };
    assert_eq!(curve.domain(), -5.0..=15.0);
    assert!(curve.evaluate(-5.0).unwrap().is_near(
        Point3::try_new(-5.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(curve.evaluate(15.0).unwrap().is_near(
        Point3::try_new(15.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));

    document.select_object(id, SelectionMode::Replace).unwrap();
    assert_eq!(
        registry.execute(&mut document, "Extend Length=5").unwrap(),
        "Naturally extended the selected curve End by length 5, merging the extension"
    );
    let Geometry::NurbsCurve(curve) = document.object(id).unwrap().geometry() else {
        panic!("Extend must retain exact NURBS geometry")
    };
    assert_eq!(*curve.domain().start(), -5.0);
    assert!(document.tolerance().approx_eq(*curve.domain().end(), 20.0));
    assert!(curve.evaluate(*curve.domain().end()).unwrap().is_near(
        Point3::try_new(20.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
}

#[test]
fn extend_rejects_invalid_requests_selection_and_closed_curves_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Extend",
        "Extend Length",
        "Extend Domain=0",
        "Extend Domain=0,1 Side=End",
        "Extend Domain=0,1 Type=Arc",
        "Extend Domain=0,1 Type=Line",
        "Extend Domain=0,1 Join=No",
        "Extend Domain=0,1 Join=Yes",
        "Extend Length=1 Side=Middle",
        "Extend Length=1 Side=End Side=Start",
        "Extend Length=1 Type=Bezier",
        "Extend Length=1 Join=Maybe",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(EXTEND_CURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Extend Length=1"),
        Err(CommandError::ExtendRequiresOneCurve { actual: 0 })
    ));

    registry.execute(&mut document, "Circle 0,0 2").unwrap();
    let circle = document.objects().next().unwrap().id();
    document
        .select_object(circle, SelectionMode::Replace)
        .unwrap();
    let before = document.object(circle).unwrap().clone();
    assert!(matches!(
        registry.execute(&mut document, "Extend Length=1"),
        Err(CommandError::Geometry(
            GeometryError::CurveExtensionMustBeOpen
        ))
    ));
    assert_eq!(document.object(circle).unwrap(), &before);

    document.clear_selection();
    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    registry.execute(&mut document, "Line 0,1 1,1").unwrap();
    registry.execute(&mut document, "SelLine").unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Extend Length=1"),
        Err(CommandError::ExtendRequiresOneCurve { actual: 2 })
    ));
    let line = document
        .objects()
        .find(|object| matches!(object.geometry(), Geometry::Line(_)))
        .unwrap()
        .id();
    document
        .select_object(line, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Extend Length=0"),
        Err(CommandError::Geometry(
            GeometryError::InvalidCurveExtensionLength
        ))
    ));
}
