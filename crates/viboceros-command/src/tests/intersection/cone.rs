//! intersection cone command regressions.
use super::*;

#[test]
fn intersect_creates_both_exact_saddle_plane_branches() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let saddle = NurbsSurface::try_bilinear([
        point(-1.0, -1.0, 1.0),
        point(1.0, -1.0, -1.0),
        point(1.0, 1.0, 1.0),
        point(-1.0, 1.0, -1.0),
    ])
    .unwrap();
    let plane = NurbsSurface::try_bilinear([
        point(-2.0, -2.0, 0.25),
        point(2.0, -2.0, 0.25),
        point(2.0, 2.0, 0.25),
        point(-2.0, 2.0, 0.25),
    ])
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(saddle))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(plane))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 2 intersection object(s) from 1 object pair(s)"
    );
    let output = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(output.len(), 2);
    assert!(ids.iter().all(|id| !document.is_selected(*id)));
    for object in output {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("saddle intersection must create a NURBS conic")
        };
        assert_eq!(curve.degree(), 2);
        let midpoint = (*curve.domain().start() + *curve.domain().end()) * 0.5;
        let point = curve.evaluate(midpoint).unwrap();
        assert!((point.x() * point.y() - 0.25).abs() < 1e-12);
        assert!((point.z() - 0.25).abs() < 1e-12);
    }
}

#[test]
fn intersect_outputs_exact_cone_cylinder_section() {
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
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(frame, 1.5, 0.0, 4.0).unwrap(),
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
    let Geometry::NurbsCurve(circle) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("coaxial cone and cylinder should produce an exact circle")
    };
    assert_eq!(circle.degree(), 2);
    assert!(circle.is_closed().unwrap());
    assert!(
        (circle.length(document.tolerance()).unwrap() - 3.0 * std::f64::consts::PI).abs() < 1e-8
    );
    assert!((circle.evaluate(*circle.domain().start()).unwrap().z() - 2.0).abs() < 1e-9);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_perpendicular_cone_cylinder_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let cone_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = Frame3::try_from_normal(
        Point3::try_new(-4.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(cone_frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 2.0, 0.0, 8.0).unwrap(),
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
    let Geometry::NurbsCurve(curve) = document.selected_objects().next().unwrap().geometry() else {
        panic!("perpendicular cone and cylinder should produce a curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(curve.is_closed().unwrap());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_coaxial_cone_cone_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(first_frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(second_frame, 3.0, 4.0).unwrap(),
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
    let Geometry::NurbsCurve(circle) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("coaxial cones should create a circle")
    };
    assert_eq!(circle.degree(), 2);
    assert!(circle.is_closed().unwrap());
    assert!((circle.evaluate(*circle.domain().start()).unwrap().z() - 2.0).abs() < 1e-9);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_parallel_offset_cone_cone_sections() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = first_frame.with_origin(Point3::try_new(1.0, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(first_frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(second_frame, 3.0, 4.0).unwrap(),
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
    let curves = document
        .selected_objects()
        .map(|object| {
            let Geometry::NurbsCurve(curve) = object.geometry() else {
                panic!("offset cones should produce conic curves")
            };
            assert!(!curve.is_closed().unwrap());
            curve
        })
        .collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_unequal_parallel_cone_cone_branches() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(1.0, 0.0, 4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(first_frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(second_frame, 2.0, 4.0).unwrap(),
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
    for object in document.selected_objects() {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("unequal parallel cones should produce curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(!curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_shared_apex_cone_generators() {
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
                NurbsSurface::try_cone(first_frame, 8.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(second_frame, 6.0, 3.0).unwrap(),
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
    let lines = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    for object in lines {
        let Geometry::NurbsCurve(line) = object.geometry() else {
            panic!("shared-apex cones should create line segments")
        };
        assert_eq!(line.degree(), 1);
        assert!(
            line.evaluate(*line.domain().start())
                .unwrap()
                .distance_to(origin)
                .unwrap()
                < 1e-9
        );
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_coaxial_torus_cone_circles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let cone_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let torus_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_torus(torus_frame, 3.0, 1.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(cone_frame, 6.0, 8.0).unwrap(),
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
            panic!("coaxial torus and cone should create circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_offset_parallel_cone_cylinder_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = frame.with_origin(Point3::try_new(0.5, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 1.5, 0.0, 4.0).unwrap(),
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
    let Geometry::NurbsCurve(curve) = selected.next().unwrap().geometry() else {
        panic!("offset parallel cone and cylinder must create a curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(curve.is_closed().unwrap());
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_cone_cylinder_apex_corner() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cylinder_frame = frame.with_origin(Point3::try_new(1.5, 0.0, 0.0).unwrap());
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(cylinder_frame, 1.5, 0.0, 4.0).unwrap(),
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
    let Geometry::NurbsCurve(curve) = selected.next().unwrap().geometry() else {
        panic!("cone/cylinder apex crossing must create a curve")
    };
    assert!(curve.is_closed().unwrap());
    assert!(
        curve
            .evaluate(std::f64::consts::PI)
            .unwrap()
            .distance_to(frame.origin())
            .unwrap()
            < 1e-12
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}
