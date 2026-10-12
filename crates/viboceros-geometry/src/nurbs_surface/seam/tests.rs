use super::*;

fn periodic() -> NurbsSurface {
    let row = [
        (0., 0.),
        (2., 0.),
        (2., 2.),
        (0., 2.),
        (0., 0.),
        (2., 0.),
        (2., 2.),
    ];
    NurbsSurface::try_new(
        3,
        1,
        7,
        2,
        [0., 3.]
            .into_iter()
            .flat_map(|z| row.map(|(x, y)| Point3::try_new(x, y, z).unwrap()))
            .collect(),
        vec![0., 0., 1., 2., 3., 4., 5., 6., 7., 8., 8.],
        vec![10., 10., 17., 17.],
    )
    .unwrap()
}

#[test]
fn seam_policy_calls_whole_tensor_refinement_once_and_rejects_changed_layouts() {
    let source = periodic();
    let before = source.clone();
    let mut calls = 0;
    let result = source
        .try_change_closed_seam_with_operations(
            SurfaceKnotDirection::U,
            3.25,
            Real::NAN,
            &mut |s, d, p, t| {
                calls += 1;
                assert_eq!(d, SurfaceKnotDirection::U);
                assert_eq!(s.control_points().len(), 14);
                s.try_insert_knot_u(p, t)
            },
            &mut |_, _, _, _| -> Result<_, GeometryError> {
                panic!("periodic simple seam must refine")
            },
        )
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(result.knots_v(), source.knots_v());
    assert_eq!(source, before);
    let invalid = source.try_change_closed_seam_with_operations(
        SurfaceKnotDirection::U,
        3.25,
        Real::NAN,
        &mut |s, _, _, _| s.try_swapped_uv(),
        &mut |_, _, _, _| -> Result<_, GeometryError> {
            panic!("invalid layout must fail before split")
        },
    );
    assert!(matches!(
        invalid,
        Err(GeometryError::InvalidControlNet { .. })
    ));
}

#[test]
fn clamped_seam_calls_one_split_and_invalid_or_endpoint_requests_avoid_backend_entry() {
    let source = periodic()
        .try_make_non_periodic(SurfaceKnotDirection::U)
        .unwrap();
    let mut splits = 0;
    source
        .try_change_closed_seam_with_operations(
            SurfaceKnotDirection::U,
            3.25,
            Real::NAN,
            &mut |_, _, _, _| -> Result<_, GeometryError> { panic!("clamped seam must split") },
            &mut |s, d, p, periodic| {
                splits += 1;
                assert!(!periodic);
                s.split_seam_control_curves(d, p, periodic)
            },
        )
        .unwrap();
    assert_eq!(splits, 1);
    for p in [2., 6., Real::NAN, 1., 7.] {
        let result = source.try_change_closed_seam_with_operations(
            SurfaceKnotDirection::U,
            p,
            Real::NAN,
            &mut |_, _, _, _| -> Result<_, GeometryError> { panic!("request entered refinement") },
            &mut |_, _, _, _| -> Result<_, GeometryError> { panic!("request entered split") },
        );
        assert_eq!(result.is_ok(), p == 2. || p == 6.);
    }
}
