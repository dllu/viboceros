use super::*;
use crate::{AffineTransform3, Circle3, NurbsCurve, Point3, Polyline3};

fn p(a: [Real; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn v(a: [Real; 3]) -> Vector3 {
    Vector3::try_from(a).unwrap()
}
fn unit(a: [Real; 3]) -> UnitVector3 {
    v(a).normalized_nonzero().unwrap()
}
fn spatial() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        [[0., 0., 0.], [2., 0., 4.], [3., 4., -2.], [5., 2., 3.]]
            .map(p)
            .to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}
fn difference(a: UnitVector3, b: UnitVector3) -> Real {
    a.as_vector()
        .to_array()
        .into_iter()
        .zip(b.as_vector().to_array())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<Real>()
        .sqrt()
}
fn frames(curve: CurveRef<'_>, ts: &[Real]) -> Vec<Frame3> {
    curve
        .rotation_minimizing_frames(
            ts,
            Some(unit([0., 1., 0.])),
            FrameTransportOptions::default(),
        )
        .unwrap()
}

#[test]
fn continuous_multispan_curves_do_not_require_bit_identical_knot_points() {
    let c = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [1., 2., 3.],
            [2., -3., 5.],
            [3., 4., -1.],
            [4., -2., 4.],
            [5., 1., 2.],
        ]
        .map(p)
        .to_vec(),
        vec![0., 0., 0., 0., 1., 2., 3., 3., 3., 3.],
    )
    .unwrap();
    let result = frames(CurveRef::NurbsCurve(&c), &[0., 0.5, 1., 1.5, 2., 2.5, 3.]);
    assert_eq!(result.len(), 7);
}

#[test]
fn closed_spatial_paths_retain_holonomy_and_reverse_transport_is_an_inverse() {
    let mut controls = [
        [3., 0., 0.],
        [1., 3., 1.],
        [-2., 2., -1.],
        [-3., -1., 2.],
        [0., -3., 0.],
        [2., -2., -2.],
    ]
    .map(p)
    .to_vec();
    controls.extend_from_within(..3);
    let c = NurbsCurve::try_new(3, controls, (-3..=9).map(|i| i as Real).collect()).unwrap();
    assert!(c.is_closed().unwrap());
    let ts = (0..=16).map(|i| 6.0 * i as Real / 16.0).collect::<Vec<_>>();
    let result = frames(CurveRef::NurbsCurve(&c), &ts);
    assert!(difference(result[0].z_axis(), result[16].z_axis()) < 1e-14);
    assert!(difference(result[0].x_axis(), result[16].x_axis()) > 1e-4);
    let reversed = c.reversed().unwrap();
    let back = CurveRef::NurbsCurve(&reversed)
        .rotation_minimizing_frames(&[-6., 0.], Some(result[16].x_axis()), Default::default())
        .unwrap();
    assert!(difference(back[1].x_axis(), result[0].x_axis()) < 2e-10);
}

#[test]
fn seed_gauge_changes_do_not_change_relative_transport() {
    let c = spatial();
    let ts = [0., 0.25, 0.5, 0.75, 1.];
    let base = frames(CurveRef::NurbsCurve(&c), &ts);
    let changed = CurveRef::NurbsCurve(&c)
        .rotation_minimizing_frames(&ts, Some(base[0].y_axis()), Default::default())
        .unwrap();
    for (a, b) in base.iter().zip(changed) {
        assert!(difference(a.y_axis(), b.x_axis()) < 2e-10);
    }
}

#[test]
fn circles_keep_their_plane_normal_and_exact_tangent() {
    let circle =
        Circle3::try_new(p([0., 0., 0.]), 2.0, unit([0., 0., 1.]), Tolerance::DEFAULT).unwrap();
    let curve = CurveRef::Circle(&circle);
    let parameters = (0..=16)
        .map(|i| curve.parameter_at(i as Real / 16.).unwrap())
        .collect::<Vec<_>>();
    let result = curve
        .rotation_minimizing_frames(
            &parameters,
            Some(unit([0., 0., 1.])),
            FrameTransportOptions::default(),
        )
        .unwrap();
    for (frame, t) in result.into_iter().zip(parameters) {
        assert!(difference(frame.x_axis(), unit([0., 0., 1.])) < 1e-14);
        assert!(
            difference(
                frame.z_axis(),
                curve.evaluate_with_tangent(t).unwrap().tangent()
            ) < 1e-14
        );
        assert_eq!(frame.origin(), curve.evaluate(t).unwrap());
    }
}

