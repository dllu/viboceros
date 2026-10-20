use super::*;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(4e-5, 1e-12, 1e-10).unwrap()
}

fn check(source: &Brep, morph: &StretchPointMorph) -> Brep {
    let before = source.clone();
    let after = source
        .try_stretch_affine_faces(morph, tolerance())
        .unwrap()
        .expect("affine face specialization");
    assert_eq!(after.faces.len(), source.faces.len());
    assert_eq!(after.edges.len(), source.edges.len());
    assert_eq!(after.vertices.len(), source.vertices.len());
    assert_eq!(after.is_solid(), source.is_solid());
    for (a, b) in after.edges.iter().zip(&source.edges) {
        assert_eq!(a.vertices, b.vertices);
    }
    for (old, new) in source.faces.iter().zip(&after.faces) {
        let chart = Chart::new(&old.surface, morph);
        assert_eq!(old.reversed, new.reversed);
        assert!(
            new.surface.control_points().len() <= 24,
            "planar face grew a dense tensor"
        );
        for j in 0..=12 {
            for i in 0..=12 {
                let fraction = |i| {
                    if i == 0 {
                        0.
                    } else if i == 12 {
                        1.
                    } else {
                        (i as Real - 0.37) / 12.
                    }
                };
                let u = old.surface.parameter_at_u(fraction(i)).unwrap();
                let v = old.surface.parameter_at_v(fraction(j)).unwrap();
                let uv = chart.as_ref().map_or_else(
                    || Point2::try_new(u, v).unwrap(),
                    |chart| chart.uv(Point2::try_new(u, v).unwrap()).unwrap(),
                );
                let expected = morph
                    .morph_point(old.surface.evaluate(u, v).unwrap())
                    .unwrap();
                let actual = new.surface.evaluate(uv.x(), uv.y()).unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() < 2e-11,
                    "{actual:?} != {expected:?}"
                );
                assert_eq!(
                    old.contains_parameters(u, v, tolerance()).unwrap(),
                    new.contains_parameters(uv.x(), uv.y(), tolerance())
                        .unwrap()
                );
                let (_, du, dv) = old.surface.evaluate_with_derivatives(u, v).unwrap();
                let n = morph.axis().1.as_vector();
                let t = n.dot_point_difference(old.surface.evaluate(u, v).unwrap(), morph.axis().0)
                    / morph.length();
                let factor = if t > 0. && t < 1. {
                    (morph.new_length() - morph.length()) / morph.length() * 6. * t * (1. - t)
                } else {
                    0.
                };
                let mapped = |d: Vector3| {
                    let along = n.scaled(factor * n.dot(d).unwrap()).unwrap();
                    Vector3::try_from(std::array::from_fn(|i| {
                        d.to_array()[i] + along.to_array()[i]
                    }))
                    .unwrap()
                };
                let expected = mapped(du)
                    .cross(mapped(dv))
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap();
                let (_, du, dv) = new
                    .surface
                    .evaluate_with_derivatives(uv.x(), uv.y())
                    .unwrap();
                let actual = du.cross(dv).unwrap().normalized_nonzero().unwrap();
                assert!(actual.as_vector().dot(expected.as_vector()).unwrap() > 1. - 1e-12);
            }
        }
        for (old, new) in old.loops.iter().zip(&new.loops) {
            assert_eq!(old.loop_type, new.loop_type);
            assert_eq!(old.trims.len(), new.trims.len());
            for (a, b) in old.trims.iter().zip(&new.trims) {
                assert_eq!(a.vertices, b.vertices);
                assert_eq!(a.edge, b.edge);
                assert_eq!(a.reversed_3d, b.reversed_3d);
                assert_eq!(a.curve.degree(), b.curve.degree());
                assert_eq!(a.curve.knots(), b.curve.knots());
                assert_eq!(
                    a.curve.control_points().len(),
                    b.curve.control_points().len()
                );
                for (a, b) in a
                    .curve
                    .control_points()
                    .iter()
                    .zip(b.curve.control_points())
                {
                    assert_eq!(a.weight(), b.weight());
                }
            }
        }
    }
    assert_eq!(source, &before);
    after
}

