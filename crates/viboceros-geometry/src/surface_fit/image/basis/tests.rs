use super::*;

#[test]
fn explicit_image_fits_share_solver_contracts_and_reject_invalid_maps_before_solving() {
    let source = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(4., 0., 0.).unwrap(),
        Point3::try_new(4., 6., 1.).unwrap(),
        Point3::try_new(0., 6., 0.).unwrap(),
    ])
    .unwrap();
    let before = source.clone();
    let knots = [0., 0., 0., 0., 1., 1., 1., 1.];
    let identity = [[0., 0.], [1., 1.]];
    let actual =
        try_fit_surface_image_in_basis(&source, [3; 2], [&knots; 2], [&identity; 2], 1e-12)
            .unwrap()
            .unwrap();
    assert!(actual.maximum_deviation_bound <= 1e-12);
    let mut calls = 0;
    let mut solve = |input: &NurbsInterpolationSystem<'_>| {
        calls += 1;
        crate::surface_rebuild::portable_solve(input)
    };
    let delegated = try_fit_surface_image_in_basis_with_solver(
        &source,
        [3; 2],
        [&knots; 2],
        [&identity; 2],
        1e-12,
        &mut solve,
    )
    .unwrap()
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(actual, delegated);
    let mut never = |_: &NurbsInterpolationSystem<'_>| -> Result<Vec<Real>, GeometryError> {
        panic!("invalid request reached solver")
    };
    for limit in [-1., Real::INFINITY, Real::NAN] {
        assert_eq!(
            try_fit_surface_image_in_basis_with_solver(
                &source,
                [3; 2],
                [&knots; 2],
                [&identity; 2],
                limit,
                &mut never
            ),
            Err(GeometryError::InvalidTolerance)
        );
    }
    assert_eq!(
        try_fit_surface_image_in_basis_with_solver(
            &source,
            [3; 2],
            [&knots; 2],
            [&[], &identity],
            1.,
            &mut never
        ),
        Err(GeometryError::InvalidSurfaceCorrespondence)
    );
    let malformed = try_fit_surface_image_in_basis_with_solver(
        &source,
        [3; 2],
        [&knots; 2],
        [&identity; 2],
        1.,
        &mut |_| -> Result<Vec<Real>, GeometryError> { Ok(vec![]) },
    );
    assert!(matches!(
        malformed,
        Err(GeometryError::InterpolationBackendFailed { .. })
    ));
    assert_eq!(source, before);
}
