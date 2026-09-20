//! Prepared projection cost, excluding setup. Run in release mode.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{Point3, PointProjection3, Vector3};

fn main() {
    let p = |a| Point3::try_from(a).unwrap();
    let v = |a| Vector3::try_from(a).unwrap();
    for (name, projector, target) in [
        (
            "line-axis",
            PointProjection3::onto_line(p([1., 2., 3.]), p([1., 2., 7.])).unwrap(),
            p([4., 5., 6.]),
        ),
        (
            "line-oblique",
            PointProjection3::onto_line(p([1., 2., 3.]), p([5., 7., 11.])).unwrap(),
            p([4.1, 5.2, 6.3]),
        ),
        (
            "plane-oblique",
            PointProjection3::onto_plane(p([1., 2., 3.]), v([2., -32., 19.])).unwrap(),
            p([4.1, 5.2, 6.3]),
        ),
        (
            "plane-cancellation",
            PointProjection3::onto_plane(p([1., 0., 0.]), v([1., 1., 0.])).unwrap(),
            p([2_f64.powi(1023), 2_f64.powi(1023), 1.]),
        ),
    ] {
        print!("{name}:");
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..10_000 {
                black_box(projector.project(black_box(target)).unwrap());
            }
            print!(" {:.2}", start.elapsed().as_nanos() as f64 / 10_000.);
        }
        println!(" ns/query");
    }
}
