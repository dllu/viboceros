use super::*;
use viboceros_document::{ControlPointId, SelectionMode};
#[test]
fn sphere_fit_points_builds_sphere_controls_and_preserves_grips_through_history() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    doc.begin_transaction("sphere fit inputs").unwrap();
    let points = [
        [3., 0., 0.],
        [-3., 0., 0.],
        [0., 3., 0.],
        [0., -3., 0.],
        [0., 0., 3.],
        [0., 0., -3.],
    ]
    .map(|p| Point3::try_from(p).unwrap());
    let id = doc
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(1, points.to_vec()).unwrap(),
        ))
        .unwrap();
    doc.commit_transaction().unwrap();
    doc.enable_control_points([id]).unwrap();
    doc.select_control_points(
        (0..6).map(|index| ControlPointId { object: id, index }),
        SelectionMode::Replace,
    )
    .unwrap();
    let before = doc.object(id).unwrap().clone();
    registry.execute(&mut doc, "Sph _FitPoints").unwrap();
    assert_eq!(doc.object(id).unwrap(), &before);
    assert_eq!(doc.selected_control_points().count(), 6);
    assert_eq!(doc.undo_label(), Some("Sphere"));
    let Geometry::NurbsSurface(surface) = doc.objects().last().unwrap().geometry() else {
        panic!("missing sphere")
    };
    let (center, radius) = surface.canonical_sphere(doc.tolerance()).unwrap().unwrap();
    assert!(
        center
            .distance_to(Point3::try_new(0., 0., 0.).unwrap())
            .unwrap()
            + (radius - 3.).abs()
            < 1e-12
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.object(id).unwrap(), &before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 2);
}
#[test]
fn sphere_fit_points_failed_fits_and_invalid_options_leave_redo_intact() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for _ in 0..4 {
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
    }
    registry.execute(&mut doc, "Sphere 0,0,0 2").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let before = format!("{doc:?}");
    assert!(matches!(
        registry.execute(&mut doc, "Sphere FitPoints"),
        Err(CommandError::OperationDeclined)
    ));
    assert_eq!(format!("{doc:?}"), before);
    assert!(
        registry
            .execute(&mut doc, "Sphere FitPoints extra")
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 5);
}
