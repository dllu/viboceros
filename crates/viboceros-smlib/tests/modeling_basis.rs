#![cfg(feature = "native")]
use viboceros_geometry::{
    AffineTransform3, NurbsCurve, Point3, Tolerance, UnitVector3, Vector3, WeightedPoint3,
};
use viboceros_smlib::KernelCurve;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn profile(scale: f64) -> NurbsCurve {
    let close = 0.5 + 1e-14;
    NurbsCurve::try_new_rational(
        2,
        [
            [2., 0., 0.],
            [3., 0., 0.],
            [4., 0., 0.],
            [4., 0., 10.],
            [5., 0., 0.],
            [6., 0., 0.],
            [7., 0., 0.],
        ]
        .into_iter()
        .zip([1., 0.75, 1., 0.5, 1., 0.75, 1.])
        .map(|(q, w)| WeightedPoint3::try_new(p(q[0], q[1], q[2]), w).unwrap())
        .collect(),
        [0., 0., 0., 0.5, 0.5, close, close, 1., 1., 1.]
            .map(|t| t * scale)
            .to_vec(),
    )
    .unwrap()
}
fn parameters(scale: f64) -> [f64; 7] {
    let close = 0.5 + 1e-14;
    [0., 0.25, 0.5, (0.5 + close) * 0.5, close, 0.75, 1.].map(|t| t * scale)
}
fn scales() -> [f64; 3] {
    [2f64.powi(-360), 1., 2f64.powi(360)]
}

#[test]
fn extruded_sheets_preserve_short_profile_spans_and_extreme_charts() {
    let offset = Vector3::try_new(1., 3., 4.).unwrap();
    for scale in scales() {
        let source = profile(scale);
        let native = KernelCurve::from_nurbs(&source).unwrap();
        let result = native.extruded_surface(offset).unwrap();
        assert_eq!(result.knots_u(), source.knots());
        for t in parameters(scale) {
            let expected = source.evaluate(t).unwrap();
            for f in [0., 0.25, 0.5, 1.] {
                let value = result
                    .evaluate(t, result.parameter_at_v(f).unwrap())
                    .unwrap();
                assert!(
                    value
                        .distance_to(expected.translated(offset.scaled(f).unwrap()).unwrap())
                        .unwrap()
                        < 1e-10
                );
            }
            assert!(native.evaluate(t).unwrap().distance_to(expected).unwrap() < 1e-11);
        }
    }
}

#[test]
fn apex_sheets_preserve_native_copies_and_short_profile_spans() {
    let apex = p(1., 4., 7.);
    for scale in scales() {
        let source = profile(scale);
        let native = KernelCurve::from_nurbs(&source).unwrap();
        let result = native.apex_surface(apex).unwrap();
        assert_eq!(result.knots_v(), source.knots());
        for t in parameters(scale) {
            let point = source.evaluate(t).unwrap();
            for f in [0., 0.25, 0.5, 1.] {
                let expected = point
                    .translated(point.vector_to(apex).unwrap().scaled(f).unwrap())
                    .unwrap();
                assert!(
                    result
                        .evaluate(result.parameter_at_u(f).unwrap(), t)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-10
                );
            }
            assert!(native.evaluate(t).unwrap().distance_to(point).unwrap() < 1e-11);
        }
    }
}

#[test]
fn translational_sheets_preserve_both_bases_and_the_copied_path() {
    for scale in scales() {
        let source = profile(scale);
        let path = profile(scale)
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(20., 3., 2.).unwrap(),
            ))
            .unwrap();
        let native = KernelCurve::from_nurbs(&source).unwrap();
        let rail = KernelCurve::from_nurbs(&path).unwrap();
        let result = native.translational_sweep(&rail).unwrap();
        assert_eq!(result.knots_u(), source.knots());
        assert_eq!(result.knots_v(), path.knots());
        let start = path.evaluate(*path.domain().start()).unwrap();
        for u in parameters(scale) {
            for v in parameters(scale) {
                let expected = source
                    .evaluate(u)
                    .unwrap()
                    .translated(start.vector_to(path.evaluate(v).unwrap()).unwrap())
                    .unwrap();
                assert!(
                    result
                        .evaluate(u, v)
                        .unwrap()
                        .distance_to(expected)
                        .unwrap()
                        < 1e-10
                );
            }
            assert!(
                rail.evaluate(u)
                    .unwrap()
                    .distance_to(path.evaluate(u).unwrap())
                    .unwrap()
                    < 1e-11
            );
        }
    }
}

#[test]
fn revolutions_preserve_short_spans_without_canonical_row_averaging() {
    let t = Tolerance::DEFAULT;
    let origin = p(0., 0., 0.);
    let axis = Vector3::try_new(0., 0., 1.).unwrap();
    let rotation = AffineTransform3::try_rotation(
        origin,
        UnitVector3::try_new(0., 0., 1., t).unwrap(),
        120f64.to_radians(),
    )
    .unwrap();
    for scale in scales() {
        let source = profile(scale);
        let native = KernelCurve::from_nurbs(&source).unwrap();
        let result = native.revolved_surface(origin, axis, 120., 7.).unwrap();
        assert_eq!(result.knots_v(), source.knots());
        for v in parameters(scale) {
            let expected = source.evaluate(v).unwrap();
            assert!(
                result
                    .evaluate(0., v)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-10
            );
            assert!(
                result
                    .evaluate(7., v)
                    .unwrap()
                    .distance_to(rotation.transform_point(expected).unwrap())
                    .unwrap()
                    < 1e-10
            );
            for f in [0.25, 0.5, 0.75] {
                let point = result
                    .evaluate(result.parameter_at_u(f).unwrap(), v)
                    .unwrap();
                assert!(
                    (point.x().hypot(point.y()) - expected.x().hypot(expected.y())).abs() < 1e-10
                );
                assert!((point.z() - expected.z()).abs() < 1e-10);
            }
        }
    }
}
