//! intersection cylinder command regressions.
use super::*;

#[test]
fn intersect_outputs_parallel_cylinder_generatrices() {
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
                NurbsSurface::try_cylinder(frame, 2.0, 0.0, 5.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(
                    frame.with_origin(Point3::try_new(2.0, 0.0, 1.0).unwrap()),
                    2.0,
                    0.0,
                    3.0,
                )
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
    let mut sides = Vec::new();
    for object in document.selected_objects() {
        let Geometry::NurbsCurve(line) = object.geometry() else {
            panic!("parallel cylinders should create exact line intersections")
        };
        assert_eq!(line.degree(), 1);
        assert!((line.length(document.tolerance()).unwrap() - 3.0).abs() < 1e-9);
        let sample = line.evaluate(*line.domain().start()).unwrap();
        assert!((sample.x() - 1.0).abs() < 1e-9);
        sides.push(sample.y());
    }
    sides.sort_by(f64::total_cmp);
    assert!((sides[0] + 3.0_f64.sqrt()).abs() < 1e-9);
    assert!((sides[1] - 3.0_f64.sqrt()).abs() < 1e-9);
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_crossed_cylinder_ellipses() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -2.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-2.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 4.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 1.0, 0.0, 4.0).unwrap(),
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
            panic!("crossed cylinders should create ellipses")
        };
        assert_eq!(curve.degree(), 2);
        assert!(curve.is_rational());
        assert!(curve.is_closed().unwrap());
        for index in 0..=32 {
            let domain = curve.domain();
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as f64 / 32.0);
            let sample = curve.evaluate(parameter).unwrap();
            for frame in [first_frame, second_frame] {
                let local = frame.coordinates_of(sample).unwrap();
                assert!((local[0].hypot(local[1]) - 1.0).abs() < 2e-12);
            }
        }
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_unequal_crossed_cylinder_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -3.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-3.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 6.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 6.0).unwrap(),
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
            panic!("unequal crossed cylinders should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=32 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as f64 / 32.0);
            let sample = curve.evaluate(parameter).unwrap();
            for (frame, radius) in [(first_frame, 1.0), (second_frame, 2.0)] {
                let local = frame.coordinates_of(sample).unwrap();
                assert!((local[0].hypot(local[1]) - radius).abs() < 2e-9);
            }
        }
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_oblique_unequal_cylinder_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let (sine, cosine) = std::f64::consts::FRAC_PI_3.sin_cos();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-4.0 * sine, 0.0, -4.0 * cosine).unwrap(),
        Vector3::try_new(sine, 0.0, cosine).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap(),
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
            panic!("oblique unequal cylinders should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
        let domain = curve.domain();
        for index in 0..=32 {
            let parameter =
                *domain.start() + (*domain.end() - *domain.start()) * (index as f64 / 32.0);
            let sample = curve.evaluate(parameter).unwrap();
            for (frame, radius) in [(first_frame, 1.0), (second_frame, 2.0)] {
                let local = frame.coordinates_of(sample).unwrap();
                assert!((local[0].hypot(local[1]) - radius).abs() < 2e-9);
            }
        }
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_separated_skew_cylinder_curves() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-4.0, 0.5, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap(),
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
            panic!("separated skew cylinders should create curves")
        };
        assert_eq!(curve.degree(), 3);
        assert!(curve.is_closed().unwrap());
    }
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_joined_skew_cylinder_curve() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-4.0, 1.5, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap(),
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
        panic!("joined skew cylinders should create a curve")
    };
    assert!(curve.is_closed().unwrap());
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_nodal_skew_cylinder_curve() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let first_frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, -4.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let second_frame = Frame3::try_from_normal(
        Point3::try_new(-4.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let ids = [
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(first_frame, 1.0, 0.0, 8.0).unwrap(),
            ))
            .unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_cylinder(second_frame, 2.0, 0.0, 8.0).unwrap(),
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
        panic!("internal skew tangency should create a curve")
    };
    assert!(curve.is_closed().unwrap());
    assert!(
        curve
            .evaluate(0.0)
            .unwrap()
            .distance_to(curve.evaluate(std::f64::consts::TAU).unwrap())
            .unwrap()
            < 1e-9
    );
    assert!(selected.next().is_none());
    assert!(ids.iter().all(|id| document.object(*id).is_some()));
}

