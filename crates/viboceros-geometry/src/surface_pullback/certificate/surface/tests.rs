use super::*;
use crate::{WeightedPoint2, WeightedPoint3};

#[test]
fn nonlinear_isocurve_images_equal_full_rational_tensor_products() {
    for gauge in [1., -2., 1e-280] {
        let surface = NurbsSurface::try_new_rational(
            2,
            3,
            3,
            4,
            (0..4)
                .flat_map(|j| {
                    (0..3).map(move |i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(i as Real / 2., j as Real / 4., (i * j) as Real / 8.)
                                .unwrap(),
                            [1., 0.5, 2., 0.25][(i + j) % 4] * gauge,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            [vec![-3.; 3], vec![5.; 3]].concat(),
            [vec![1e9; 4], vec![1e9 + 8.; 4]].concat(),
        )
        .unwrap();
        for fixed in 0..2 {
            for station in [0., 0.25, 1.] {
                for degree in [2, 3] {
                    let variable = 1 - fixed;
                    let uv = NurbsCurve2::try_new_rational(
                        degree,
                        (0..=degree)
                            .map(|i| {
                                let mut point = [0.; 2];
                                point[fixed] = station;
                                point[variable] = [0., 0.75, 0.25, 1.][i];
                                WeightedPoint2::try_new(
                                    Point2::try_new(-3. + 8. * point[0], 1e9 + 8. * point[1])
                                        .unwrap(),
                                    [1., 2., 0.5, 4.][i] * -gauge,
                                )
                                .unwrap()
                            })
                            .collect(),
                        [vec![-7.; degree + 1], vec![11.; degree + 1]].concat(),
                    )
                    .unwrap();
                    let mut budget = Budget(MAX_WORK);
                    let mut exact = Surface::new(&surface, &mut budget).unwrap().unwrap();
                    let parameters = curve::Spline::uv(&uv, &mut budget).unwrap().unwrap();
                    let controls = parameters
                        .extract(&Rational::zero(), &Rational::one(), &mut budget)
                        .unwrap();
                    // Exclude full-tensor preparation while conservatively
                    // charging the reduced form's first-use extraction.
                    exact.patch([2, 3], &mut budget).unwrap();
                    let before = budget.0;
                    let image = exact
                        .isocurve_image([2, 3], &controls, &mut budget)
                        .unwrap()
                        .unwrap();
                    let reduced_work = before - budget.0;
                    let before = budget.0;
                    let full = exact.tensor_image([2, 3], &controls, &mut budget).unwrap();
                    let tensor_work = before - budget.0;
                    assert!(reduced_work < tensor_work);
                    assert_eq!(image.len(), degree * exact.degree[variable] + 1);
                    assert_eq!(full.len(), degree * 5 + 1);
                    // Exact polynomial identity, including rational UV weights:
                    // all three cross-multiplied coordinates vanish on the
                    // complete interval, independently of homogeneous degree.
                    let difference = algebra::difference(&image, &full, &mut budget).unwrap();
                    assert!(difference.iter().all(|h| h[..3].iter().all(Zero::is_zero)));
                    assert!(difference.iter().all(|h| h[3] > Rational::zero()));
                }
            }
        }
    }
}

#[test]
fn cached_original_isocurves_equal_both_incident_patches_across_tensor_knots() {
    let surface = NurbsSurface::try_new_rational(
        2,
        3,
        5,
        6,
        (0..6)
            .flat_map(|j| {
                (0..5).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(i as Real / 4., j as Real / 8., (i * j) as Real / 16.)
                            .unwrap(),
                        [1., 0.5, 2., 0.25][(i + j) % 4],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-3., -3., -3., -1., -1., 5., 5., 5.],
        vec![7., 7., 7., 7., 9., 10., 15., 15., 15., 15.],
    )
    .unwrap();
    let mut preparation = Budget(MAX_WORK);
    let mut reduced = Surface::new(&surface, &mut preparation).unwrap().unwrap();
    let mut full = Surface::new(&surface, &mut preparation).unwrap().unwrap();
    for fixed in 0..2 {
        let stations = if fixed == 0 {
            [-3., -1., 3., 5., -1., -3.]
        } else {
            [7., 9., 11., 15., 9., 7.]
        };
        let variable = 1 - fixed;
        for coordinate in stations.map(rational) {
            for &fixed_span in &full.spans[fixed].clone() {
                if coordinate < full.knots[fixed][fixed_span]
                    || coordinate > full.knots[fixed][fixed_span + 1]
                {
                    continue;
                }
                // Repeated/reversed span visits exercise the retained
                // original controls and the per-span one-dimensional cache.
                let spans = full.spans[variable].clone();
                for &variable_span in spans.iter().chain(spans.iter().rev()) {
                    let mut budget = Budget(MAX_WORK);
                    let mut span = [0; 2];
                    span[fixed] = fixed_span;
                    span[variable] = variable_span;
                    let low = &full.knots[variable][variable_span];
                    let width = &full.knots[variable][variable_span + 1] - low;
                    let uv = [1., 0.75, 0.5, 0.]
                        .into_iter()
                        .zip([1., 2., 0.5, 1.])
                        .map(|(t, w)| {
                            let mut h = [Rational::zero(), Rational::zero(), rational(w)];
                            h[fixed] = &coordinate * &h[2];
                            h[variable] = (low + &width * rational(t)) * &h[2];
                            h
                        })
                        .collect::<Vec<_>>();
                    let image = reduced
                        .isocurve_image(span, &uv, &mut budget)
                        .unwrap()
                        .unwrap();
                    let reference = full.tensor_image(span, &uv, &mut budget).unwrap();
                    let difference = algebra::difference(&image, &reference, &mut budget).unwrap();
                    assert!(difference.iter().all(|h| h[..3].iter().all(Zero::is_zero)));
                    // Reducing the original tensor never extracts or retains
                    // its two-dimensional Bernstein patches.
                    assert!(reduced.patches.is_empty());
                }
            }
        }
    }
}

#[test]
fn shared_endpoints_and_nearly_constant_controls_do_not_collapse_a_tensor() {
    let surface = NurbsSurface::try_bilinear([
        Point3::try_new(0., 0., 0.).unwrap(),
        Point3::try_new(1., 0., 0.).unwrap(),
        Point3::try_new(1., 1., 1.).unwrap(),
        Point3::try_new(0., 1., 0.).unwrap(),
    ])
    .unwrap();
    for fixed in 0..2 {
        for interior in [0.5_f64.next_up(), 0.75] {
            let controls = (0..4)
                .map(|i| {
                    let mut point = [Rational::zero(), Rational::zero(), Rational::one()];
                    point[fixed] = rational(if i == 1 { interior } else { 0.5 });
                    point[1 - fixed] = Rational::new(i.into(), 3.into());
                    point
                })
                .collect::<Vec<_>>();
            let mut budget = Budget(MAX_WORK);
            let mut exact = Surface::new(&surface, &mut budget).unwrap().unwrap();
            assert!(
                exact
                    .isocurve_patch([1, 1], &controls, &mut budget)
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn reduced_interval_bounds_agree_with_independent_full_tensor_proofs() {
    for fixed in 0..2 {
        // S(u,v)=(u/(1+u), v^2, uv/(1+u)), swapping the
        // parameter axes for the other direction. The supplied spatial
        // controls below come directly from this formula, not extraction.
        let variable = 1 - fixed;
        let mut degree = [1; 2];
        degree[fixed] = 2;
        let counts = degree.map(|p| p + 1);
        let surface = NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            counts[0],
            counts[1],
            (0..counts[1])
                .flat_map(|j| {
                    (0..counts[0]).map(move |i| {
                        let index = [i, j];
                        let x = index[variable] as Real / 2.;
                        let v = index[fixed] as Real / 2.;
                        WeightedPoint3::try_new(
                            Point3::try_new(x, if index[fixed] == 2 { 1. } else { 0. }, x * v)
                                .unwrap(),
                            1. + index[variable] as Real,
                        )
                        .unwrap()
                    })
                })
                .collect(),
            [vec![0.; counts[0]], vec![1.; counts[0]]].concat(),
            [vec![0.; counts[1]], vec![1.; counts[1]]].concat(),
        )
        .unwrap();
        let station = rational(0.25);
        let uv = [0., 0.125, 0.75, 1.]
            .into_iter()
            .zip([1., 2., 0.5, 1.])
            .map(|(x, w)| {
                let mut h = [Rational::zero(), Rational::zero(), rational(w)];
                h[variable] = rational(x) * &h[2];
                h[fixed] = &station * &h[2];
                h
            })
            .collect::<Vec<_>>();
        let mut accepted = 0;
        let mut rejected = 0;
        for offset in [0., 1e-7, 9e-7, 1.1e-6, -2e-6] {
            let spatial = uv
                .iter()
                .map(|h| {
                    let weight = &h[2] + &h[variable];
                    [
                        h[variable].clone(),
                        &station * &station * &weight,
                        &station * &h[variable] + rational(offset) * &weight,
                        weight,
                    ]
                })
                .collect::<Vec<_>>();
            let mut budget = Budget(MAX_WORK);
            let mut exact = Surface::new(&surface, &mut budget).unwrap().unwrap();
            let span = degree;
            let full = exact.tensor_image(span, &uv, &mut budget).unwrap();
            let difference = algebra::difference(&full, &spatial, &mut budget).unwrap();
            match exact
                .interval_bound(span, &uv, &spatial, 1e-6, &mut budget)
                .unwrap()
            {
                interval::Outcome::Within(upper) => {
                    assert!(upper <= 1e-6 && upper >= offset.abs());
                    assert!(
                        algebra::hull_bound(difference, upper, &mut budget)
                            .unwrap()
                            .is_some()
                    );
                    accepted += 1;
                }
                interval::Outcome::Outside => {
                    assert!(
                        algebra::hull_bound(difference, 1e-6, &mut budget)
                            .unwrap()
                            .is_none()
                    );
                    rejected += 1;
                }
                interval::Outcome::Inconclusive => {}
            }
        }
        assert_eq!(accepted, 3);
        assert_eq!(rejected, 2);
    }
}