#[test]
fn adaptive_transport_matches_an_independent_bishop_ode_integration() {
    let curve = spatial();
    let view = CurveRef::NurbsCurve(&curve);
    let actual = frames(view, &[0., 1.]);
    let mut x = actual[0].x_axis().as_vector().to_array();
    let rhs = |t, x: [Real; 3]| {
        let (_, d, dd) = view.evaluate_with_second_derivative(t).unwrap();
        let tangent = d.normalized_nonzero().unwrap().as_vector();
        let speed = d.length().unwrap();
        let along = dd.dot(tangent).unwrap();
        let derivative = std::array::from_fn::<_, 3, _>(|i| {
            (dd.to_array()[i] - along * tangent.to_array()[i]) / speed
        });
        let projection = x
            .into_iter()
            .zip(derivative)
            .map(|(a, b)| a * b)
            .sum::<Real>();
        tangent.to_array().map(|a| -projection * a)
    };
    let h = 1.0 / 4096.0;
    let add = |a: [Real; 3], b: [Real; 3], s: Real| std::array::from_fn(|i| a[i] + s * b[i]);
    for i in 0..4096 {
        let t = i as Real * h;
        let k1 = rhs(t, x);
        let k2 = rhs(t + h * 0.5, add(x, k1, h * 0.5));
        let k3 = rhs(t + h * 0.5, add(x, k2, h * 0.5));
        let k4 = rhs(t + h, add(x, k3, h));
        x = std::array::from_fn(|j| x[j] + h * (k1[j] + 2. * k2[j] + 2. * k3[j] + k4[j]) / 6.);
    }
    assert!(difference(actual[1].x_axis(), unit(x)) < 2e-10);
}

#[test]
fn output_density_does_not_set_the_transport_accuracy() {
    let c = spatial();
    let sparse = frames(CurveRef::NurbsCurve(&c), &[0., 0.5, 1.]);
    let ts = (0..=64).map(|i| i as Real / 64.).collect::<Vec<_>>();
    let dense = frames(CurveRef::NurbsCurve(&c), &ts);
    for (i, j) in [(0, 0), (1, 32), (2, 64)] {
        assert!(difference(sparse[i].x_axis(), dense[j].x_axis()) < 2e-10);
    }
}

#[test]
fn neighboring_float_queries_do_not_fail_at_natural_frame_seeds() {
    let c = spatial();
    let t = 1.0_f64 / 3.0;
    let next = f64::from_bits(t.to_bits() + 1);
    let actual = frames(CurveRef::NurbsCurve(&c), &[0., t, next, 1.]);
    let reference = frames(CurveRef::NurbsCurve(&c), &[0., 1.]);
    assert!(difference(actual[1].x_axis(), actual[2].x_axis()) < 1e-14);
    assert!(difference(actual[3].x_axis(), reference[1].x_axis()) < 2e-10);
}

#[test]
fn tangents_make_transport_translation_invariant_and_scale_independent() {
    let c = spatial();
    let ts = [0., 0.125, 0.25, 0.5, 0.875, 1.];
    let base = frames(CurveRef::NurbsCurve(&c), &ts);
    let moved = c
        .transformed(AffineTransform3::from_translation(v([1e12, -2e12, 3e12])))
        .unwrap();
    for (a, b) in base.iter().zip(frames(CurveRef::NurbsCurve(&moved), &ts)) {
        assert_eq!(a.axes(), b.axes());
    }
    for scale in [1e-140, 1e140] {
        let scaled = NurbsCurve::try_new(
            3,
            c.control_points()
                .iter()
                .map(|c| p(c.point().to_array().map(|a| a * scale)))
                .collect(),
            c.knots().to_vec(),
        )
        .unwrap();
        for (a, b) in base.iter().zip(frames(CurveRef::NurbsCurve(&scaled), &ts)) {
            assert!(difference(a.x_axis(), b.x_axis()) < 2e-10);
        }
    }
    let mapped = c.try_reparameterized(-3e80..=7e80).unwrap();
    let mapped_ts = ts.map(|t| mapped.parameter_at(t).unwrap());
    for (a, b) in base
        .iter()
        .zip(frames(CurveRef::NurbsCurve(&mapped), &mapped_ts))
    {
        assert!(difference(a.x_axis(), b.x_axis()) < 2e-10);
    }
}

