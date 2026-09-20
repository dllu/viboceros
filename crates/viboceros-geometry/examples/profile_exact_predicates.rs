//! Bounded primitive timings; no performance thresholds or special kernel hooks.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{Point3, Vector3};

fn measure<T>(name: &str, mut query: impl FnMut() -> T) {
    const ITERATIONS: u32 = 20_000;
    drop(black_box(query()));
    print!("{name}:");
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            drop(black_box(query()));
        }
        let elapsed = start.elapsed().as_nanos() as f64 / f64::from(ITERATIONS);
        print!(" {elapsed:.2}");
    }
    println!(" ns/query");
}

fn main() {
    let p = |v| Point3::try_from(v).unwrap();
    let q = 0.5_f64.sqrt();
    for (name, target, a, b) in [
        (
            "distance-near-tie",
            p([2_f64.sqrt(), 2_f64.sqrt(), 0.]),
            p([q, q, 0.]),
            p([q.next_up(), q.next_down(), 0.]),
        ),
        (
            "distance-wide",
            p([f64::MAX; 3]),
            p([f64::from_bits(1), 1., 2.]),
            p([0., 2., 1.]),
        ),
        (
            "distance-identical",
            p([f64::MAX, 2., 3.]),
            p([1., 2., 3.]),
            p([1., 2., 3.]),
        ),
    ] {
        measure(name, || {
            black_box(target).compare_distances(black_box(a), black_box(b))
        });
    }
    let v = |v| Vector3::try_from(v).unwrap();
    let large = 2_f64.powi(512);
    for (name, a, b) in [
        ("dot-ordinary", v([1., 2., 3.]), v([4., 5., 6.])),
        (
            "dot-overflow-cancellation",
            v([large, large, 0.]),
            v([large, -large.next_down(), 0.]),
        ),
        ("dot-subnormal", v([f64::from_bits(1); 3]), v([0.5; 3])),
        (
            "dot-wide-cancellation",
            v([f64::MAX, f64::MAX, f64::from_bits(1)]),
            v([1., -1., 1.]),
        ),
    ] {
        measure(name, || black_box(a).dot(black_box(b)).unwrap());
    }
}
