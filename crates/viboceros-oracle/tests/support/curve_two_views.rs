//! Whole-span sampling with fast matching proposals. Every measured distance is
//! to an actual evaluated target point, hence an upper bound on the minimum.
//! Above-gate values are inconclusive unless an independent witness proves a gap.
use rstar::{RTree, primitives::GeomWithData};
use viboceros_geometry::{NurbsCurve, Tolerance};
pub fn directed_upper_bound(a: &[NurbsCurve], b: &[NurbsCurve]) -> f64 {
    let indexes = b
        .iter()
        .map(|c| {
            let points = c
                .spans()
                .flat_map(|(lo, hi)| (0..=8).map(move |i| lo + (hi - lo) * i as f64 / 8.))
                .map(|t| GeomWithData::new(c.evaluate(t).unwrap().to_array(), t))
                .collect::<Vec<_>>();
            RTree::bulk_load(points)
        })
        .collect::<Vec<_>>();
    let mut maximum = 0_f64;
    for c in a {
        for (lo, hi) in c.spans() {
            for i in 0..=8 {
                let p = c.evaluate(lo + (hi - lo) * i as f64 / 8.).unwrap();
                let mut best = f64::INFINITY;
                for (target, index) in b.iter().zip(&indexes) {
                    for seed in index.nearest_neighbor_iter(p.to_array()).take(4) {
                        for t in [
                            Ok(seed.data),
                            target.point_getter_parameter_from_seed(
                                p,
                                seed.data,
                                Tolerance::DEFAULT,
                            ),
                        ] {
                            if let Ok(point) = t.and_then(|t| target.evaluate(t)) {
                                best = best.min(p.distance_to(point).unwrap());
                            }
                        }
                    }
                }
                maximum = maximum.max(best);
            }
        }
    }
    maximum
}
