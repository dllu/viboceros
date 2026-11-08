use super::*;

#[test]
fn a_large_control_net_retains_original_basis_and_does_not_require_a_dense_tensor_matrix() {
    let n = 65;
    let p = 3;
    let knots = [
        vec![0.; p + 1],
        (1..n - p).map(|i| i as Real).collect(),
        vec![(n - p) as Real; p + 1],
    ]
    .concat();
    let points = (0..n)
        .flat_map(|j| (0..n).map(move |i| Point3::try_new(i as Real, j as Real, 0.).unwrap()))
        .collect();
    let source = NurbsSurface::try_new(p, p, n, n, points, knots.clone(), knots).unwrap();
    let result = source.try_loose_offset(0.25, Tolerance::DEFAULT).unwrap();
    assert_eq!(source.knots_u(), result.knots_u());
    assert_eq!(source.knots_v(), result.knots_v());
    for (a, b) in source.control_points().iter().zip(result.control_points()) {
        assert_eq!(a.weight(), b.weight());
        assert!(
            a.point()
                .translated(Vector3::try_new(0., 0., 0.25).unwrap())
                .unwrap()
                .distance_to(b.point())
                .unwrap()
                < 1e-12
        );
    }
}

#[test]
fn mixed_weights_and_undefined_normals_decline_without_mutating_the_source() {
    let p = Point3::try_new(0., 0., 0.).unwrap();
    let source = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![p; 4],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    assert!(source.try_loose_offset(1., Tolerance::DEFAULT).is_err());
    assert_eq!(source, before);
    assert_eq!(
        source.try_loose_offset(0., Tolerance::DEFAULT).unwrap(),
        source
    );
    let mut controls = source.control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(p, -1.).unwrap();
    let mixed = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        controls,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        mixed.try_loose_offset(1., Tolerance::DEFAULT),
        Err(GeometryError::UnsupportedLooseSurfaceOffset { .. })
    ));
}
