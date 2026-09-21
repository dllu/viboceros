use super::*;

fn point(x: Real) -> Point3 {
    Point3::try_new(x, 2. * x, 3. * x).unwrap()
}

fn proposal(source: &NurbsCurve, allowance: Real) -> Option<(NurbsCurve, Real)> {
    line(source, allowance, Tolerance::DEFAULT, &mut Budget(MAX_WORK)).unwrap()
}

#[test]
fn exact_lines_normalize_degree_weights_direction_and_parameter_speed() {
    for degree in [1, 2, 3, 17] {
        for gauge in [1e-300, -1e-300, 1., -1., 1e300, -1e300] {
            let controls = (0..=degree)
                .map(|i| {
                    WeightedPoint3::try_new(point(i as Real), gauge * (i + 1) as Real).unwrap()
                })
                .collect();
            let knots = [vec![5.; degree + 1], vec![11.; degree + 1]].concat();
            let source = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
            for source in [source.clone(), source.reversed().unwrap()] {
                let (result, bound) = proposal(&source, 0.).unwrap();
                assert_eq!(bound, 0.);
                assert_eq!(result.degree(), 1);
                assert_eq!(result.control_points().len(), 2);
                assert!(result.control_points().iter().all(|p| p.weight() == 1.));
                assert_eq!(
                    result.control_points()[0].point(),
                    source.control_points()[0].point()
                );
                assert_eq!(
                    result.control_points()[1].point(),
                    source.control_points().last().unwrap().point()
                );
                assert_eq!(
                    result.domain(),
                    0.0..=point(0.).distance_to(point(degree as Real)).unwrap()
                );
                assert_eq!(proposal(&result, 0.).unwrap().0, result);
            }
        }
    }
    let source = NurbsCurve::try_new(
        1,
        vec![point(0.), point(1.), point(4.)],
        vec![5., 5., 7., 11., 11.],
    )
    .unwrap();
    assert_eq!(proposal(&source, 0.).unwrap().0.degree(), 1);
    assert_eq!(proposal(&source, 0.).unwrap().0.control_points().len(), 2);
}

