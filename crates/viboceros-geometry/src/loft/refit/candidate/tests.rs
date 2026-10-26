use super::*;
use crate::Point3;

fn polynomial(ys: &[Real]) -> NurbsCurve {
    let degree = ys.len() - 1;
    NurbsCurve::try_new(
        degree,
        ys.iter()
            .enumerate()
            .map(|(i, &y)| Point3::try_new(i as Real, y, 0.).unwrap())
            .collect(),
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
    )
    .unwrap()
}

#[test]
fn admitted_smooth_cubics_have_a_complete_half_tolerance_certificate() {
    let source = NurbsCurve::try_new(
        3,
        [[0., 0., 0.], [1., 2., 0.], [3., -1., 0.], [4., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        [vec![0.; 4], vec![1.; 4]].concat(),
    )
    .unwrap();
    let sources = vec![source; 2];
    let before = sources.clone();
    let fitted = try_prepare_refitted_loft_sections(&sources, 0.1).unwrap();
    for (source, candidate) in sources.iter().zip(fitted) {
        assert!(
            source
                .parameter_curve_deviation_bound(&candidate, 0.05)
                .unwrap()
                .is_some()
        );
        assert!(candidate.control_points().iter().all(|c| c.weight() == 1.));
    }
    assert_eq!(sources, before);
    assert!(try_prepare_refitted_loft_sections(&sources, 0.001).is_err());
    assert_eq!(sources, before);
}

#[test]
fn endpoint_and_midpoint_agreement_cannot_hide_an_interior_bump() {
    // Horizontal endpoint tangents propose a straight cubic. Both endpoints
    // and the midpoint agree exactly; the complete polynomial does not.
    let source = polynomial(&[0., 0., 13., -13., 0., 0.]);
    let candidate = crate::curve_fit::g1::initial_cubic(&source).unwrap();
    for t in [0., 0.5, 1.] {
        assert!(
            source
                .evaluate(t)
                .unwrap()
                .distance_to(candidate.evaluate(t).unwrap())
                .unwrap()
                < 1e-14
        );
    }
    assert!(
        source
            .evaluate(0.25)
            .unwrap()
            .distance_to(candidate.evaluate(0.25).unwrap())
            .unwrap()
            > 1.
    );
    let sources = vec![source; 2];
    let before = sources.clone();
    assert!(try_prepare_refitted_loft_sections(&sources, 0.1).is_err());
    assert_eq!(sources, before);
}
