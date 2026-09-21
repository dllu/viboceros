use super::*;

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn shared_edge_splitting_updates_both_faces_without_changing_their_surfaces() {
    let source =
        Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let edge = &source.edges[0];
    let t = edge.curve.parameter_at(0.25).unwrap();
    let result = source
        .try_split_edges_at_parameters(&[(0, vec![t])], Tolerance::DEFAULT)
        .unwrap();
    assert!(result.is_solid());
    assert_eq!(result.vertices.len(), 9);
    assert_eq!(result.edges.len(), 13);
    assert_eq!(&result.vertices[..8], &source.vertices);
    assert_eq!(&result.edges[1..12], &source.edges[1..]);
    assert_eq!(result.vertices[8].point, edge.curve.evaluate(t).unwrap());
    for (a, b) in source.faces.iter().zip(&result.faces) {
        assert_eq!(a.surface, b.surface);
        assert_eq!(a.reversed, b.reversed);
    }
    assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-10);
}

#[test]
fn seam_splitting_updates_both_uses_and_keeps_incidence() {
    let source = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    let seam = source
        .faces
        .iter()
        .flat_map(|f| &f.loops)
        .flat_map(|l| &l.trims)
        .find(|t| t.trim_type == BrepTrimType::Seam)
        .unwrap()
        .edge
        .unwrap();
    let edge = &source.edges[seam];
    let result = source
        .try_split_edges_at_parameters(
            &[(
                seam,
                vec![
                    edge.curve.parameter_at(0.25).unwrap(),
                    edge.curve.parameter_at(0.75).unwrap(),
                ],
            )],
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(result.is_solid());
    assert_eq!(result.edges.len(), source.edges.len() + 2);
    assert_eq!(
        result
            .faces
            .iter()
            .flat_map(|f| &f.loops)
            .flat_map(|l| &l.trims)
            .filter(|t| t.trim_type == BrepTrimType::Seam)
            .count(),
        source
            .faces
            .iter()
            .flat_map(|f| &f.loops)
            .flat_map(|l| &l.trims)
            .filter(|t| t.trim_type == BrepTrimType::Seam)
            .count()
            + 4
    );
}

#[test]
fn invalid_split_requests_never_change_the_input() {
    let source =
        Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let before = source.clone();
    let t = source.edges[0].curve.parameter_at(0.5).unwrap();
    for splits in [
        vec![(usize::MAX, vec![t])],
        vec![(0, vec![])],
        vec![(0, vec![t, t])],
        vec![(0, vec![f64::NAN])],
        vec![(0, vec![t]), (0, vec![t])],
    ] {
        assert!(
            source
                .try_split_edges_at_parameters(&splits, Tolerance::DEFAULT)
                .is_err()
        );
        assert_eq!(source, before);
    }
}

#[test]
fn rational_trim_speed_and_parameter_domain_are_independent_of_spatial_edge_speed() {
    for degree in [1, 2] {
        let mut source =
            Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
        for trim in source
            .faces
            .iter_mut()
            .flat_map(|f| &mut f.loops)
            .flat_map(|l| &mut l.trims)
            .filter(|t| t.edge == Some(0))
        {
            let a = trim.curve.start_point().unwrap();
            let b = trim.curve.end_point().unwrap();
            let controls = (0..=degree)
                .map(|i| {
                    let t = i as Real / degree as Real;
                    WeightedPoint2::try_new(
                        Point2::try_new(a.x() * (1. - t) + b.x() * t, a.y() * (1. - t) + b.y() * t)
                            .unwrap(),
                        if i == degree { 9. } else { 1. },
                    )
                    .unwrap()
                })
                .collect();
            let knots = std::iter::repeat_n(100., degree + 1)
                .chain(std::iter::repeat_n(500., degree + 1))
                .collect();
            trim.curve = NurbsCurve2::try_new_rational(degree, controls, knots).unwrap();
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let parameters = [0.25, 0.75]
            .map(|t| source.edges[0].curve.parameter_at(t).unwrap())
            .to_vec();
        let result = source
            .try_split_edges_at_parameters(&[(0, parameters)], Tolerance::DEFAULT)
            .unwrap();
        assert!(result.is_solid());
        assert_eq!(result.edges.len(), 14);
        assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-8);
        for face in &result.faces {
            for trim in face
                .loops
                .iter()
                .flat_map(|l| &l.trims)
                .filter(|t| t.edge == Some(0))
            {
                let interval = trim.curve.domain();
                assert!(*interval.start() >= 100. && *interval.end() <= 500.);
                assert_eq!(trim.curve.degree(), degree);
            }
        }
    }
}

#[test]
fn ambiguous_backtracking_trim_correspondence_fails_atomically() {
    let mut source =
        Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    let trim = source
        .faces
        .iter_mut()
        .flat_map(|f| &mut f.loops)
        .flat_map(|l| &mut l.trims)
        .find(|t| t.edge == Some(0))
        .unwrap();
    let a = trim.curve.start_point().unwrap();
    let b = trim.curve.end_point().unwrap();
    trim.curve = NurbsCurve2::try_new(1, vec![a, b, a, b], vec![0., 0., 1., 2., 3., 3.]).unwrap();
    source.validate(Tolerance::DEFAULT).unwrap();
    let before = source.clone();
    let parameters = [0.25, 0.75]
        .map(|t| source.edges[0].curve.parameter_at(t).unwrap())
        .to_vec();
    assert!(
        source
            .try_split_edges_at_parameters(&[(0, parameters)], Tolerance::DEFAULT)
            .is_err()
    );
    assert_eq!(source, before);
}

#[test]
fn complete_knots_slice_exactly_with_work_proportional_to_retained_controls() {
    let points = (0..=4096)
        .map(|i| Point3::try_new(i as Real, (i % 2) as Real, 0.).unwrap())
        .collect();
    let curve = NurbsCurve::try_clamped_uniform(1, points).unwrap();
    let mut budget = Budget(8192);
    for (start, end) in curve.spans() {
        let piece = partition::subcurve(&curve, start..=end, &mut budget).unwrap();
        assert_eq!(piece.control_points().len(), 2);
        assert_eq!(
            piece.evaluate(start).unwrap(),
            curve.evaluate(start).unwrap()
        );
        assert_eq!(piece.evaluate(end).unwrap(), curve.evaluate(end).unwrap());
    }
    assert_eq!(budget.0, 0);
    assert!(partition::subcurve(&curve, 0.0..=1.0, &mut budget).is_err());
}

#[test]
fn split_counts_and_angle_options_are_bounded_before_output_construction() {
    let source =
        Brep::try_box(frame(), [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    assert!(
        source
            .try_split_edges_at_parameters(&[(0, vec![0.5; MAX_SPLITS + 1])], Tolerance::DEFAULT)
            .is_err()
    );
    for angle in [0., -1., f64::NAN, 4.] {
        assert!(
            source
                .try_split_kinky_edges(&[0], angle, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert!(
        source
            .try_split_kinky_edges(&[0, 0], 1., Tolerance::DEFAULT)
            .is_err()
    );
    assert!(
        source
            .try_split_kinky_edges(&[0], 1., Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
}

#[test]
fn rational_nonuniform_kink_domains_preserve_native_knots_and_weights() {
    for origin in [0., 1e9, -1e9] {
        let controls = [[0., 0.], [4., 0.], [0., 3.], [0., 0.]]
            .into_iter()
            .zip([1., 2., 3., 1.])
            .map(|(p, w)| {
                WeightedPoint3::try_new(Point3::try_new(p[0], p[1], 0.).unwrap(), w).unwrap()
            })
            .collect();
        let profile = NurbsCurve::try_new_rational(
            1,
            controls,
            [0., 0., 7., 11., 19., 19.].map(|t| t + origin).to_vec(),
        )
        .unwrap();
        let source = Brep::try_extruded_curve(
            &profile,
            Vector3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 2., 5.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let before = source.clone();
        let result = source
            .try_split_kinky_edges(&[1, 0], 1_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(source, before);
        assert!(result.is_solid());
        assert_eq!(result.vertices.len(), 6);
        assert_eq!(result.edges.len(), 7);
        for (edge, interval, weights) in [
            (0, [0., 7.], [1., 2.]),
            (3, [11., 19.], [3., 1.]),
            (4, [7., 11.], [2., 3.]),
        ] {
            let curve = &result.edges[edge].curve;
            assert_eq!(
                curve.domain(),
                (origin + interval[0])..=(origin + interval[1])
            );
            assert_eq!(
                curve
                    .control_points()
                    .iter()
                    .map(|p| p.weight())
                    .collect::<Vec<_>>(),
                weights
            );
        }
        assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 30.).abs() < 1e-8);
    }
}

#[test]
fn exact_partition_handles_full_order_jumps_nonclamped_ends_and_interior_cuts() {
    for (degree, knots) in [
        (1, vec![0., 0., 1., 1., 2., 2., 3., 3.]),
        (2, vec![-2., -1., 0., 1., 1., 2., 2., 3., 4., 5.]),
        (
            3,
            vec![0., 0., 0., 0., 1., 1., 1., 2., 2., 2., 3., 3., 3., 3.],
        ),
    ] {
        let controls = (0..knots.len() - degree - 1)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new(i as Real, (i % 3) as Real, 0.).unwrap(),
                    1. + (i % 2) as Real,
                )
                .unwrap()
            })
            .collect();
        let curve = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
        for (a, b) in curve.spans() {
            for (a, b) in [(a, b), (a + (b - a) * 0.125, b - (b - a) * 0.25)] {
                let piece = partition::subcurve(&curve, a..=b, &mut Budget(MAX_WORK)).unwrap();
                assert_eq!(piece.domain(), a..=b);
                assert_eq!(piece.degree(), degree);
                for i in 0..=16 {
                    let t = a + (b - a) * i as Real / 16.;
                    let side = if i == 16 {
                        crate::ParameterSide::Left
                    } else {
                        crate::ParameterSide::Right
                    };
                    assert!(
                        piece
                            .evaluate_on_side(t, side)
                            .unwrap()
                            .distance_to(curve.evaluate_on_side(t, side).unwrap())
                            .unwrap()
                            < 1e-12
                    );
                }
            }
        }
    }
}
