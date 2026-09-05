use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn frame() -> Frame3 {
    Frame3::try_from_directions(
        p(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

struct Lift;
impl PointMorph for Lift {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(
            p.x(),
            p.y(),
            p.z() + p.x().powi(2) + p.x() * p.y() * 0.25 + p.y().powi(3),
        )
    }
}

fn check(source: &Brep, fitted: &Brep, morph: &impl PointMorph, epsilon: Real) {
    assert_eq!(source.vertices.len(), fitted.vertices.len());
    assert_eq!(source.edges.len(), fitted.edges.len());
    assert_eq!(source.faces.len(), fitted.faces.len());
    assert_eq!(source.is_solid(), fitted.is_solid());
    for (source, fitted) in source.vertices.iter().zip(&fitted.vertices) {
        assert_eq!(fitted.point, morph.morph_point(source.point).unwrap());
    }
    for (source, fitted) in source.edges.iter().zip(&fitted.edges) {
        assert_eq!(source.vertices, fitted.vertices);
        assert_eq!(source.curve.domain(), fitted.curve.domain());
        for i in 0..=64 {
            let t = source.curve.parameter_at(fraction(i, 64)).unwrap();
            let expected = morph
                .morph_point(source.curve.evaluate(t).unwrap())
                .unwrap();
            assert!(
                fitted
                    .curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    <= epsilon
            );
        }
    }
    for (source, fitted) in source.faces.iter().zip(&fitted.faces) {
        assert_eq!(source.reversed, fitted.reversed);
        assert_eq!(source.loops, fitted.loops);
        assert_eq!(source.surface.domain_u(), fitted.surface.domain_u());
        assert_eq!(source.surface.domain_v(), fitted.surface.domain_v());
        for j in 0..=16 {
            for i in 0..=16 {
                let u = source.surface.parameter_at_u(fraction(i, 16)).unwrap();
                let v = source.surface.parameter_at_v(fraction(j, 16)).unwrap();
                let expected = morph
                    .morph_point(source.surface.evaluate(u, v).unwrap())
                    .unwrap();
                assert!(
                    fitted
                        .surface
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        <= epsilon
                );
            }
        }
    }
    fitted
        .validate(Tolerance::try_new(epsilon, 1e-12, 1e-10).unwrap())
        .unwrap();
}

fn fraction(i: usize, count: usize) -> Real {
    if i == 0 {
        0.0
    } else if i == count {
        1.0
    } else {
        (i as Real - 0.3819660112501051) / count as Real
    }
}

#[test]
fn nonlinear_box_morph_retains_shared_edges_and_exact_cubic_face_images() {
    let source = Brep::try_box(frame(), [[0.0, 1.0]; 3], Tolerance::DEFAULT).unwrap();
    let fitted = Lift.morph_brep(&source, Tolerance::DEFAULT).unwrap();
    check(&source, &fitted, &Lift, 1e-9);
    assert!((fitted.signed_volume(Tolerance::DEFAULT).unwrap() - 1.0).abs() < 1e-9);
    let topology = fitted.tessellate(2, Tolerance::DEFAULT).unwrap().topology();
    assert!(topology.is_closed());
    assert_eq!(topology.orientation_conflict_edge_count(), 0);
}

#[test]
fn each_shared_edge_is_fitted_once_and_loose_source_tolerances_cannot_hide_gaps() {
    use std::cell::Cell;
    struct Counted {
        curves: Cell<usize>,
        surfaces: Cell<usize>,
    }
    impl PointMorph for Counted {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Ok(p)
        }
        fn morph_nurbs_curve(
            &self,
            curve: &NurbsCurve,
            _: Tolerance,
        ) -> Result<NurbsCurve, GeometryError> {
            self.curves.set(self.curves.get() + 1);
            Ok(curve.clone())
        }
        fn morph_nurbs_surface(
            &self,
            surface: &NurbsSurface,
            _: Tolerance,
        ) -> Result<NurbsSurface, GeometryError> {
            self.surfaces.set(self.surfaces.get() + 1);
            Ok(surface.clone())
        }
    }
    let mut source = Brep::try_box(frame(), [[0.0, 1.0]; 3], Tolerance::DEFAULT).unwrap();
    let counted = Counted {
        curves: Cell::new(0),
        surfaces: Cell::new(0),
    };
    let fitted = counted.morph_brep(&source, Tolerance::DEFAULT).unwrap();
    assert_eq!(counted.curves.get(), source.edges.len());
    assert_eq!(counted.surfaces.get(), source.faces.len());
    assert_eq!(fitted.faces, source.faces);
    source.edges[0].curve = NurbsCurve::try_clamped_uniform(
        2,
        vec![p(0.0, 0.0, 0.0), p(0.5, 0.0, 1e-4), p(1.0, 0.0, 0.0)],
    )
    .unwrap();
    source.edges[0].tolerance = 1e-4;
    source.validate(Tolerance::DEFAULT).unwrap();
    assert!(counted.morph_brep(&source, Tolerance::DEFAULT).is_err());
}

#[test]
fn capped_rational_cylinder_morph_retains_seams_and_trimmed_caps() {
    let source = Brep::try_cylinder(frame(), 0.4, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let fitted = Lift.morph_brep(&source, tolerance).unwrap();
    check(&source, &fitted, &Lift, 1e-6);
    let topology = fitted.tessellate(2, tolerance).unwrap().topology();
    assert!(topology.is_closed());
    assert_eq!(topology.orientation_conflict_edge_count(), 0);
    assert!(
        fitted
            .polygon_mesh(0.0, false, false, tolerance)
            .unwrap()
            .topology()
            .is_solid()
    );
}

#[test]
fn nonlinearly_morphed_planar_hole_keeps_exact_uv_loops() {
    let ring = |radius: Real| {
        crate::Circle3::try_new(
            p(0.0, 0.0, 0.0),
            radius,
            frame().z_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let source =
        Brep::try_planar_face_with_holes(&ring(0.5), &[ring(0.2)], Tolerance::DEFAULT).unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let fitted = Lift.morph_brep(&source, tolerance).unwrap();
    check(&source, &fitted, &Lift, 1e-6);
}
