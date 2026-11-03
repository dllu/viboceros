use super::*;
use crate::{NurbsCurve, Point3, nurbs::bspline_basis_nonzero};

fn point(i: usize) -> Point3 {
    let t = i as Real;
    Point3::try_new((0.3 * t).sin(), (0.7 * t).cos(), 0.1 * t).unwrap()
}

#[test]
fn banded_greville_recovers_controls_from_independent_de_boor_values() {
    for degree in [1, 2, 3, 5, 7, 17] {
        let mut knots = vec![0.; degree + 1];
        for (k, multiplicity) in [(0.2, 1), (0.6, degree), (0.8, degree + 1)] {
            knots.extend(std::iter::repeat_n(k, multiplicity));
        }
        knots.extend(std::iter::repeat_n(1., degree + 1));
        let n = knots.len() - degree - 1;
        let curve = NurbsCurve::try_new(degree, (0..n).map(point).collect(), knots).unwrap();
        let axis = Axis::sites(degree, curve.knots().to_vec()).unwrap();
        let rows = axis
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if s.fixed {
                    (i, vec![1.])
                } else {
                    bspline_basis_nonzero(curve.knots(), degree, n, s.parameter).unwrap()
                }
            })
            .collect::<Vec<_>>();
        let rhs = Mat::from_fn(n, 3, |i, c| {
            let s = axis.stations[i];
            if s.fixed {
                point(i).to_array()[c]
            } else {
                curve
                    .evaluate_on_side(s.parameter, s.side)
                    .unwrap()
                    .to_array()[c]
            }
        });
        let result = general_banded::solve(degree, &rows, rhs).unwrap();
        for i in 0..n {
            for c in 0..3 {
                assert!(
                    (result[(i, c)] - point(i).to_array()[c]).abs() < 2e-8,
                    "degree={degree}, control={i}"
                );
            }
        }
    }
}

#[test]
fn cyclic_greville_recovers_nonuniform_periodic_controls_without_dense_rows() {
    for (degree, n) in [
        (2, 2),
        (3, 3),
        (7, 7),
        (2, 9),
        (3, 11),
        (5, 17),
        (7, 21),
        (3, 997),
    ] {
        let intervals = (0..n)
            .map(|i| 0.7 + (i % 5) as Real * 0.13)
            .collect::<Vec<_>>();
        let mut cumulative = vec![0.];
        for step in intervals {
            cumulative.push(cumulative.last().unwrap() + step);
        }
        let period = cumulative[n];
        let knots = (0..n + 2 * degree + 1)
            .map(|i| {
                let k = i as isize - degree as isize;
                k.div_euclid(n as isize) as Real * period
                    + cumulative[k.rem_euclid(n as isize) as usize]
            })
            .collect::<Vec<_>>();
        let curve = NurbsCurve::try_new(
            degree,
            (0..n + degree).map(|i| point(i % n)).collect(),
            knots,
        )
        .unwrap();
        let (parameters, shift) =
            crate::nurbs::periodic_greville_parameters(degree, n + degree, n, curve.knots())
                .unwrap();
        let rows = parameters
            .iter()
            .map(|&t| bspline_basis_nonzero(curve.knots(), degree, n + degree, t).unwrap())
            .collect::<Vec<_>>();
        let rhs = Mat::from_fn(n, 3, |i, c| {
            curve.evaluate(parameters[i]).unwrap().to_array()[c]
        });
        let result = cyclic::solve(degree, &rows, shift, rhs).unwrap();
        for i in 0..n {
            for c in 0..3 {
                assert!(
                    (result[(i, c)] - point(i).to_array()[c]).abs() < 1e-9,
                    "degree={degree}, n={n}, control={i}"
                );
            }
        }
    }
}
