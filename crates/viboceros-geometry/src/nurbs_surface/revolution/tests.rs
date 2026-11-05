use super::*;
use crate::{Brep, BrepTrimType, CurveContinuity};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn partial_exact_revolution_retains_axis_poles_as_singular_boundaries() {
    let tolerance = Tolerance::DEFAULT;
    let origin = point(12., -7., 2.);
    let axis = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let profile = NurbsCurve::try_new(
        1,
        vec![origin, point(15., -7., 3.), point(12., -7., 6.)],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    for sweep in [120_f64.to_radians(), -270_f64.to_radians()] {
        let surface =
            NurbsSurface::try_revolved_curve(&profile, origin, axis, 30_f64.to_radians(), sweep)
                .unwrap();
        let brep = Brep::try_surface_partitioned_at_discontinuities(
            &surface,
            &[(CurveContinuity::G1, 0.1_f64.to_radians())],
            tolerance,
        )
        .unwrap();
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (2, 5, 4)
        );
        assert_eq!(
            brep.faces()
                .iter()
                .flat_map(|f| f.loops())
                .flat_map(|l| l.trims())
                .filter(|t| t.trim_type() == BrepTrimType::Singular)
                .count(),
            2
        );
    }
}

#[test]
fn deformable_revolution_preserves_rational_profiles_and_periodic_seam_jets() {
    let profile = NurbsCurve::try_new_rational(
        3,
        [
            (point(3., 0., 0.), 1.),
            (point(4., 0., 2.), 0.5),
            (point(2., 0., 5.), 2.),
            (point(3., 0., 8.), 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![-2., -2., -2., -2., 7., 7., 7., 7.],
    )
    .unwrap();
    let origin = point(0., 0., 0.);
    let axis = UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap();
    let original = profile.clone();
    for count in [4, 10, 32] {
        for sweep in [std::f64::consts::TAU, -std::f64::consts::TAU, 2.1, -2.1] {
            let s = NurbsSurface::try_deformable_revolved_curve(
                &profile, origin, axis, 0.3, sweep, count,
            )
            .unwrap();
            assert_eq!(s.degree_u(), 3);
            assert_eq!(s.degree_v(), 3);
            assert_eq!(s.knots_v(), profile.knots());
            let full = sweep.abs() == std::f64::consts::TAU;
            assert_eq!(s.control_point_count_u(), count + if full { 3 } else { 0 });
            assert_eq!(s.is_periodic_u(), full);
            for (i, row) in s
                .control_points()
                .chunks(s.control_point_count_u())
                .enumerate()
            {
                assert!(
                    row.iter()
                        .all(|p| p.weight() == profile.control_points()[i].weight())
                );
            }
            let rotation = AffineTransform3::try_rotation(
                origin,
                axis,
                if sweep < 0. { 0.3 + sweep } else { 0.3 },
            )
            .unwrap();
            for i in 0..=16 {
                let v = profile.parameter_at(i as Real / 16.).unwrap();
                let expected = rotation
                    .transform_point(profile.evaluate(v).unwrap())
                    .unwrap();
                assert!(
                    expected
                        .distance_to(s.evaluate(*s.domain_u().start(), v).unwrap())
                        .unwrap()
                        < 1e-11
                );
                if full {
                    let first = s
                        .evaluate_with_second_derivatives(*s.domain_u().start(), v)
                        .unwrap();
                    let last = s
                        .evaluate_with_second_derivatives(*s.domain_u().end(), v)
                        .unwrap();
                    assert!(first.point.distance_to(last.point).unwrap() < 1e-11);
                    for (a, b) in [
                        (first.derivative_u, last.derivative_u),
                        (first.derivative_uu, last.derivative_uu),
                    ] {
                        assert!(
                            a.to_array()
                                .into_iter()
                                .zip(b.to_array())
                                .all(|(x, y)| (x - y).abs() < 1e-11)
                        );
                    }
                }
            }
        }
    }
    assert_eq!(profile, original);
}
