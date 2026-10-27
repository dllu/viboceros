use super::*;
use crate::Point3;

fn polygon(knots: Vec<Real>, z: Real) -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        [0., 0.25, 1.]
            .map(|x| Point3::try_new(x, 0., z).unwrap())
            .to_vec(),
        knots,
    )
    .unwrap()
}

#[test]
fn retimed_and_refined_basis_retains_the_complete_source_allowance() {
    let prior = polygon(vec![0., 0., 0.5, 1., 1.], 0.);
    for domain in [0. ..=1., -3. ..=17., 100. ..=107.] {
        let prior = prior.try_reparameterized(domain).unwrap();
        let chart = polygon(vec![0., 0., 0.2, 1., 1.], 0.);
        let matched = chart.try_insert_knot(0.3, 1).unwrap();
        let before = (prior.clone(), chart.clone(), matched.clone());
        assert!(within_remaining_deviation(&prior, &chart, &matched, 1e-6, 2e-6).unwrap());
        assert_eq!((prior, chart, matched), before);
    }
}

#[test]
fn basis_edits_charge_the_source_error_against_the_same_total_budget() {
    let prior = polygon(vec![0., 0., 0.5, 1., 1.], 0.);
    let edited = polygon(prior.knots().to_vec(), 0.75);
    assert!(
        prior
            .parameter_curve_deviation_bound(&edited, 1.)
            .unwrap()
            .is_some()
    );
    assert!(!within_remaining_deviation(&prior, &prior, &edited, 0.5, 1.).unwrap());
    assert!(within_remaining_deviation(&prior, &prior, &edited, 0.125, 1.).unwrap());
    for deviation in [1., 2., -1., Real::NAN, Real::INFINITY] {
        assert!(!within_remaining_deviation(&prior, &prior, &prior, deviation, 1.).unwrap());
    }
    // Subtracting this positive bound rounds back to 1. in round-to-nearest.
    let edited = polygon(prior.knots().to_vec(), 1.);
    assert!(!within_remaining_deviation(&prior, &prior, &edited, Real::EPSILON / 8., 1.).unwrap());
}

#[test]
fn correspondence_knots_and_endpoints_do_not_hide_a_basis_edit_bump() {
    let prior = NurbsCurve::try_new(
        3,
        [0., 1. / 3., 2. / 3., 1.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let edited = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [1. / 3., 1., 0.],
            [2. / 3., -1., 0.],
            [1., 0., 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        prior.knots().to_vec(),
    )
    .unwrap();
    for fraction in [0., 0.5, 1.] {
        assert!(
            prior
                .evaluate(fraction)
                .unwrap()
                .distance_to(edited.evaluate(fraction).unwrap())
                .unwrap()
                < 1e-15
        );
    }
    assert!(!within_remaining_deviation(&prior, &prior, &edited, 1e-5, 0.001).unwrap());
}
