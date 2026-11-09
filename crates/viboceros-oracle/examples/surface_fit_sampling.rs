//! Repeatable interpolation timing; no source-accuracy or Rhino speed claim.
use serde_json::json;
use std::{hint::black_box, time::Instant};
use viboceros_geometry::*;

fn knots(p: usize, n: usize) -> Vec<f64> {
    [
        vec![0.; p + 1],
        (1..n - p).map(|i| i as f64 / (n - p) as f64).collect(),
        vec![1.; p + 1],
    ]
    .concat()
}
fn fingerprint(s: &NurbsSurface) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for p in s.control_points() {
        for x in p.point().to_array().into_iter().chain([p.weight()]) {
            for byte in x.to_bits().to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
    }
    format!("{hash:016x}")
}
fn main() {
    let iterations = std::env::var("VIBOCEROS_FIT_BENCH_ITERATIONS")
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(5);
    assert!((1..=1000).contains(&iterations));
    let source = NurbsSurface::try_new_rational(
        8,
        8,
        12,
        12,
        (0..12)
            .flat_map(|j| {
                (0..12).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            i as f64 / 11.,
                            j as f64 / 11.,
                            (i * i + i * j + j * j) as f64 / 121.,
                        )
                        .unwrap(),
                        0.75 + ((i + 2 * j) % 4) as f64 / 4.,
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots(8, 12),
        knots(8, 12),
    )
    .unwrap();
    for count in [16, 32, 64] {
        let k = knots(3, count);
        let candidate = try_interpolate_surface_in_basis(&source, [3, 3], [&k, &k]).unwrap();
        let expected = fingerprint(&candidate);
        let mut times = Vec::new();
        for _ in 0..iterations {
            let start = Instant::now();
            let result =
                try_interpolate_surface_in_basis(black_box(&source), [3, 3], [&k, &k]).unwrap();
            times.push(start.elapsed().as_secs_f64());
            assert_eq!(fingerprint(&result), expected);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "FIT_SAMPLING {}",
            json!({"controls_per_axis":count,"iterations":iterations,
            "median_seconds":times[times.len()/2],"times_seconds":times,"fingerprint":expected,
            "source_degree":[8,8],"source_controls":[12,12],"output_degree":[3,3],
            "source_accuracy_certified":false})
        );
    }
}
