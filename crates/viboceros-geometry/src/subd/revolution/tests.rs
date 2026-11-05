use super::*;
#[test]
fn angular_limit_stations_stay_on_the_circle_at_large_segment_counts() {
    let profile = NurbsCurve::try_new(
        1,
        [[3., 0., 0.], [3., 0., 5.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 5., 5.],
    )
    .unwrap();
    let original = profile.clone();
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let axis = UnitVector3::try_new(0., 0., 1., crate::Tolerance::DEFAULT).unwrap();
    for segments in [4, 1001] {
        for sweep in [std::f64::consts::TAU, 120_f64.to_radians()] {
            let subd =
                SubD::try_revolved_curve(&profile, origin, axis, 0.3, sweep, segments).unwrap();
            assert_eq!(subd.faces().len(), segments);
            for p in subd.surface_points() {
                assert!((p.x().hypot(p.y()) - 3.).abs() < 1e-11);
                assert!(p.z().abs() < 1e-11 || (p.z() - 5.).abs() < 1e-11);
            }
        }
    }
    for (start, sweep, segments) in [
        (f64::NAN, 1., 8),
        (0., 0., 8),
        (0., f64::INFINITY, 8),
        (0., 7., 8),
        (0., 1., 100001),
        (0., 1., usize::MAX),
    ] {
        assert!(SubD::try_revolved_curve(&profile, origin, axis, start, sweep, segments).is_err());
    }
    assert_eq!(profile, original);
}
