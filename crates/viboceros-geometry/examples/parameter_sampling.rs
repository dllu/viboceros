//! Isolated old/new viewport sampling loop timings, including frame setup.
//! Run: cargo run --release -p viboceros-geometry --example parameter_sampling
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{NurbsCurve, Point3, Real};

fn legacy(curve: &NurbsCurve) {
    let domain_end = *curve.domain().end();
    for (start, end) in curve.spans() {
        for i in 0..=16 {
            let f = i as Real / 16.;
            let mut t = start.mul_add(1. - f, end * f);
            if i == 16 && end < domain_end {
                t = end.next_down().max(start);
            }
            black_box(curve.evaluate(t).unwrap());
        }
    }
}

fn framed(curve: &NurbsCurve) {
    let sampler = curve.parameter_sampler().unwrap();
    for span in sampler.spans() {
        for i in 0..=16 {
            black_box(span.evaluate(i as Real / 16.).unwrap());
        }
    }
}

fn time(curve: &NurbsCurve, sample: fn(&NurbsCurve), count: usize) -> f64 {
    let started = Instant::now();
    for _ in 0..count {
        sample(black_box(curve));
    }
    started.elapsed().as_nanos() as f64 / count as f64
}

fn main() {
    let count = 20_000;
    println!("case,round,iterations,legacy_ns_per_curve,framed_ns_per_curve");
    for controls in [4, 19] {
        let curve = NurbsCurve::try_clamped_uniform(
            3,
            (0..controls)
                .map(|i| Point3::try_new(i as f64, (i as f64).sin(), 0.).unwrap())
                .collect(),
        )
        .unwrap();
        for origin in [0., 1e12, -1e12] {
            let curve = NurbsCurve::try_new_rational(
                3,
                curve.control_points().to_vec(),
                curve.knots().iter().map(|k| k + origin).collect(),
            )
            .unwrap();
            for _ in 0..100 {
                legacy(&curve);
                framed(&curve);
            }
            for round in 0..5 {
                let (old, new) = if round % 2 == 0 {
                    (time(&curve, legacy, count), time(&curve, framed, count))
                } else {
                    let new = time(&curve, framed, count);
                    (time(&curve, legacy, count), new)
                };
                println!("controls-{controls}-origin-{origin},{round},{count},{old},{new}");
            }
        }
    }
}
