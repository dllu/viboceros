//! Local kernel diagnostic, not a viewport benchmark or Rhino speed comparison.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};

fn main() {
    println!("round,domain,method,queries,nanoseconds_per_query");
    let cases = [
        ("unit", 0., 1., 20_000),
        ("shifted", 1e12, 1e12 + 1., 20_000),
        ("tiny", 0., 1e-170, 20_000),
        ("huge", 0., 1e170, 20_000),
        ("subnormal", 0., f64::from_bits(1), 50),
    ];
    for round in 0..5 {
        for offset in 0..cases.len() {
            let (label, start, end, repeats) = cases[(offset + round) % cases.len()];
            let curve = NurbsCurve::try_new_rational(
                3,
                [(0., 0., 1.), (1., 2., 2.), (4., -1., 0.75), (8., 3., 1.)]
                    .into_iter()
                    .map(|(x, y, w)| {
                        WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), w).unwrap()
                    })
                    .collect(),
                vec![start, start, start, start, end, end, end, end],
            )
            .unwrap();
            let sampler = curve.parameter_sampler().unwrap();
            let span = sampler.spans().next().unwrap();
            for index in 0..if label == "unit" { 2 } else { 1 } {
                let native = label == "unit" && (index + round) % 2 == 0;
                let started = Instant::now();
                for _ in 0..repeats {
                    for f in [0.137, 0.371, 0.613, 0.829] {
                        black_box(if native {
                            black_box(&curve)
                                .evaluate_with_derivative(black_box(f))
                                .unwrap()
                        } else {
                            black_box(span)
                                .evaluate_with_derivative(black_box(f))
                                .unwrap()
                        });
                    }
                }
                let elapsed = started.elapsed();
                println!(
                    "{round},{label},{},{},{:.3}",
                    if native { "native" } else { "fractional" },
                    repeats * 4,
                    elapsed.as_nanos() as f64 / (repeats * 4) as f64
                );
            }
        }
    }
}