#[test]
fn intersect_outputs_two_cylinder_plane_generatrices() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
    let plane = NurbsSurface::try_bilinear([
        Point3::try_new(0.0, -3.0, -1.0).unwrap(),
        Point3::try_new(0.0, 3.0, -1.0).unwrap(),
        Point3::try_new(0.0, 3.0, 5.0).unwrap(),
        Point3::try_new(0.0, -3.0, 5.0).unwrap(),
    ])
    .unwrap();
    let ids = [
        document.add_geometry(Geometry::NurbsSurface(wall)).unwrap(),
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
    for object in document.selected_objects() {
        let Geometry::NurbsCurve(line) = object.geometry() else {
            panic!("cylinder section should consist of lines")
        };
        assert_eq!(line.degree(), 1);
        assert!((line.length(document.tolerance()).unwrap() - 4.0).abs() < 1e-9);
    }
}

#[test]
fn intersect_retains_rhino_tangent_cylinder_line_multiplicity() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
    let tangent = NurbsSurface::try_bilinear([
        Point3::try_new(2.0, -3.0, -1.0).unwrap(),
        Point3::try_new(2.0, 3.0, -1.0).unwrap(),
        Point3::try_new(2.0, 3.0, 5.0).unwrap(),
        Point3::try_new(2.0, -3.0, 5.0).unwrap(),
    ])
    .unwrap();
    let ids = [
        document.add_geometry(Geometry::NurbsSurface(wall)).unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(tangent))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 2 intersection object(s) from 1 object pair(s)"
    );
    let selected = document
        .selected_objects()
        .map(|object| object.geometry().clone())
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0], selected[1]);
}

#[test]
fn intersect_outputs_exact_oblique_cylinder_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let wall = NurbsSurface::try_cylinder(frame, 2.0, 0.0, 4.0).unwrap();
    let tilted = NurbsSurface::try_bilinear([
        Point3::try_new(-3.0, -3.0, 0.5).unwrap(),
        Point3::try_new(3.0, -3.0, 3.5).unwrap(),
        Point3::try_new(3.0, 3.0, 3.5).unwrap(),
        Point3::try_new(-3.0, 3.0, 0.5).unwrap(),
    ])
    .unwrap();
    let ids = [
        document.add_geometry(Geometry::NurbsSurface(wall)).unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(tilted))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(ellipse) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("oblique cylinder section should be an ellipse")
    };
    assert_eq!(ellipse.degree(), 2);
    assert!(ellipse.is_closed().unwrap());
    assert!((ellipse.length(document.tolerance()).unwrap() - 13.31833512).abs() < 1e-5);
}

#[test]
fn intersect_outputs_exact_oblique_cone_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
    let tilted = NurbsSurface::try_bilinear([
        Point3::try_new(-3.0, -3.0, 0.5).unwrap(),
        Point3::try_new(3.0, -3.0, 2.5).unwrap(),
        Point3::try_new(3.0, 3.0, 2.5).unwrap(),
        Point3::try_new(-3.0, 3.0, 0.5).unwrap(),
    ])
    .unwrap();
    let ids = [
        document.add_geometry(Geometry::NurbsSurface(cone)).unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(tilted))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(section) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("cone section should be an exact conic")
    };
    assert_eq!(section.degree(), 2);
    assert!(section.is_closed().unwrap());
    assert!((section.length(document.tolerance()).unwrap() - 6.708268235).abs() < 1e-5);
}

#[test]
fn intersect_outputs_finite_pole_crossing_cone_section() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let frame = Frame3::try_from_normal(
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let cone = NurbsSurface::try_cone(frame, 2.0, 3.0).unwrap();
    let tilted = NurbsSurface::try_bilinear([
        Point3::try_new(-4.0, -4.0, -6.5).unwrap(),
        Point3::try_new(4.0, -4.0, 9.5).unwrap(),
        Point3::try_new(4.0, 4.0, 9.5).unwrap(),
        Point3::try_new(-4.0, 4.0, -6.5).unwrap(),
    ])
    .unwrap();
    let ids = [
        document.add_geometry(Geometry::NurbsSurface(cone)).unwrap(),
        document
            .add_geometry(Geometry::NurbsSurface(tilted))
            .unwrap(),
    ];
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Intersect").unwrap(),
        "Created 1 intersection object(s) from 1 object pair(s)"
    );
    let Geometry::NurbsCurve(section) = document.selected_objects().next().unwrap().geometry()
    else {
        panic!("finite cone section should be an exact conic")
    };
    assert_eq!(section.degree(), 2);
    assert!(!section.is_closed().unwrap());
    for parameter in [*section.domain().start(), *section.domain().end()] {
        assert!((section.evaluate(parameter).unwrap().z() - 3.0).abs() < 1e-8);
    }
}
