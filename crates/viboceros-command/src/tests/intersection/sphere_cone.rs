//! intersection sphere cone command regressions.
use super::*;

#[test]
fn intersect_outputs_exact_sphere_cone_sections() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(0.0, 0.0, 2.0).unwrap()),
                    1.5,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
    let mut heights = Vec::new();
    for object in document.selected_objects() {
        let Geometry::NurbsCurve(circle) = object.geometry() else {
            panic!("sphere/cone sections must be exact circles")
        };
        assert_eq!(circle.degree(), 2);
        assert!(circle.is_closed().unwrap());
        let sample = circle.evaluate(*circle.domain().start()).unwrap();
        assert!((sample.x().hypot(sample.y()) - 0.75 * sample.z()).abs() < 1e-9);
        assert!(
            (sample
                .distance_to(Point3::try_new(0.0, 0.0, 2.0).unwrap())
                .unwrap()
                - 1.5)
                .abs()
                < 1e-9
        );
        heights.push(sample.z());
    }
    heights.sort_by(f64::total_cmp);
    assert!((heights[0] - 0.56).abs() < 1e-9);
    assert!((heights[1] - 2.0).abs() < 1e-9);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_noncoaxial_sphere_cone_loop() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let center = Point3::try_new(0.5, 0.0, 0.5).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_sphere(frame.with_origin(center), 1.5).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
        panic!("noncoaxial sphere/cone must create a curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(curve.is_closed().unwrap());
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_sphere_cone_curve_closing_at_apex() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(2.0, 0.0, 0.5).unwrap()),
                    4.25_f64.sqrt(),
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
        panic!("sphere through cone apex must create a curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(curve.is_closed().unwrap());
    assert!(
        curve
            .evaluate(*curve.domain().start())
            .unwrap()
            .distance_to(frame.origin())
            .unwrap()
            < 1e-12
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_two_separated_sphere_cone_loops() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(0.2, 0.0, 2.0).unwrap()),
                    1.5,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
    let selected = document.selected_objects().collect::<Vec<_>>();
    assert_eq!(selected.len(), 2);
    for object in selected {
        let Geometry::NurbsCurve(curve) = object.geometry() else {
            panic!("separated sphere/cone branches must create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_nodal_sphere_cone_curve() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(0.2, 0.0, 2.0).unwrap()),
                    1.36,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
        panic!("internal sphere/cone tangency must create a curve")
    };
    assert_eq!(curve.degree(), 3);
    assert!(curve.is_closed().unwrap());
    assert!(
        curve
            .evaluate(std::f64::consts::PI)
            .unwrap()
            .distance_to(curve.evaluate(3.0 * std::f64::consts::PI).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_turning_sphere_cone_branches() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(0.5, 0.0, 2.0).unwrap()),
                    1.5,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
                panic!("turning sphere/cone must create curves")
            };
            assert_eq!(curve.degree(), 3);
            assert!(!curve.is_closed().unwrap());
            curve
        })
        .collect::<Vec<_>>();
    assert_eq!(curves.len(), 2);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_noncoaxial_sphere_cone_tangent_point() {
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
                NurbsSurface::try_sphere(
                    frame.with_origin(Point3::try_new(0.5, 0.0, 2.0).unwrap()),
                    0.8,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cone(frame, 3.0, 4.0).unwrap(),
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
        panic!("external sphere/cone tangency must create a point")
    };
    assert!(
        contact
            .distance_to(Point3::try_new(1.14, 0.0, 1.52).unwrap())
            .unwrap()
            < 5e-9
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}
