use super::*;
use crate::{Point3, SurfaceJet2};
fn v(a: [Real; 3]) -> Vector3 {
    Vector3::try_from(a).unwrap()
}
fn jet(scales: [Real; 2]) -> SurfaceJet3 {
    // S(u,v)=(u+2v,v,u²+uv+v²+u³+2u²v+3uv²+4v³) at zero.
    let [u, w] = scales;
    SurfaceJet3 {
        second: SurfaceJet2 {
            point: Point3::try_new(0., 0., 0.).unwrap(),
            derivative_u: v([u, 0., 0.]),
            derivative_v: v([2. * w, w, 0.]),
            derivative_uu: v([0., 0., 2. * u * u]),
            derivative_uv: v([0., 0., u * w]),
            derivative_vv: v([0., 0., 2. * w * w]),
        },
        derivative_uuu: v([0., 0., 6. * u * u * u]),
        derivative_uuv: v([0., 0., 4. * u * u * w]),
        derivative_uvv: v([0., 0., 6. * u * w * w]),
        derivative_vvv: v([0., 0., 24. * w * w * w]),
    }
}
#[test]
fn directional_contraction_solves_the_nonorthogonal_metric_and_reverses_odd_targets() {
    let d = v([3., 4., 0.]).normalized_nonzero().unwrap();
    let out = jet([1., 1.]).normal_parameter_line_jet(d).unwrap();
    let (a, b) = (-1_f64, 0.8_f64); // (a+2b,b)=(0.6,0.8)
    let second = 2. * a * a + 2. * a * b + 2. * b * b;
    let third = 6. * a * a * a + 12. * a * a * b + 18. * a * b * b + 24. * b * b * b;
    assert!((out.curvature.z() - second).abs() < 1e-12);
    assert!((out.third_derivative.z() - third).abs() < 1e-12);
    let reversed = jet([1., 1.])
        .normal_parameter_line_jet(d.opposite())
        .unwrap();
    assert_eq!(out.curvature, reversed.curvature);
    assert!((out.third_derivative.z() + reversed.third_derivative.z()).abs() < 1e-12);
}
#[test]
fn affine_chart_scaling_preserves_normalized_targets() {
    let d = v([3., 4., 0.]).normalized_nonzero().unwrap();
    let base = jet([1., 1.]).normal_parameter_line_jet(d).unwrap();
    for scales in [[2., 0.5], [1e90, 1e-90], [1e-90, 1e90]] {
        let out = jet(scales).normal_parameter_line_jet(d).unwrap();
        for (a, b) in out
            .curvature
            .to_array()
            .into_iter()
            .zip(base.curvature.to_array())
            .chain(
                out.third_derivative
                    .to_array()
                    .into_iter()
                    .zip(base.third_derivative.to_array()),
            )
        {
            assert!((a - b).abs() < 1e-11, "{scales:?}: {a} {b}");
        }
    }
    assert!(
        jet([1., 1.])
            .normal_parameter_line_jet(v([0., 0., 1.]).normalized_nonzero().unwrap())
            .is_err()
    );
}

#[test]
fn third_target_removes_the_parameter_lines_tangential_acceleration() {
    let mut j = jet([1., 1.]);
    j.second.derivative_v = v([0., 1., 0.]);
    j.second.derivative_uu = v([4., 0., 2.]);
    j.derivative_uuu = v([0., 0., 6.]);
    let out = j
        .normal_parameter_line_jet(v([1., 0., 0.]).normalized_nonzero().unwrap())
        .unwrap();
    assert_eq!(out.curvature, v([0., 0., 2.]));
    assert_eq!(out.third_derivative, v([0., 0., -18.]));
}
