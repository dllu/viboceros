//! intersection torus plane command regressions.
use super::*;

#[test]
fn intersect_outputs_exact_torus_plane_circles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |x, y| Point3::try_new(x, y, 0.0).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -6.0),
                    point(6.0, -6.0),
                    point(6.0, 6.0),
                    point(-6.0, 6.0),
                ])
                .unwrap(),
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
            panic!("torus/plane section should create circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_parallel_offset_torus_plane_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |y, z| Point3::try_new(1.0, y, z).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -2.0),
                    point(6.0, -2.0),
                    point(6.0, 2.0),
                    point(-6.0, 2.0),
                ])
                .unwrap(),
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
    let loops = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(loops.len(), 2);
    for object in loops {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("parallel offset torus/plane section should create loops")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_pinched_torus_plane_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |y, z| Point3::try_new(3.0, y, z).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -2.0),
                    point(6.0, -2.0),
                    point(6.0, 2.0),
                    point(-6.0, 2.0),
                ])
                .unwrap(),
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
    let loops = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(loops.len(), 2);
    for object in loops {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("inner tangent torus/plane section should create pinched loops")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
}

#[test]
fn intersect_outputs_oblique_torus_plane_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |x, y| Point3::try_new(x, y, 0.1 * x).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -6.0),
                    point(6.0, -6.0),
                    point(6.0, 6.0),
                    point(-6.0, 6.0),
                ])
                .unwrap(),
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
    let loops = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(loops.len(), 2);
    for object in loops {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("oblique torus/plane section should create loops")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
}

#[test]
fn intersect_outputs_shallow_tilted_torus_plane_loops() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |y: f64, z: f64| Point3::try_new(0.3 - 1.0e-5 * z, y, z).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -2.0),
                    point(6.0, -2.0),
                    point(6.0, 2.0),
                    point(-6.0, 2.0),
                ])
                .unwrap(),
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
            panic!("shallow tilted torus/plane section should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_oblique_torus_plane_tangent_point() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let point = |x, y| Point3::try_new(x, y, 0.75_f64.mul_add(x, 4.25)).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-6.0, -6.0),
                    point(6.0, -6.0),
                    point(6.0, 6.0),
                    point(-6.0, 6.0),
                ])
                .unwrap(),
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
        panic!("oblique torus/plane tangency should create a point")
    };
    assert!(
        contact
            .distance_to(Point3::try_new(-4.6, 0.0, 0.8).unwrap())
            .unwrap()
            < 5e-9
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}
