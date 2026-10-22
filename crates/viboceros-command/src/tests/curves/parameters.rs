//! curves parameters command regressions.
use super::*;

#[test]
fn reparameterizes_a_curve_without_losing_document_state() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Domains")
        .unwrap();
    registry
        .execute(
            &mut document,
            "ControlPointCurve 3 0,0,0 2,-1,1 5,2,0 7,4,2",
        )
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SetObjectName ReparameterizedCurve")
        .unwrap();
    registry.execute(&mut document, "Group Domains").unwrap();
    let before = document.object(id).unwrap().clone();
    let Geometry::NurbsCurve(source) = before.geometry() else {
        panic!("ControlPointCurve must create a NURBS curve")
    };
    let expected = source.try_reparameterized(-4.0..=6.0).unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Reparameterize -4,6")
            .unwrap(),
        "Reparameterized the selected curve domain to -4..6"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::NurbsCurve(expected.clone()));
    assert_eq!(object.attributes(), before.attributes());
    assert!(document.is_selected(id));
    assert!(
        document
            .group_by_name("Domains")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("Reparameterize"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::NurbsCurve(expected)
    );
}

#[test]
fn reparameterize_automatic_uses_curve_length_and_planar_surface_size() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0,0 3,4,0").unwrap();
    let line_id = document.objects().next().unwrap().id();
    document
        .select_object(line_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "Reparameterize _Automatic")
            .unwrap(),
        "Reparameterized the selected curve domain to 0..5"
    );
    let Geometry::Line(line) = document.object(line_id).unwrap().geometry() else {
        panic!("reparameterization must retain the native line")
    };
    assert_eq!(line.domain(), 0.0..=5.0);

    document.clear_selection();
    registry
        .execute(&mut document, "SrfPt 0,0,0 3,4,0 3,4,12 0,0,12")
        .unwrap();
    let surface_id = document.objects().last().unwrap().id();
    document
        .select_object(surface_id, SelectionMode::Replace)
        .unwrap();
    let Geometry::NurbsSurface(source) = document.object(surface_id).unwrap().geometry() else {
        panic!("SrfPt must create a NURBS surface")
    };
    let [width, height] = source.estimated_size().unwrap();
    let expected = source
        .try_reparameterized(0.0..=width, 0.0..=height)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "Reparameterize Automatic")
            .unwrap(),
        format!("Reparameterized the selected surface domains to U 0..{width}, V 0..{height}")
    );
    assert_eq!(
        document.object(surface_id).unwrap().geometry(),
        &Geometry::NurbsSurface(expected)
    );
}

#[test]
fn reparameterizes_both_surface_domains_explicitly() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Sphere 0,0,0 5").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    let Geometry::NurbsSurface(source) = document.object(id).unwrap().geometry() else {
        panic!("Sphere must create a NURBS surface")
    };
    let source = source.clone();
    let expected = source.try_reparameterized(-2.0..=8.0, 10.0..=13.0).unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Reparameterize -2 8 10 13")
            .unwrap(),
        "Reparameterized the selected surface domains to U -2..8, V 10..13"
    );
    let Geometry::NurbsSurface(mapped) = document.object(id).unwrap().geometry() else {
        panic!("Reparameterize must retain NURBS surface geometry")
    };
    assert_eq!(mapped, &expected);
    for u_index in 0..=8 {
        for v_index in 0..=8 {
            let normalized_u = u_index as Real / 8.0;
            let normalized_v = v_index as Real / 8.0;
            assert!(
                mapped
                    .evaluate(
                        mapped.parameter_at_u(normalized_u).unwrap(),
                        mapped.parameter_at_v(normalized_v).unwrap(),
                    )
                    .unwrap()
                    .is_near(
                        source
                            .evaluate(
                                source.parameter_at_u(normalized_u).unwrap(),
                                source.parameter_at_v(normalized_v).unwrap(),
                            )
                            .unwrap(),
                        document.tolerance(),
                    )
            );
        }
    }
}

#[test]
fn reparameterize_rejects_invalid_or_ambiguous_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "Reparameterize",
        "Reparameterize Automatic extra",
        "Reparameterize 0 1 2",
        "Reparameterize 0,,1",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(REPARAMETERIZE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "Reparameterize 0 1"),
        Err(CommandError::ReparameterizeRequiresOneObject { actual: 0 })
    ));

    registry.execute(&mut document, "Line 0,0 2,0").unwrap();
    let line_id = document.objects().next().unwrap().id();
    registry
        .execute(&mut document, "SrfPt 0,0 2,0 2,2 0,2")
        .unwrap();
    let surface_id = document
        .objects()
        .find(|object| object.id() != line_id)
        .unwrap()
        .id();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Reparameterize Automatic"),
        Err(CommandError::ReparameterizeMixedGeometry)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    document
        .select_object(line_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Reparameterize 0 1 2 3"),
        Err(CommandError::Usage(REPARAMETERIZE_USAGE))
    ));
    assert!(matches!(
        registry.execute(&mut document, "Reparameterize 2 1"),
        Err(CommandError::Geometry(
            GeometryError::InvalidCurveParameterInterval
        ))
    ));
    assert_eq!(
        document.object(line_id).unwrap(),
        before.iter().find(|object| object.id() == line_id).unwrap()
    );

    document
        .select_object(surface_id, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "Reparameterize 0 1"),
        Err(CommandError::Usage(REPARAMETERIZE_USAGE))
    ));
    assert_eq!(
        document.object(surface_id).unwrap(),
        before
            .iter()
            .find(|object| object.id() == surface_id)
            .unwrap()
    );
    assert_eq!(document.undo_label(), history.as_deref());
}
