use super::*;
fn profile() -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        [
            [3., 0., 0.],
            [4., 0., 1.],
            [3., 0., 2.],
            [2., 0., 1.],
            [3., 0., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 1., 2., 3., 4., 4.],
    )
    .unwrap()
}
#[test]
fn command_linear_caps_preserve_source_and_outward_volume_in_both_directions() {
    let curve = profile();
    let before = curve.clone();
    for sign in [-1., 1.] {
        for split in [false, true] {
            let start = Vector3::try_new(0., -3. * sign, 0.).unwrap();
            let end = Vector3::try_new(0., 3. * sign, 0.).unwrap();
            let result = Brep::try_command_linear_solid_with(
                &curve,
                start,
                end,
                split,
                Tolerance::DEFAULT,
                &mut NurbsSurface::try_extruded_curve,
            )
            .unwrap();
            assert!(result.is_solid());
            assert!(result.is_manifold());
            assert_eq!(result.faces().len(), if split { 6 } else { 3 });
            assert!((result.signed_volume(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-12);
        }
    }
    assert_eq!(curve, before);
    let mut calls = 0;
    let failed = Brep::try_command_linear_solid_with(
        &curve,
        Vector3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 3., 0.).unwrap(),
        true,
        Tolerance::DEFAULT,
        &mut |_, _, _| {
            calls += 1;
            Err::<NurbsSurface, _>(GeometryError::NumericalIntegrationDidNotConverge)
        },
    );
    assert_eq!(calls, 1);
    assert_eq!(
        failed,
        Err(GeometryError::NumericalIntegrationDidNotConverge)
    );
    assert_eq!(curve, before);
}
#[test]
fn polynomial_edge_gauges_leave_parameterized_loci_and_topology_unchanged() {
    for weight in [2., -2.] {
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [[0., 0., 0.], [3., 0., 0.], [0., 2., 1.], [3., 2., 1.]]
                .into_iter()
                .map(|p| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), weight).unwrap())
                .collect(),
            vec![0., 0., 3., 3.],
            vec![0., 0., 2., 2.],
        )
        .unwrap();
        let source = Brep::try_command_extrusion_sheet(surface, Tolerance::DEFAULT).unwrap();
        let result = source
            .clone()
            .try_normalized_polynomial_edge_weights()
            .unwrap();
        assert_eq!(source.vertices(), result.vertices());
        assert_eq!(source.faces(), result.faces());
        for (a, b) in source.edges().iter().zip(result.edges()) {
            assert_eq!(a.vertices(), b.vertices());
            assert_eq!(a.curve().knots(), b.curve().knots());
            for i in 0..=16 {
                let t = a.curve().parameter_at(i as f64 / 16.).unwrap();
                assert_eq!(
                    a.curve().evaluate(t).unwrap(),
                    b.curve().evaluate(t).unwrap()
                );
            }
            assert!(b.curve().control_points().iter().all(|p| p.weight() == 1.));
        }
    }
}
