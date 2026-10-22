//! intersection torus quadric command regressions.
use super::*;

#[test]
fn intersect_outputs_exact_coaxial_torus_cylinder_circles() {
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
                NurbsSurface::try_cylinder(frame, 4.6, -2.0, 2.0).unwrap(),
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
            panic!("coaxial torus and cylinder should create circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_offset_torus_cylinder_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = frame.with_origin(Point3::try_new(0.5, 0.0, -2.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 4.0, 0.0, 4.0).unwrap(),
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
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("offset torus and cylinder should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_centered_perpendicular_torus_cylinder_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let origin = Point3::try_new(0.0, 0.0, 0.0).unwrap();
    let torus_frame = Frame3::try_from_normal(
        origin,
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = Frame3::try_from_normal(
        Point3::try_new(-6.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(torus_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 0.4, 0.0, 12.0).unwrap(),
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
            panic!("perpendicular torus and cylinder should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_centered_perpendicular_turning_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let torus_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = Frame3::try_from_normal(
        Point3::try_new(-6.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(torus_frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 4.0, 0.0, 12.0).unwrap(),
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
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("turned torus/cylinder section should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_axis_centered_torus_sphere_circles() {
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
                NurbsSurface::try_sphere(frame, 4.0).unwrap(),
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
            panic!("axis-centered torus and sphere should create circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_offset_torus_sphere_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let sphere_frame = Frame3::try_from_normal(
        Point3::try_new(0.5, 0.0, 0.0).unwrap(),
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
                NurbsSurface::try_sphere(sphere_frame, 4.0).unwrap(),
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
    let curves = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    for object in curves {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("offset torus and sphere should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}