#[test]
fn reversing_degenerate_unclamped_and_mixed_sign_curves_are_not_assumed_linear() {
    let curves = [
        NurbsCurve::try_new(
            3,
            [0., 3., -2., 4.].map(point).to_vec(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap(),
        NurbsCurve::try_new(1, vec![point(0.), point(0.)], vec![0., 0., 1., 1.]).unwrap(),
        NurbsCurve::try_new(
            2,
            [0., 1., 4.].map(point).to_vec(),
            vec![-2., -1., 0., 1., 2., 3.],
        )
        .unwrap(),
        NurbsCurve::try_new_rational(
            2,
            [(0., 1.), (1., -1.), (4., 1.)]
                .map(|(x, w)| WeightedPoint3::try_new(point(x), w).unwrap())
                .to_vec(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap(),
    ];
    for source in curves {
        assert!(proposal(&source, 1e-9).is_none());
    }
}

#[test]
fn nearly_linear_replacements_require_a_whole_curve_bound() {
    let error = 1e-10;
    let source = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., error, 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![5., 5., 5., 11., 11., 11.],
    )
    .unwrap();
    assert!(proposal(&source, 0.).is_none());
    assert!(proposal(&source, error / 4.).is_none());
    let (result, bound) = proposal(&source, error).unwrap();
    assert!((error / 2. ..=error).contains(&bound));
    for i in 0..=128 {
        let t = i as Real / 128.;
        let a = source.evaluate(source.parameter_at(t).unwrap()).unwrap();
        let b = result.evaluate(result.parameter_at(t).unwrap()).unwrap();
        assert!(a.distance_to(b).unwrap() <= bound);
    }
}

#[test]
fn cleanup_covers_every_incident_trim_without_changing_surfaces_or_join_policy() {
    for source in [
        super::super::tests::cube(),
        Brep::try_cylinder(super::super::tests::frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap(),
    ] {
        let original = source.clone();
        assert_eq!(
            source
                .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
                .unwrap(),
            source
        );
        let result = source.try_cleanup_edges(1e-10, Tolerance::DEFAULT).unwrap();
        assert_eq!(source, original);
        assert_eq!(result.vertices, source.vertices);
        assert_eq!(result.edges.len(), source.edges.len());
        assert_eq!(result.edge_use_counts(), source.edge_use_counts());
        for (a, b) in result.faces.iter().zip(&source.faces) {
            assert_eq!(a.surface, b.surface);
            assert_eq!(a.reversed, b.reversed);
        }
        for (usage, original) in result.trim_uses().iter().zip(source.trim_uses()) {
            let Some(edge) = usage.trim.edge else {
                continue;
            };
            if usage.trim.trim_type == BrepTrimType::Seam {
                assert_eq!(usage.trim.curve, original.trim.curve);
                continue;
            }
            if certificate::linear_endpoints(&result.edges[edge].curve).is_some() {
                assert_eq!(usage.trim.curve.domain(), result.edges[edge].curve.domain());
                assert_eq!(usage.trim.curve.degree(), 1);
                assert_eq!(usage.trim.curve.control_points().len(), 2);
            }
        }
        assert_eq!(
            result.try_cleanup_edges(1e-10, Tolerance::DEFAULT).unwrap(),
            result
        );
        assert!(
            merge_with_cleanup(
                &source,
                1e-10,
                Tolerance::DEFAULT,
                &mut Budget(0),
                true,
                None
            )
            .is_err()
        );
    }
}

#[test]
fn rational_uv_segments_simplify_by_exact_locus_not_parameter_speed() {
    for sign in [-1., 1.] {
        let mut source = super::super::tests::cube();
        for face in &mut source.faces {
            for ring in &mut face.loops {
                for trim in &mut ring.trims {
                    let a = trim.curve.start_point().unwrap();
                    let b = trim.curve.end_point().unwrap();
                    let middle =
                        Point2::try_new(0.75 * a.x() + 0.25 * b.x(), 0.75 * a.y() + 0.25 * b.y())
                            .unwrap();
                    trim.curve = NurbsCurve2::try_new_rational(
                        2,
                        [(a, sign), (middle, 3. * sign), (b, 2. * sign)]
                            .map(|(p, w)| crate::WeightedPoint2::try_new(p, w).unwrap())
                            .to_vec(),
                        vec![5., 5., 5., 11., 11., 11.],
                    )
                    .unwrap();
                }
            }
        }
        source.validate(Tolerance::DEFAULT).unwrap();
        let result = source.try_cleanup_edges(1e-10, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.vertices, source.vertices);
        assert_eq!(result.edges, source.edges);
        for (a, b) in result.trim_uses().iter().zip(source.trim_uses()) {
            assert_eq!(a.trim.curve.degree(), 1);
            assert_eq!(a.trim.curve.control_points().len(), 2);
            assert!(
                a.trim
                    .curve
                    .control_points()
                    .iter()
                    .all(|p| p.weight() == 1.)
            );
            assert_eq!(
                a.trim.curve.start_point().unwrap(),
                b.trim.curve.start_point().unwrap()
            );
            assert_eq!(
                a.trim.curve.end_point().unwrap(),
                b.trim.curve.end_point().unwrap()
            );
        }
    }
}

#[test]
fn simplification_shares_previous_displacement_and_retains_exact_uv_loci() {
    let source = super::super::tests::cube();
    let tolerance = Tolerance::try_new(1e-3, 1e-12, 1e-10).unwrap();
    for prior in [0., 0.75e-3] {
        let mut state = State::new(&source, &mut Budget(MAX_WORK)).unwrap();
        let edge = state.edges[0].as_mut().unwrap();
        let [a, b] = edge.geometry.vertices.map(|v| source.vertices[v].point);
        let middle = a.midpoint(b).unwrap();
        // The first box edge is parallel to X; perturb perpendicular to it.
        assert_eq!(a.y(), b.y());
        let middle = Point3::try_new(middle.x(), middle.y() + 1e-3, middle.z()).unwrap();
        let curve =
            NurbsCurve::try_new(2, vec![a, middle, b], vec![0., 0., 0., 1., 1., 1.]).unwrap();
        edge.geometry.curve = curve.clone();
        edge.geometry.tolerance = 2e-3;
        edge.displacement = prior;
        let use_index = edge.uses[0];
        let trim = &mut state.trims[use_index].as_mut().unwrap().geometry;
        let a = trim.curve.start_point().unwrap();
        let b = trim.curve.end_point().unwrap();
        let nonlinear = NurbsCurve2::try_new(
            2,
            vec![
                a,
                Point2::try_new((a.x() + b.x()) / 2. + 1e-14, (a.y() + b.y()) / 2. + 1e-14)
                    .unwrap(),
                b,
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        trim.curve = nonlinear.clone();
        state
            .simplify_linear_edges(tolerance, &mut Budget(MAX_WORK))
            .unwrap();
        let edge = state.edges[0].as_ref().unwrap();
        assert_eq!(state.trim(use_index).geometry.curve, nonlinear);
        if prior == 0. {
            assert_eq!(edge.geometry.curve.degree(), 1);
            assert!(edge.displacement >= 0.5e-3);
            assert!(edge.displacement <= tolerance.absolute());
            assert!(edge.geometry.tolerance >= 2.5e-3);
        } else {
            assert_eq!(edge.geometry.curve, curve);
            assert_eq!(edge.displacement, prior);
            assert_eq!(edge.geometry.tolerance, 2e-3);
        }
    }
}