#[test]
fn ruled_cubic_charts_preserve_spatial_images_normals_trims_and_shared_topology() {
    let source = Brep::try_box(frame(), [[1., 3.], [-1., 1.], [0., 10.]], tolerance()).unwrap();
    for (a, b) in [
        (p(0., 0., 10.), p(0., 0., 0.)),
        (p(1., 2., 3.), p(5., 6., 11.)),
        (p(1., 2., 3.), p(4., -2., 11.)),
        (p(0., 0., 0.), p(10., 0., 0.)),
    ] {
        let morph = StretchPointMorph::try_new(a, b, a.distance_to(b).unwrap() * 0.5).unwrap();
        let result = check(&source, &morph);
        // Integrate the analytic determinant of the point map over the original
        // box independently, using finite differences of truncated powers.
        let n = morph.axis().1.as_vector();
        let coefficients = n
            .to_array()
            .into_iter()
            .zip([2., 2., 10.])
            .map(|(a, w)| a * w / morph.length())
            .filter(|a| *a != 0.)
            .collect::<Vec<_>>();
        let offset = n.dot_point_difference(p(1., -1., 0.), a) / morph.length();
        let order = coefficients.len();
        let factorial = |n: usize| (1..=n).product::<usize>() as Real;
        let primitive = |t: Real| {
            6. * (t.max(0.).powi((order + 1) as i32) + (t - 1.).max(0.).powi((order + 1) as i32))
                / factorial(order + 1)
                + 12.
                    * (-t.max(0.).powi((order + 2) as i32)
                        + (t - 1.).max(0.).powi((order + 2) as i32))
                    / factorial(order + 2)
        };
        let mut integral = 0.;
        for corner in 0_u32..1 << order {
            let t = offset
                + coefficients
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| corner & (1 << i) != 0)
                    .map(|(_, c)| c)
                    .sum::<Real>();
            integral += if (order - corner.count_ones() as usize).is_multiple_of(2) {
                primitive(t)
            } else {
                -primitive(t)
            };
        }
        integral /= coefficients.iter().product::<Real>();
        let expected = 40. * (1. + (morph.new_length() / morph.length() - 1.) * integral);
        let actual = result
            .signed_volume(Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap())
            .unwrap();
        assert!(
            (actual - expected).abs() < 2e-7,
            "volume {actual} != analytic {expected}"
        );
        let expected = BoundingBox3::from_points(
            source
                .vertices
                .iter()
                .map(|v| morph.morph_point(v.point).unwrap()),
        )
        .unwrap();
        let actual = result
            .tight_bounds(Tolerance::try_new(1e-8, 1e-12, 1e-10).unwrap())
            .unwrap();
        assert!(actual.min().distance_to(expected.min()).unwrap() < 2e-7);
        assert!(actual.max().distance_to(expected.max()).unwrap() < 2e-7);
    }
}

#[test]
fn preserve_structure_keeps_the_original_surface_basis_instead_of_an_axial_chart() {
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p(0., 0., 0.), p(4., 0., 0.), p(0., 4., 0.), p(4., 4., 0.)],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = Brep::try_surface_face(surface.clone(), tolerance()).unwrap();
    let morph = StretchPointMorph::try_new(p(-1., -1., 0.), p(4., 3., 0.), 3.)
        .unwrap()
        .with_preserve_structure(true);
    let result = morph.morph_brep(&source, tolerance()).unwrap();
    let result = result.faces[0].surface();
    assert_eq!(result.degree_u(), surface.degree_u());
    assert_eq!(result.degree_v(), surface.degree_v());
    assert_eq!(result.knots_u(), surface.knots_u());
    assert_eq!(result.knots_v(), surface.knots_v());
    for (a, b) in surface.control_points().iter().zip(result.control_points()) {
        assert_eq!(morph.morph_point(a.point()).unwrap(), b.point());
    }
}

#[test]
fn warped_source_faces_are_declined_without_tolerance_flattening() {
    let s = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(0., 3., 0.),
            p(2., 3., f64::EPSILON),
        ]
        .map(|p| WeightedPoint3::try_new(p, 1.).unwrap())
        .to_vec(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let b = Brep::try_surface_face(s, tolerance()).unwrap();
    let morph = StretchPointMorph::try_new(p(0., 0., 0.), p(2., 1., 1.), 4.).unwrap();
    assert!(
        b.try_stretch_affine_faces(&morph, tolerance())
            .unwrap()
            .is_none()
    );
    assert!(Chart::new(b.faces[0].surface(), &morph).is_none());
}

#[test]
fn nonrectangular_outer_trim_and_curved_hole_keep_their_weighted_boundaries() {
    let outer = crate::Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(8., 0., 0.),
            p(7., 6., 0.),
            p(0., 6., 0.),
            p(0., 0., 0.),
        ],
        tolerance(),
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let circle = crate::Circle3::try_new(
        p(3., 3., 0.),
        1.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        tolerance(),
    )
    .unwrap();
    let hole = crate::Curve3::Circle(circle).as_ref().to_nurbs().unwrap();
    let source = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance()).unwrap();
    let a = p(-1., -1., 0.);
    let b = p(8., 2., 0.);
    let morph = StretchPointMorph::try_new(a, b, a.distance_to(b).unwrap() * 0.75).unwrap();
    check(&source, &morph);
}
