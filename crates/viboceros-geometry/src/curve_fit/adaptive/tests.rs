use super::super::interpolate::{
    constrain_endpoint_handles, constrain_kink_handles, solve_cubic_fit_controls,
    solve_dense_fit_controls,
};
use super::*;
use crate::Polyline3;

#[test]
fn cubic_banded_fit_matches_dense_elimination_with_fixed_kink_handles() {
    let spatial = NurbsCurve::try_new(
        3,
        vec![
            point(1., 2., 0.),
            point(2., 0., 4.),
            point(3., 4., -2.),
            point(5., 2., 3.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let kinked = Polyline3::try_new(
        vec![
            point(0., 0., 0.),
            point(1., 0., 0.),
            point(1., 2., 0.),
            point(2., 2., 1.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for source in [CurveRef::NurbsCurve(&spatial), CurveRef::Polyline(&kinked)] {
        let arc = ArcLengthSampler::try_new(source, Tolerance::DEFAULT).unwrap();
        let source_kinks = arc.kinks(1e-10).unwrap();
        let sampler = FitSampler::try_new(arc, &[], None).unwrap();
        let kinks = sampler.map_kinks(&source_kinks).unwrap();
        let mut breaks = kinks
            .iter()
            .map(|k| FitBreak {
                distance: k.parameter,
                multiplicity: 3,
            })
            .collect::<Vec<_>>();
        for f in [0.03, 0.07, 0.23, 0.31, 0.44, 0.67, 0.88, 0.99] {
            insert_smooth_break(&mut breaks, f * sampler.end_parameter());
        }
        let n = fit_control_count(3, &breaks, MAX_CURVE_FIT_CONTROL_POINTS).unwrap();
        let knots = fit_knots(3, sampler.end_parameter(), &breaks, n);
        let mut banded = vec![None; n];
        constrain_endpoint_handles(&sampler, 3, &knots, &mut banded).unwrap();
        for kink in &kinks {
            constrain_kink_handles(&sampler, 3, &knots, kink, &mut banded).unwrap();
        }
        let mut dense = banded.clone();
        solve_cubic_fit_controls(&sampler, &knots, &mut banded, &[]).unwrap();
        solve_dense_fit_controls(&sampler, 3, &knots, &mut dense).unwrap();
        for (a, b) in banded.into_iter().zip(dense) {
            assert!(a.unwrap().distance_to(b.unwrap()).unwrap() < 2e-13);
        }
    }
}

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
