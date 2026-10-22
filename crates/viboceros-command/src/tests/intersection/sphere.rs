//! intersection sphere command regressions.
use super::*;

#[test]
fn intersect_outputs_exact_sphere_cylinder_sections() {
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
                    frame.with_origin(Point3::try_new(0.0, 0.0, 2.5).unwrap()),
                    2.5,
                )
                .unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap(),
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
    let mut heights = document
        .selected_objects()
        .map(|object| {
            let Geometry::NurbsCurve(circle) = object.geometry() else {
                panic!("sphere/cylinder sections must be exact circles")
            };
            assert_eq!(circle.degree(), 2);
            assert!(circle.is_closed().unwrap());
            assert!(
                (circle.length(document.tolerance()).unwrap() - 3.0 * std::f64::consts::PI).abs()
                    < 1e-7
            );
            circle.evaluate(*circle.domain().start()).unwrap().z()
        })
        .collect::<Vec<_>>();
    heights.sort_by(f64::total_cmp);
    assert!((heights[0] - 0.5).abs() < 1e-9);
    assert!((heights[1] - 4.5).abs() < 1e-9);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_noncoaxial_sphere_cylinder_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let sphere_center = Point3::try_new(0.5, 0.0, 2.5).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_sphere(frame.with_origin(sphere_center), 2.5).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(frame, 1.5, 0.0, 5.0).unwrap(),
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
            panic!("offset sphere/cylinder sections must be curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=32 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as f64 / 32.0);
            let sample = curve.evaluate(parameter).unwrap();
            assert!((sample.x().hypot(sample.y()) - 1.5).abs() < 2e-9);
            assert!((sample.distance_to(sphere_center).unwrap() - 2.5).abs() < 2e-9);
        }
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_singular_sphere_cylinder_curve() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let center = Point3::try_new(1.0, 0.0, 2.5).unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_sphere(frame.with_origin(center), 2.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(frame, 1.0, 0.0, 5.0).unwrap(),
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
    let object = document.selected_objects().next().unwrap();
    let Geometry::NurbsCurve(curve) = object.geometry() else {
        panic!("singular sphere/cylinder section must be a curve")
    };
    assert_eq!(curve.degree(), 4);
    assert!(curve.is_rational());
    assert!(curve.is_closed().unwrap());
    assert!(
        curve
            .evaluate(1.0)
            .unwrap()
            .distance_to(curve.evaluate(3.0).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_sphere_sphere_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let sphere = |center, radius| {
        let frame = Frame3::try_from_normal(
            center,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        NurbsSurface::try_sphere(frame, radius).unwrap()
    };
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(sphere(
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                2.0,
            )))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(sphere(
                Point3::try_new(2.0, 0.0, 0.0).unwrap(),
                2.0,
            )))
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
        let Geometry::NurbsCurve(arc) = object.geometry() else {
            panic!("two spheres should create semicircular arcs")
        };
        assert_eq!(arc.degree(), 2);
        assert!(!arc.is_closed().unwrap());
        assert!(
            (arc.length(document.tolerance()).unwrap() - std::f64::consts::PI * 3.0_f64.sqrt())
                .abs()
                < 1e-7
        );
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_exact_sphere_planar_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(5.0, 5.0, 1.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let input_ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_sphere(frame, 2.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(planar_intersection_surface()))
            .unwrap(),
    ];
    document
        .select_objects_direct(input_ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(circle) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("sphere and plane should create a section circle")
    };
    assert!(circle.is_closed().unwrap());
    assert!(
        (circle.length(document.tolerance()).unwrap()
            - 2.0 * std::f64::consts::PI * 3.0_f64.sqrt())
        .abs()
            < 1e-7
    );
    assert!(input_ids.iter().all(|id| document.object(*id).is_some()));
}
