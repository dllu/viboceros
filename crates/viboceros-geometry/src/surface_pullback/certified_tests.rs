use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{One, Zero};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn a_sample_accepted_pullback_is_refined_until_its_complete_image_is_certified() {
    // S(u,v)=(u*(1-v/4),v,0), C(t)=(3t/4,t²,0). The cubic
    // Hermite proposal's error maximum lies between the 16 sample stations.
    let surface =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(0.75, 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let spatial = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0.375, 0., 0.), p(0.75, 1., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let initial_tolerance = Tolerance::try_new(1e-6, 1e-14, 1e-12).unwrap();
    let numeric = numerical_pullback_tolerance(initial_tolerance).unwrap();
    let start = pullback_node(&surface, &spatial, 0., 0., initial_tolerance, numeric).unwrap();
    let end = pullback_node(&surface, &spatial, 1., 1., initial_tolerance, numeric).unwrap();
    let proposal = hermite_segment(start, end).unwrap();
    let distance = |t| {
        let uv = evaluate_cubic(proposal.controls, t).unwrap();
        surface
            .evaluate(uv.x(), uv.y())
            .unwrap()
            .distance_to(spatial.evaluate(t).unwrap())
            .unwrap()
    };
    let sampled = (0..=16)
        .map(|i| distance(i as Real / 16.))
        .fold(0_f64, Real::max);
    let witnessed = (0..=1024)
        .map(|i| distance(i as Real / 1024.))
        .fold(0_f64, Real::max);
    assert!(witnessed > sampled);
    let tolerance = Tolerance::try_new((sampled + witnessed) * 0.5, 1e-14, 1e-12).unwrap();
    assert!(segment_matches_curve(&surface, &spatial, proposal, tolerance).unwrap());
    let old = surface.try_pullback_curve(&spatial, tolerance).unwrap();
    assert_eq!(old.control_points().len(), 4);
    assert_eq!(
        surface
            .parameter_curve_deviation_bound(&old, &spatial, tolerance.absolute())
            .unwrap(),
        None
    );
    let originals = (surface.clone(), spatial.clone());
    let fitted = surface
        .try_pullback_curve_certified(&spatial, tolerance)
        .unwrap();
    assert!(fitted.control_points().len() > old.control_points().len());
    assert!(
        surface
            .parameter_curve_deviation_bound(&fitted, &spatial, tolerance.absolute())
            .unwrap()
            .is_some()
    );
    assert_eq!((surface, spatial), originals);
}

pub(crate) fn station_excursion_surface() -> NurbsSurface {
    // z(u,v)=1e8 * v²(1-v)² * prod(u-i/16), i=1..15.
    // Along the diagonal all 17 sample stations vanish and the endpoint
    // tangents match a straight spatial curve. The actual image has excursions.
    let choose = |n: usize, k: usize| {
        (0..k.min(n - k)).fold(1_u64, |v, i| v * (n - i) as u64 / (i + 1) as u64)
    };
    let mut power = vec![Rational::one()];
    for i in 1..=15 {
        let root = rational(i as Real / 16.);
        let mut next = vec![Rational::zero(); power.len() + 1];
        for (j, c) in power.iter().enumerate() {
            next[j] -= &root * c;
            next[j + 1] += c;
        }
        power = next;
    }
    let z = (0..=15)
        .map(|i| {
            (0..=i)
                .map(|j| &power[j] * Rational::new(choose(i, j).into(), choose(15, j).into()))
                .sum::<Rational>()
        })
        .collect::<Vec<_>>();
    let controls = (0..=4)
        .flat_map(|v| {
            z.iter().enumerate().map(move |(i, z)| {
                let height = if v == 2 {
                    scalar(&(z * rational(1e8) / rational(6.))).unwrap()
                } else {
                    0.
                };
                p(i as Real / 15., v as Real / 4., height)
            })
        })
        .collect();
    NurbsSurface::try_new(
        15,
        4,
        16,
        5,
        controls,
        [vec![0.; 16], vec![1.; 16]].concat(),
        [vec![0.; 5], vec![1.; 5]].concat(),
    )
    .unwrap()
}

#[test]
fn certified_fitting_rejects_a_surface_excursion_hidden_at_every_sample_station() {
    let surface = station_excursion_surface();
    let spatial =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 1., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-14, 1e-12).unwrap();
    let old = surface.try_pullback_curve(&spatial, tolerance).unwrap();
    assert_eq!(old.control_points().len(), 4);
    let uv = old.evaluate(1. / 32.).unwrap();
    assert!(
        surface
            .evaluate(uv.x(), uv.y())
            .unwrap()
            .distance_to(spatial.evaluate(1. / 32.).unwrap())
            .unwrap()
            > 100. * tolerance.absolute()
    );
    let originals = (surface.clone(), spatial.clone());
    assert!(
        surface
            .try_pullback_curve_certified(&spatial, tolerance)
            .is_err()
    );
    assert_eq!((surface, spatial), originals);
}
