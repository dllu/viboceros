use super::*;
use crate::exact_scalar::{Rational, rational};
use num_traits::{One, Zero};

// An independent full 4x4 determinant, with no translated 3x3 formula or
// interval arithmetic shared with the production circle predicate.
fn reference(points: [Point3; 4], [x, y]: [usize; 2]) -> Ordering {
    let matrix = points.map(|p| {
        let p = p.to_array().map(rational);
        [
            p[x].clone(),
            p[y].clone(),
            p.iter().map(|x| x * x).sum(),
            Rational::one(),
        ]
    });
    let mut result = Rational::zero();
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let p = [a, b, c, d];
                    if p.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
                        continue;
                    }
                    let inversions = (0..4)
                        .map(|i| (i + 1..4).filter(|&j| p[i] > p[j]).count())
                        .sum::<usize>();
                    let value = (0..4).map(|i| &matrix[i][p[i]]).product::<Rational>();
                    if inversions.is_multiple_of(2) {
                        result += value;
                    } else {
                        result -= value;
                    }
                }
            }
        }
    }
    result.cmp(&Rational::zero())
}

#[test]
fn interval_filtered_world_circles_match_independent_exact_determinants() {
    let mut seed = 0x1d72_5a83_631e_902fu64;
    let mut integer = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 32) % 65) as f64 - 32.
    };
    for exponent in [-700, -80, 0, 80, 700] {
        let scale = 2_f64.powi(exponent);
        for _ in 0..32 {
            let points = std::array::from_fn(|_| {
                let (x, y) = (integer(), integer());
                Point3::try_new(
                    (x + 128.) * scale,
                    (y - 64.) * scale,
                    (2. * x - 3. * y + 32.) * scale,
                )
                .unwrap()
            });
            for axes in [[0, 1], [1, 2], [2, 0]] {
                assert_eq!(predicates::circle(points, axes), reference(points, axes));
            }
        }
    }
}

#[test]
fn world_metric_flips_an_oblique_rhombus_without_changing_its_boundary() {
    for exponent in [-700, -20, 0, 20, 700] {
        let scale = 2_f64.powi(exponent);
        let points = [[0., 0., 0.], [1., 0., 1.], [1., 1., 2.], [0., 1., 1.]]
            .map(|p| Point3::try_from(p.map(|x| (x + 4.) * scale)).unwrap());
        for axes in [[0, 1], [1, 2], [2, 0]] {
            let mut faces = [[0, 1, 2], [0, 2, 3]];
            let original_edges = faces.iter().flat_map(|&t| edges(t)).collect::<Vec<_>>();
            let boundary = original_edges
                .iter()
                .filter(|&&e| original_edges.iter().filter(|&&f| f == e).count() == 1)
                .copied()
                .collect::<BTreeSet<_>>();
            let mut work = 100;
            legalize(&points, axes, &mut faces, &mut work).unwrap();
            let mut expected = vec![vec![0, 1, 3], vec![1, 2, 3]];
            let mut actual = faces
                .map(|t| {
                    let mut t = t.to_vec();
                    t.sort();
                    t
                })
                .to_vec();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected);
            let result_edges = faces.iter().flat_map(|&t| edges(t)).collect::<Vec<_>>();
            assert_eq!(
                boundary,
                result_edges
                    .iter()
                    .filter(|&&e| result_edges.iter().filter(|&&f| f == e).count() == 1)
                    .copied()
                    .collect()
            );
        }
    }
}

#[test]
fn exact_circle_ties_preserve_the_existing_layout() {
    let points = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
        .map(|p| Point3::try_from(p).unwrap());
    assert_eq!(predicates::circle(points, [0, 1]), Ordering::Equal);
    for mut faces in [[[0, 1, 2], [0, 2, 3]], [[0, 1, 3], [1, 2, 3]]] {
        let original = faces;
        let mut work = 100;
        legalize(&points, [0, 1], &mut faces, &mut work).unwrap();
        assert_eq!(faces, original);
    }
}

#[test]
fn invalid_indices_incidence_and_exhausted_work_budgets_are_rejected() {
    let points = [[0., 0., 0.], [1., 0., 1.], [1., 1., 2.], [0., 1., 1.]]
        .map(|p| Point3::try_from(p).unwrap());
    for mut faces in [vec![[0, 1, 4]], vec![[0, 1, 2], [0, 1, 2], [0, 2, 3]]] {
        assert!(legalize(&points, [0, 1], &mut faces, &mut 100).is_err());
    }
    assert!(matches!(
        legalize(&points, [0, 1], &mut [[0, 1, 2], [0, 2, 3]], &mut 0),
        Err(GeometryError::SubDCapResourceLimit { .. })
    ));
}
