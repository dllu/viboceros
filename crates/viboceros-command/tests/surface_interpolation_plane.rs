use viboceros_command::{
    CommandContext, CommandRegistry,
    surface_interpolation::{Plan, Support},
};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::{Frame3, Plane, Point3, Vector3};
fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
#[test]
fn plane_arguments_use_context_without_model_surfaces_and_create_one_history_edit() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let plane = Plane::new(
        point([2., 3., 4.]),
        Vector3::try_from([1., 0., 0.])
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
    );
    let context = CommandContext {
        construction_plane: Frame3::try_from_unit_normal(plane.origin(), plane.normal()).unwrap(),
    };
    registry
        .execute_in_context(
            &mut doc,
            "InterpCrvOnSrf 9,-0.005,-0.002 -3,0,0.004 8,0.005,-0.002 InfinitePlane=CPlaneXY",
            context,
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.selected_object_count(), 0);
    let Geometry::NurbsCurve(curve) = doc.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert!(
        curve
            .control_points()
            .iter()
            .all(|p| (p.point().x() - 2.).abs() < 1e-10)
    );
    assert_eq!(doc.undo_label(), Some("InterpCrvOnSrf"));
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 1);
}
#[test]
fn stale_referenced_plane_and_invalid_combined_targets_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Rectangle 0,0 10,5").unwrap();
    let id = doc.objects().next().unwrap().id();
    let support = Support::capture_plane(
        &doc,
        &format!("Object:{id}"),
        CommandContext::default().construction_plane,
    )
    .unwrap();
    let plan = Plan::prepare(
        &doc,
        support,
        &[point([0., 0., 0.]), point([3., 4., 0.])],
        false,
    )
    .unwrap();
    doc.clear_history().unwrap();
    registry.execute(&mut doc, "Point 20,30,40").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for text in [
        "InterpCrvOnSrf 0,0 2,3 InfinitePlane=WorldXY Face=0",
        "InterpCrvOnSrf 0,0 2,3 InfinitePlane=bad",
        "InterpCrvOnSrf 0,0 0,0,3 InfinitePlane=WorldXY",
    ] {
        assert!(registry.execute(&mut doc, text).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, &format!("SelID {id}")).unwrap();
    registry.execute(&mut doc, "Hide").unwrap();
    assert!(!plan.current(&doc));
    assert!(plan.apply(&mut doc).is_err());
}

#[test]
fn document_footprint_collapses_outside_inputs_without_consuming_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "Line 50,60,70 60,60,70")
        .unwrap();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let support = Support::capture_plane(
        &doc,
        "WorldXY",
        CommandContext::default().construction_plane,
    )
    .unwrap();
    let clipped = support.point(point([-4., -3., 5.])).unwrap();
    assert!(clipped.distance_to(point([49.89, 59.99, 0.])).unwrap() < 1e-12);
    assert!(
        registry
            .execute(
                &mut doc,
                "InterpCrvOnSrf -4,-3,5 -2,2,-1 0,-1.5,3 InfinitePlane=WorldXY"
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
