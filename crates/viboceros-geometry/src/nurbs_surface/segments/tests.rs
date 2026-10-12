use super::*;

fn plane() -> NurbsSurface {
    NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(4., 0., 0.).unwrap(),
        Point3::try_new(4., 6., 0.).unwrap(),
        Point3::try_new(0., 6., 0.).unwrap(),
    ])
    .unwrap()
}

#[test]
fn rectangular_extraction_calls_u_then_v_once_and_preserves_its_source() {
    let source = plane();
    let before = source.clone();
    let mut calls = Vec::new();
    let actual = source
        .try_trimmed_with_extractor(0.2..=0.8, 0.25..=0.75, &mut |s, d, r| {
            calls.push(d);
            match d {
                SurfaceKnotDirection::U => s.try_trimmed_u(r),
                SurfaceKnotDirection::V => s.try_trimmed_v(r),
                SurfaceKnotDirection::Both => unreachable!(),
            }
        })
        .unwrap();
    assert_eq!(
        calls,
        vec![SurfaceKnotDirection::U, SurfaceKnotDirection::V]
    );
    assert_eq!(actual.domain_u(), 0.2..=0.8);
    assert_eq!(actual.domain_v(), 0.25..=0.75);
    assert_eq!(source, before);
}

#[test]
fn full_domain_noops_and_invalid_bounds_avoid_backend_entry_and_changed_layouts_fail() {
    let source = plane();
    let mut reject = |_: &NurbsSurface, _, _| -> Result<NurbsSurface, GeometryError> {
        panic!("request entered backend")
    };
    assert_eq!(
        source
            .try_trimmed_with_extractor(source.domain_u(), source.domain_v(), &mut reject)
            .unwrap(),
        source
    );
    for (u, v) in [(0.2..=0.8, 0.0..=2.0), (Real::NAN..=1.0, 0.0..=1.0)] {
        assert!(
            source
                .try_trimmed_with_extractor(u, v, &mut reject)
                .is_err()
        );
    }
    let wrong = source.try_trimmed_direction_with_extractor(
        SurfaceKnotDirection::U,
        0.2..=0.8,
        &mut |s, _, _| s.try_reparameterized(0.2..=0.8, 10.0..=20.0),
    );
    assert!(matches!(
        wrong,
        Err(GeometryError::InvalidControlNet { .. })
    ));
}

#[test]
fn splitting_uses_one_whole_tensor_callback_and_propagates_backend_errors() {
    #[derive(Debug, PartialEq)]
    enum ProbeError {
        Geometry(GeometryError),
        Backend,
    }
    impl From<GeometryError> for ProbeError {
        fn from(error: GeometryError) -> Self {
            Self::Geometry(error)
        }
    }
    let source = plane();
    let mut calls = 0;
    let (left, right) = source
        .try_split_direction_with_extractor(SurfaceKnotDirection::U, 0.5, &mut |s, d, p| {
            calls += 1;
            assert_eq!(d, SurfaceKnotDirection::U);
            s.try_split_u(p)
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(left.domain_u(), 0.0..=0.5);
    assert_eq!(right.domain_u(), 0.5..=1.0);
    assert_eq!(left.knots_v(), source.knots_v());
    assert_eq!(right.knots_v(), source.knots_v());
    let error = source.try_split_direction_with_extractor(
        SurfaceKnotDirection::U,
        0.5,
        &mut |_, _, _| -> Result<_, ProbeError> { Err(ProbeError::Backend) },
    );
    assert_eq!(error, Err(ProbeError::Backend));
}