#[test]
fn corners_are_transported_from_exact_one_sided_tangents() {
    let c = Polyline3::try_new(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]]
            .map(p)
            .to_vec(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let result = CurveRef::Polyline(&c)
        .rotation_minimizing_frames(
            &[0., 1., 2., 3.],
            Some(unit([0., 0., 1.])),
            FrameTransportOptions::default(),
        )
        .unwrap();
    assert!(difference(result[1].x_axis(), unit([0., 0., 1.])) < 1e-14);
    assert!(difference(result[2].x_axis(), unit([0., -1., 0.])) < 1e-14);
    assert!(difference(result[3].x_axis(), unit([0., -1., 0.])) < 1e-14);
    let left = CurveRef::Polyline(&c)
        .rotation_minimizing_frames(
            &[0., 1., 2., 3.],
            Some(unit([0., 0., 1.])),
            FrameTransportOptions {
                side: ParameterSide::Left,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(left[1].z_axis(), unit([1., 0., 0.]));
    assert_eq!(left[2].z_axis(), unit([0., 1., 0.]));
    assert!(difference(left[3].x_axis(), result[3].x_axis()) < 1e-14);
    let from_corner = CurveRef::Polyline(&c)
        .rotation_minimizing_frames(
            &[1., 2., 3.],
            Some(left[1].x_axis()),
            FrameTransportOptions {
                side: ParameterSide::Left,
                ..Default::default()
            },
        )
        .unwrap();
    for (a, b) in left[1..].iter().zip(from_corner) {
        assert!(difference(a.x_axis(), b.x_axis()) < 1e-14);
    }
}

#[test]
fn stationary_straight_endpoints_do_not_require_second_derivatives() {
    let c = NurbsCurve::try_new(
        2,
        [[0., 0., 0.], [0., 0., 0.], [1., 0., 0.]].map(p).to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let result = frames(CurveRef::NurbsCurve(&c), &[0., 0.5, 1.]);
    for f in result {
        assert_eq!(f.x_axis(), unit([0., 1., 0.]));
    }
}

#[test]
fn invalid_parameters_jumps_cusps_and_resource_exhaustion_fail_explicitly() {
    let c = spatial();
    let curve = CurveRef::NurbsCurve(&c);
    for ts in [
        vec![],
        vec![0., 0.],
        vec![1., 0.],
        vec![-0.1, 0.],
        vec![0., Real::NAN],
    ] {
        assert!(matches!(
            curve.rotation_minimizing_frames(&ts, None, FrameTransportOptions::default()),
            Err(GeometryError::InvalidCurveFrameParameters)
        ));
    }
    assert!(matches!(
        curve.rotation_minimizing_frames(
            &[0., 1.],
            None,
            FrameTransportOptions {
                maximum_evaluations: 8,
                ..Default::default()
            }
        ),
        Err(GeometryError::CurveFrameResourceLimit { .. })
    ));
    let cusp = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [0., 0., 0.]].map(p).to_vec(),
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    assert!(matches!(
        CurveRef::NurbsCurve(&cusp).rotation_minimizing_frames(&[0., 2.], None, Default::default()),
        Err(GeometryError::DiscontinuousCurveFrame)
    ));
    let jump = NurbsCurve::try_new(
        1,
        [[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]]
            .map(p)
            .to_vec(),
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    assert!(matches!(
        CurveRef::NurbsCurve(&jump).rotation_minimizing_frames(&[0., 2.], None, Default::default()),
        Err(GeometryError::DiscontinuousCurveFrame)
    ));
}
