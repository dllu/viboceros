//! Bounded whole-fit timings; no performance assertions or special kernel hooks.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{Point3, PointProjection3};

fn main() {
    for count in [100, 10_000] {
        for planar in [true, false] {
            let mut state = 53_u64;
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 32) as f64 / 4_294_967_296.
            };
            let points = (0..count)
                .map(|_| {
                    let (x, y, z) = (next(), next(), next());
                    // Exact axis-plane inputs exercise the complete affine-rank check.
                    Point3::try_new(
                        x,
                        y,
                        if planar {
                            2.
                        } else {
                            2. + 0.125 * x + 0.25 * y + 0.01 * z
                        },
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            print!("count={count} planar={planar}:");
            for _ in 0..3 {
                let start = Instant::now();
                black_box(PointProjection3::onto_best_fit_plane(black_box(&points)).unwrap());
                print!(" {:.3}", start.elapsed().as_secs_f64() * 1000.);
            }
            println!(" ms/fit");
        }
    }
}
