//! Recover complete edge parameter correspondence through native tile charts.
use super::*;

pub(super) fn pairs(
    chart: &Chart,
    actual: [Point2; 2],
    native: [Point2; 2],
    reversed: bool,
) -> Vec<[f64; 2]> {
    let [a, b] = actual.map(|p| p.to_array());
    let [na, nb] = native.map(|p| p.to_array());
    let mut stations = vec![0., 1.];
    for axis in 0..2 {
        if a[axis] != b[axis] {
            for i in 1..chart.grid[axis] {
                let t = (i as f64 / chart.grid[axis] as f64 - a[axis]) / (b[axis] - a[axis]);
                if t > 0. && t < 1. {
                    stations.push(t);
                }
            }
        }
    }
    let axis = usize::from(na[0] == nb[0]);
    let mut pairs = stations
        .into_iter()
        .map(|t| {
            let p = Point2::try_new(a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])).unwrap();
            let q = (chart.point(p)[axis] - na[axis]) / (nb[axis] - na[axis]);
            assert!((-1e-12..=1. + 1e-12).contains(&q));
            if reversed {
                [1. - t, 1. - q.clamp(0., 1.)]
            } else {
                [t, q.clamp(0., 1.)]
            }
        })
        .collect::<Vec<_>>();
    pairs.sort_by(|a, b| a[0].total_cmp(&b[0]));
    pairs.dedup();
    assert!(pairs[0][1].abs() < 1e-12);
    assert!((pairs.last().unwrap()[1] - 1.).abs() < 1e-12);
    pairs[0] = [0., 0.];
    *pairs.last_mut().unwrap() = [1., 1.];
    assert!(
        pairs
            .windows(2)
            .all(|w| w[0][0] < w[1][0] && w[0][1] < w[1][1])
    );
    pairs
}
