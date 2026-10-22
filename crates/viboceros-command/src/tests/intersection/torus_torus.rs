//! intersection torus torus command regressions.
use super::*;

#[test]
fn intersect_outputs_exact_coaxial_torus_torus_circles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 5.0, 1.0).unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 2 intersection object(s) from 1 object pair(s)"
    );
    let circles = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(circles.len(), 2);
    for object in circles {
        let Geometry::NurbsCurve(circle) = object.geometry() else {
            panic!("coaxial tori should create circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_equal_parallel_offset_torus_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let offset_frame = frame.with_origin(Point3::try_new(0.5, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(offset_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 4 intersection object(s) from 1 object pair(s)"
    );
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 4);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("equal offset tori should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_equal_centered_crossed_torus_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let origin = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    let first_frame = Frame3::try_from_normal(
        origin,
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        origin,
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(first_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(second_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 4 intersection object(s) from 1 object pair(s)"
    );
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 4);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("crossed tori should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_unequal_major_parallel_torus_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let offset_frame = frame.with_origin(Point3::try_new(1.0, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(offset_frame, 4.5, 1.0).unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 4 intersection object(s) from 1 object pair(s)"
    );
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 4);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("unequal major tori should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_unequal_major_parallel_torus_inner_contact() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let offset_frame = frame.with_origin(Point3::try_new(1.0, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 7.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(offset_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let mut selected = document.selected_objects();
    let Geometry::Point(contact) = selected.next().unwrap().geometry() else {
        panic!("inner torus tangency should create a point")
    };
    assert!(
        contact
            .distance_to(Point3::try_new(6.0, 0.0, 0.0).unwrap())
            .unwrap()
            < 5e-9
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}
