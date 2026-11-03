//! Exact ordered convex-hull admission for curved boundary certificates.
use super::*;
/// Every endpoint must be an extreme hull vertex, visited in perimeter order.
/// This is equivalent to strict interior-side checks against every edge, with
/// O(n log n) exact-predicate work. Local turns alone admit star polygons.
pub(super) fn strict_cycle(points: &[ExactPoint]) -> Option<Sense> {
    if points.len() < 3 {
        return None;
    }
    let mut order = (0..points.len()).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| points[a].cmp(&points[b]));
    if order.windows(2).any(|w| points[w[0]] == points[w[1]]) {
        return None;
    }
    let zero = rational(0.);
    let half = |indices: &mut dyn Iterator<Item = usize>| {
        let mut hull = Vec::<usize>::new();
        for i in indices {
            while hull.len() >= 2
                && cross(
                    &points[hull[hull.len() - 2]],
                    &points[hull[hull.len() - 1]],
                    &points[i],
                ) <= zero
            {
                hull.pop();
            }
            hull.push(i);
        }
        hull.pop();
        hull
    };
    let mut hull = half(&mut order.iter().copied());
    hull.extend(half(&mut order.iter().rev().copied()));
    if hull.len() != points.len() {
        return None;
    }
    let start = hull.iter().position(|&i| i == 0)?;
    if (0..points.len()).all(|i| hull[(start + i) % points.len()] == i) {
        Some(Sense::Counterclockwise)
    } else if (0..points.len()).all(|i| hull[(start + points.len() - i) % points.len()] == i) {
        Some(Sense::Clockwise)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn convex_hull_cycle_matches_all_pairs_exact_predicate() {
        let reference = |points: &[ExactPoint]| {
            if points.len() < 3 {
                return None;
            }
            let zero = rational(0.);
            let first = cross(&points[0], &points[1], &points[2]);
            if first == zero {
                return None;
            }
            let positive = first > zero;
            (0..points.len())
                .all(|i| {
                    let next = (i + 1) % points.len();
                    (0..points.len()).all(|j| {
                        j == i || j == next || {
                            let value = cross(&points[i], &points[next], &points[j]);
                            if positive { value > zero } else { value < zero }
                        }
                    })
                })
                .then_some(if positive {
                    Sense::Counterclockwise
                } else {
                    Sense::Clockwise
                })
        };
        let hull = [
            [0., 0.],
            [4., 0.],
            [7., 2.],
            [8., 5.],
            [6., 8.],
            [3., 9.],
            [0., 7.],
            [-1., 3.],
        ];
        for step in [1, 3, 5, 7] {
            for phase in 0..8 {
                let points = (0..8)
                    .map(|i| hull[(phase + step * i) % 8].map(rational))
                    .collect::<Vec<_>>();
                assert!(strict_cycle(&points) == reference(&points));
            }
        }
        let mut state = 0x7162459_u64;
        for n in 3..=9 {
            for _ in 0..300 {
                let points = (0..n)
                    .map(|_| {
                        std::array::from_fn(|_| {
                            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                            rational(((state >> 32) % 11) as Real - 5.)
                        })
                    })
                    .collect::<Vec<_>>();
                assert!(strict_cycle(&points) == reference(&points), "{points:?}");
            }
        }
    }
}
