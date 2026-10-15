#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Point3, Tolerance, Vector3};
use viboceros_smlib::{Shape, Solid};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap()
}
fn box_shape() -> Shape {
    let frame = viboceros_geometry::Frame3::try_from_directions(
        p(0., 0., 0.),
        v(1., 0., 0.),
        v(0., 1., 0.),
        tolerance(),
    )
    .unwrap();
    Shape::from_brep(
        &Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 4.]], tolerance()).unwrap(),
        tolerance(),
    )
    .unwrap()
}

#[test]
fn parallel_box_silhouettes_preserve_source_and_direction_scale_and_spatial_depth() {
    let shape = box_shape();
    let before = shape.to_brep(tolerance()).unwrap();
    for (direction, z) in [
        (v(0., 0., 1.), 4.),
        (v(0., 0., 100.), 4.),
        (v(0., 0., -1.), 0.),
    ] {
        let curves = shape
            .parallel_silhouette_curves(direction, 1e-5, 0.05, false)
            .unwrap();
        assert_eq!(curves.len(), 4);
        let mut length = 0.;
        for curve in &curves {
            let start = curve.evaluate(*curve.domain().start()).unwrap();
            let end = curve.evaluate(*curve.domain().end()).unwrap();
            length += start.distance_to(end).unwrap();
            for i in 0..=8 {
                let q = curve
                    .evaluate(curve.parameter_at(i as f64 / 8.).unwrap())
                    .unwrap();
                assert!((q.z() - z).abs() < 1e-8, "view {direction:?}: {q:?}");
                assert!(
                    q.x() >= -1e-8 && q.x() <= 2. + 1e-8 && q.y() >= -1e-8 && q.y() <= 3. + 1e-8
                );
                assert!(
                    q.x().abs() < 1e-8
                        || (q.x() - 2.).abs() < 1e-8
                        || q.y().abs() < 1e-8
                        || (q.y() - 3.).abs() < 1e-8
                );
            }
        }
        assert!((length - 10.).abs() < 1e-8);
    }
    assert_eq!(shape.to_brep(tolerance()).unwrap(), before);
}

#[test]
fn parallel_sphere_silhouette_is_spatial_equator_for_oblique_views() {
    let center = p(1., 2., 3.);
    let brep = Solid::sphere(center, 2.)
        .unwrap()
        .to_brep(tolerance())
        .unwrap();
    let shape = Shape::from_brep(&brep, tolerance()).unwrap();
    for direction in [v(0., 0., 1.), v(1., 2., 3.)] {
        let normal = direction.normalized_nonzero().unwrap().as_vector();
        let curves = shape
            .parallel_silhouette_curves(direction, 1e-5, 0.05, false)
            .unwrap();
        assert!(!curves.is_empty());
        let length = curves
            .iter()
            .map(|c| {
                viboceros_geometry::CurveRef::NurbsCurve(c)
                    .length(tolerance())
                    .unwrap()
            })
            .sum::<f64>();
        assert!(
            (length - 4. * std::f64::consts::PI).abs() < 1e-4,
            "sphere silhouette length {length}"
        );
        for curve in curves {
            for i in 0..=64 {
                let q = curve
                    .evaluate(curve.parameter_at(i as f64 / 64.).unwrap())
                    .unwrap();
                assert!((q.distance_to(center).unwrap() - 2.).abs() < 5e-5);
                assert!(center.vector_to(q).unwrap().dot(normal).unwrap().abs() < 5e-5);
            }
        }
    }
}

#[test]
fn parallel_torus_silhouette_has_both_hole_and_outer_circles() {
    let frame = viboceros_geometry::Frame3::try_from_directions(
        p(0., 0., 0.),
        v(1., 0., 0.),
        v(0., 1., 0.),
        tolerance(),
    )
    .unwrap();
    let surface = viboceros_geometry::NurbsSurface::try_torus(frame, 3., 1.).unwrap();
    let brep = Brep::try_surface_grid(&surface, &[], &[], tolerance()).unwrap();
    let shape = Shape::from_brep(&brep, tolerance()).unwrap();
    let curves = shape
        .parallel_silhouette_curves(v(0., 0., 1.), 1e-5, 0.05, false)
        .unwrap();
    assert!(!curves.is_empty());
    let (mut inner, mut outer) = (false, false);
    let mut length = 0.;
    for curve in curves {
        length += viboceros_geometry::CurveRef::NurbsCurve(&curve)
            .length(tolerance())
            .unwrap();
        for i in 0..=32 {
            let q = curve
                .evaluate(curve.parameter_at(i as f64 / 32.).unwrap())
                .unwrap();
            assert!(q.z().abs() < 5e-5);
            let radius = q.x().hypot(q.y());
            assert!((radius - 2.).abs() < 5e-5 || (radius - 4.).abs() < 5e-5);
            inner |= (radius - 2.).abs() < 5e-5;
            outer |= (radius - 4.).abs() < 5e-5;
        }
    }
    assert!(inner && outer);
    assert!((length - 12. * std::f64::consts::PI).abs() < 1e-3);
}

#[test]
fn parallel_silhouette_respects_rational_outer_and_hole_trims_from_both_sides() {
    let normal = viboceros_geometry::UnitVector3::try_new(0., 0., 1., tolerance()).unwrap();
    let circle = |radius| {
        viboceros_geometry::Circle3::try_new(p(0., 0., 5.), radius, normal, tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let brep = Brep::try_planar_face_with_holes(&circle(5.), &[circle(2.)], tolerance()).unwrap();
    let shape = Shape::from_brep(&brep, tolerance()).unwrap();
    for direction in [v(0., 0., 1.), v(0., 0., -1.)] {
        let curves = shape
            .parallel_silhouette_curves(direction, 1e-5, 0.05, false)
            .unwrap();
        let mut length = 0.;
        let (mut outer, mut hole) = (false, false);
        for curve in curves {
            length += viboceros_geometry::CurveRef::NurbsCurve(&curve)
                .length(tolerance())
                .unwrap();
            for i in 0..=32 {
                let q = curve
                    .evaluate(curve.parameter_at(f64::from(i) / 32.).unwrap())
                    .unwrap();
                let radius = q.x().hypot(q.y());
                assert!((q.z() - 5.).abs() < 1e-7);
                assert!((radius - 5.).abs() < 1e-7 || (radius - 2.).abs() < 1e-7);
                outer |= (radius - 5.).abs() < 1e-7;
                hole |= (radius - 2.).abs() < 1e-7;
            }
        }
        assert!(outer && hole);
        assert!((length - 14. * std::f64::consts::PI).abs() < 1e-4);
    }
}

#[test]
fn invalid_silhouette_input_declines_and_leaves_shape_reusable() {
    let shape = box_shape();
    for (direction, distance, angle) in [
        (v(0., 0., 0.), 1e-5, 0.05),
        (v(0., 0., 1.), 0., 0.05),
        (v(0., 0., 1.), f64::from_bits(1), 0.05),
        (v(0., 0., 1.), f64::NAN, 0.05),
        (v(0., 0., 1.), 1e-5, 0.),
        (v(0., 0., 1.), 1e-5, 4.),
    ] {
        assert!(
            shape
                .parallel_silhouette_curves(direction, distance, angle, false)
                .is_err()
        );
    }
    assert_eq!(
        shape
            .parallel_silhouette_curves(v(0., 0., 1.), 1e-5, 0.05, false)
            .unwrap()
            .len(),
        4
    );
}
