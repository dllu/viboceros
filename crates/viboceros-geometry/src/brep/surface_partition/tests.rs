use super::*;

fn point(x: Real, y: Real, z: Real) -> WeightedPoint3 {
    WeightedPoint3::try_new(Point3::try_new(x, y, z).unwrap(), 1.).unwrap()
}

fn hidden_crease(amplitude: Real) -> NurbsSurface {
    let controls = [0., -amplitude / 6., amplitude / 6., 0.]
        .into_iter()
        .enumerate()
        .flat_map(|(v, z)| {
            (0..3).map(move |u| point(u as Real, v as Real / 3., if u == 2 { z } else { 0. }))
        })
        .collect();
    NurbsSurface::try_new_rational(
        1,
        3,
        3,
        4,
        controls,
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}

#[test]
fn a_crease_between_native_samples_is_split_without_changing_the_tensor() {
    let surface = hidden_crease(0.04);
    let before = surface.clone();
    let angle = 0.1_f64.to_radians();
    // Its angle is atan(A*t*(1-t)*(t-1/2)): zero at 0, 1/2 and 1,
    // but about 0.107 degrees at 1/4. The basic public query misses it.
    for continuity in [
        CurveContinuity::G1,
        CurveContinuity::G2,
        CurveContinuity::Gsmooth,
    ] {
        assert_eq!(
            surface.discontinuity_parameters(continuity, angle).unwrap(),
            [Vec::<Real>::new(), Vec::new()]
        );
        let result = Brep::try_surface_partitioned_at_discontinuities(
            &surface,
            &[(continuity, angle)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            (
                result.faces().len(),
                result.edges().len(),
                result.vertices().len()
            ),
            (2, 7, 6)
        );
        check_locus(&surface, &result);
        assert_eq!(surface, before);
    }
}

#[test]
fn a_bend_below_the_angle_limit_retains_its_single_face() {
    for amplitude in [0., 0.02] {
        let surface = hidden_crease(amplitude);
        let result = Brep::try_surface_partitioned_at_discontinuities(
            &surface,
            &[(CurveContinuity::G1, 0.1_f64.to_radians())],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.faces().len(), 1);
        assert_eq!(result.faces()[0].surface(), &surface);
    }
}

#[test]
fn adaptive_crease_work_uses_the_shared_partition_budget() {
    let surface = hidden_crease(0.04);
    let mut cuts = [vec![], vec![]];
    assert!(g1::augment(&surface, 0.1_f64.to_radians(), &mut cuts, &mut 0).is_err());
}

#[test]
fn zero_and_obtuse_angle_limits_retain_smooth_enough_cells() {
    let (sine, cosine) = 160_f64.to_radians().sin_cos();
    for (end, angle) in [
        ([3., 0.], 0.),
        ([1. + cosine, sine], 170_f64.to_radians()),
        ([1. + cosine, sine], std::f64::consts::PI),
    ] {
        let controls = [0., 1.]
            .into_iter()
            .flat_map(|y| [[0., 0.], [1., 0.], end].map(move |[x, z]| point(x, y, z)))
            .collect();
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            3,
            2,
            controls,
            vec![0., 0., 1., 2., 2.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let result = Brep::try_surface_partitioned_at_discontinuities(
            &surface,
            &[(CurveContinuity::G1, angle)],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(result.faces().len(), 1);
        assert_eq!(result.faces()[0].surface(), &surface);
    }
}

fn check_locus(source: &NurbsSurface, result: &Brep) {
    for face in result.faces() {
        let surface = face.surface();
        for i in 0..=4 {
            for j in 0..=4 {
                let u = surface.parameter_at_u(i as Real / 4.).unwrap();
                let v = surface.parameter_at_v(j as Real / 4.).unwrap();
                assert!(
                    source
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(surface.evaluate(u, v).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
}

#[test]
fn a_partial_crease_adds_a_shared_t_junction_without_cutting_the_smooth_neighbor() {
    let controls = (0..3)
        .flat_map(|v| {
            (0..3).map(move |u| point(u as Real, v as Real, if u == 2 && v > 0 { 1. } else { 0. }))
        })
        .collect();
    let surface = NurbsSurface::try_new_rational(
        1,
        1,
        3,
        3,
        controls,
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let original = surface.clone();
    let result = Brep::try_surface_partitioned_at_discontinuities(
        &surface,
        &[(CurveContinuity::G1, 0.1_f64.to_radians())],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(surface, original);
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (3, 10, 8)
    );
    assert_eq!(
        result
            .edge_use_counts()
            .into_iter()
            .filter(|n| *n == 2)
            .count(),
        3
    );
    assert!(
        result
            .faces()
            .iter()
            .any(|f| f.loops()[0].trims().len() == 5)
    );
    check_locus(&surface, &result);
}

#[test]
fn curvature_partition_refines_a_c1_basis_without_requiring_bitwise_edge_restrictions() {
    let controls = [0., 1.]
        .into_iter()
        .flat_map(|y| [[0., 0.], [1., 0.], [1., 1.], [1., 2.]].map(move |[x, z]| point(x, y, z)))
        .collect();
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        4,
        2,
        controls,
        vec![0., 0., 0., 1., 2., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let result = Brep::try_surface_partitioned_at_discontinuities(
        &surface,
        &[(CurveContinuity::G2, 1_f64.to_radians())],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (2, 7, 6)
    );
    let smooth = Brep::try_surface_partitioned_at_discontinuities(
        &surface,
        &[(CurveContinuity::G1, 0.1_f64.to_radians())],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(smooth.faces().len(), 1);
    check_locus(&surface, &result);
}

#[test]
fn invalid_angles_and_positional_jumps_are_rejected() {
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap(),
        Point3::try_new(1., 1., 0.).unwrap(),
        Point3::try_new(0., 1., 0.).unwrap(),
    ])
    .unwrap();
    for angle in [Real::NAN, -1., 4.] {
        assert!(
            Brep::try_surface_partitioned_at_discontinuities(
                &surface,
                &[(CurveContinuity::G1, angle)],
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
    assert_eq!(
        Brep::try_surface_partitioned_at_discontinuities(&surface, &[], Tolerance::DEFAULT)
            .unwrap()
            .faces()
            .len(),
        1
    );
    let jumped = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| [0., 1., 2., 3.].map(move |x| point(x, y, 0.)))
            .collect(),
        vec![0., 0., 1., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        Brep::try_surface_partitioned_at_discontinuities(&jumped, &[], Tolerance::DEFAULT).is_err()
    );
}

#[test]
fn a_closed_sphere_sews_its_seam_and_keeps_both_singular_vertices() {
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = NurbsSurface::try_sphere(frame, 1.).unwrap();
    let result =
        Brep::try_surface_partitioned_at_discontinuities(&surface, &[], Tolerance::DEFAULT)
            .unwrap();
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (1, 1, 2)
    );
    assert!(result.is_solid());
    assert_eq!(result.faces()[0].surface(), &surface);
}

#[test]
fn a_large_c2_tensor_does_not_spend_a_quadratic_budget_on_impossible_g1_breaks() {
    let count = 1000;
    let mut knots = vec![0.; 4];
    knots.extend((1..count - 3).map(|i| i as Real));
    knots.extend([count as Real - 3.; 4]);
    let controls = [0., 1.]
        .into_iter()
        .flat_map(|y| (0..count).map(move |i| point(i as Real, y, 0.)))
        .collect();
    let source =
        NurbsSurface::try_new_rational(3, 1, count, 2, controls, knots, vec![0., 0., 1., 1.])
            .unwrap();
    let result = Brep::try_surface_partitioned_at_discontinuities(
        &source,
        &[(CurveContinuity::G1, 0.1_f64.to_radians())],
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (1, 4, 4)
    );
    assert_eq!(result.faces()[0].surface(), &source);
}

#[test]
fn signed_zero_endpoints_share_one_internal_edge() {
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(-1., 0., 0.).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap(),
        Point3::try_new(1., 1., 0.).unwrap(),
        Point3::try_new(-1., 1., 0.).unwrap(),
    ])
    .unwrap()
    .try_reparameterized(-1. ..=1., 0. ..=1.)
    .unwrap();
    let left = surface.try_trimmed(-1. ..=-0., 0. ..=1.).unwrap();
    let right = surface.try_trimmed(0. ..=1., 0. ..=1.).unwrap();
    assert_ne!(
        left.domain_u().end().to_bits(),
        right.domain_u().start().to_bits()
    );
    let mut budget = MAX_WORK;
    let result =
        topology::build(&surface, vec![left, right], Tolerance::DEFAULT, &mut budget).unwrap();
    assert_eq!(
        (
            result.faces().len(),
            result.edges().len(),
            result.vertices().len()
        ),
        (2, 7, 6)
    );
    assert_eq!(
        result
            .edge_use_counts()
            .into_iter()
            .filter(|n| *n == 2)
            .count(),
        1
    );
    check_locus(&surface, &result);
}
