//! curves subcurve command regressions.
use super::*;

#[test]
fn subcurve_replaces_a_curve_without_losing_document_state() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Subcurves")
        .unwrap();
    registry
        .execute(&mut document, "Line 0,0,0 10,0,0")
        .unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "SetObjectName Shortened")
        .unwrap();
    registry.execute(&mut document, "Group Directed").unwrap();
    let before = document.object(id).unwrap().clone();
    let expected = LineSegment::try_new(
        Point3::try_new(0.2, 0.0, 0.0).unwrap(),
        Point3::try_new(0.8, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap()
    .try_reparameterized(0.2..=0.8)
    .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SubCrv Parameter=0.2,0.8")
            .unwrap(),
        "Created a directed subcurve from parameter 0.2 to 0.8, replacing the input"
    );
    let object = document.object(id).unwrap();
    assert_eq!(object.geometry(), &Geometry::Line(expected));
    assert_eq!(object.attributes(), before.attributes());
    assert!(!document.is_selected(id));
    assert!(
        document
            .group_by_name("Directed")
            .unwrap()
            .members()
            .any(|member| member == id)
    );
    assert_eq!(document.undo_label(), Some("SubCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.object(id).unwrap(), &before);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        document.object(id).unwrap().geometry(),
        &Geometry::Line(expected)
    );
}

#[test]
fn subcurve_point_order_retains_open_orientation_and_copy_retains_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 10,0").unwrap();
    let source_id = document.objects().next().unwrap().id();
    document
        .select_object(source_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "SetObjectName DirectedCopy")
        .unwrap();
    registry
        .execute(&mut document, "Group SubcurveCopies")
        .unwrap();
    let source = document.object(source_id).unwrap().clone();

    registry
        .execute(&mut document, "SubCrv 8,0 2,0 Copy=Yes")
        .unwrap();
    assert_eq!(document.objects().count(), 2);
    assert_eq!(document.object(source_id).unwrap(), &source);
    let copy = document
        .objects()
        .find(|object| object.id() != source_id)
        .unwrap();
    let copy_id = copy.id();
    assert_eq!(copy.attributes(), source.attributes());
    let Geometry::Line(curve) = copy.geometry() else {
        panic!("SubCrv must retain the native line")
    };
    assert!(curve.evaluate(*curve.domain().start()).unwrap().is_near(
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(curve.evaluate(*curve.domain().end()).unwrap().is_near(
        Point3::try_new(8.0, 0.0, 0.0).unwrap(),
        document.tolerance()
    ));
    assert!(!document.is_selected(source_id));
    assert!(document.is_selected(copy_id));
    let members = document
        .group_by_name("SubcurveCopies")
        .unwrap()
        .members()
        .collect::<BTreeSet<_>>();
    assert_eq!(members, BTreeSet::from([source_id, copy_id]));
    assert_eq!(document.undo_label(), Some("SubCrv"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().count(), 1);
    assert_eq!(document.object(source_id).unwrap(), &source);
}

#[test]
fn subcurve_crosses_a_closed_curve_seam_in_pick_order() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 5").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();

    registry
        .execute(&mut document, "SubCrv Parameter=5,1 Copy=No")
        .unwrap();
    let Geometry::PolyCurve(curve) = document.object(id).unwrap().geometry() else {
        panic!("a seam-crossing SubCrv must retain both native arc pieces");
    };
    assert_eq!(curve.segments().len(), 2);
    assert!(
        curve
            .segments()
            .iter()
            .all(|s| matches!(s, viboceros_geometry::CurveSegment3::Arc(_)))
    );
    assert_eq!(curve.domain(), 5.0..=5.0 * std::f64::consts::TAU + 1.0);
    for (t, angle) in [(5.0, 1.0_f64), (*curve.domain().end(), 0.2_f64)] {
        assert!(
            curve
                .evaluate(t)
                .unwrap()
                .distance_to(Point3::try_new(5.0 * angle.cos(), 5.0 * angle.sin(), 0.0).unwrap())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn subcurve_rejects_invalid_or_ambiguous_requests_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in [
        "SubCrv",
        "SubCrv Parameter",
        "SubCrv Parameter=0.2",
        "SubCrv Parameter=0.2,0.8 Copy=Maybe",
        "SubCrv 0,0 1,0 Copy=No Copy=Yes",
    ] {
        assert!(matches!(
            registry.execute(&mut document, command),
            Err(CommandError::Usage(SUBCURVE_USAGE))
        ));
    }
    assert!(matches!(
        registry.execute(&mut document, "SubCrv Parameter=0.2,0.8"),
        Err(CommandError::SubcurveRequiresOneCurve { actual: 0 })
    ));

    registry.execute(&mut document, "Line 0,0 2,0").unwrap();
    registry.execute(&mut document, "Line 0,1 2,1").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SubCrv Parameter=0.2,0.8"),
        Err(CommandError::SubcurveRequiresOneCurve { actual: 2 })
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let first = before[0].id();
    document
        .select_object(first, SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "SubCrv Parameter=0.5,0.5"),
        Err(CommandError::Geometry(
            GeometryError::InvalidCurveTrimInterval
        ))
    ));
    assert_eq!(document.object(first).unwrap(), &before[0]);
    assert_eq!(document.undo_label(), history.as_deref());
}
