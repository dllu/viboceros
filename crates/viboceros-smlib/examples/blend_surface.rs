//! Construction benchmark and control-net agreement for native exact BlendSrf.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{
    NurbsSurface, Point3, SurfaceBlendContinuity as G, SurfaceBlendOptions,
    SurfaceExtensionEdge as Edge, Tolerance, WeightedPoint3, try_blend_nurbs_surfaces,
};

fn patch(count: usize, offset: f64) -> NurbsSurface {
    let mut knots = vec![0.; 3];
    knots.extend((1..count - 2).map(|i| i as f64 / (count - 2) as f64));
    knots.extend([1.; 3]);
    let controls = (0..3)
        .flat_map(|v| {
            let knots = &knots;
            (0..count).map(move |u| {
                let t = (knots[u + 1] + knots[u + 2]) * 0.5;
                WeightedPoint3::try_new(
                    Point3::try_new(
                        3. * t,
                        offset + v as f64 * 0.5,
                        (t * v as f64 + (v * v) as f64) * 0.1,
                    )
                    .unwrap(),
                    1. + 0.1 * t + v as f64 * 0.05,
                )
                .unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        2,
        2,
        count,
        3,
        controls,
        knots,
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}

fn measure(mut operation: impl FnMut() -> NurbsSurface, repetitions: usize) -> u128 {
    black_box(operation());
    let start = Instant::now();
    for _ in 0..repetitions {
        black_box(operation());
    }
    start.elapsed().as_nanos() / repetitions as u128
}

fn main() {
    let mut records = Vec::new();
    for (count, repetitions) in [(3, 1000), (64, 100), (512, 20)] {
        let parents = [patch(count, 0.), patch(count, 3.)];
        for continuity in [
            [G::Position; 2],
            [G::Curvature; 2],
            [G::Curvature, G::Tangency],
        ] {
            let options = SurfaceBlendOptions {
                continuity,
                bulges: [0.3, 1.7],
                reverse_second: Some(false),
            };
            let mut native = || {
                viboceros_smlib::blend_surfaces(
                    &parents[0],
                    Edge::North,
                    &parents[1],
                    Edge::South,
                    options,
                    Tolerance::DEFAULT,
                )
                .unwrap()
                .surface
            };
            let mut portable = || {
                try_blend_nurbs_surfaces(
                    &parents[0],
                    Edge::North,
                    &parents[1],
                    Edge::South,
                    options,
                    Tolerance::DEFAULT,
                )
                .unwrap()
                .surface
            };
            let a = native();
            let b = portable();
            assert_eq!(a.knots_u(), b.knots_u());
            assert_eq!(a.knots_v(), b.knots_v());
            let mut maximum_position_error = 0_f64;
            let mut maximum_weight_error = 0_f64;
            for (a, b) in a.control_points().iter().zip(b.control_points()) {
                maximum_position_error =
                    maximum_position_error.max(a.point().distance_to(b.point()).unwrap());
                maximum_weight_error = maximum_weight_error.max((a.weight() - b.weight()).abs());
            }
            assert!(maximum_position_error < 1e-10 && maximum_weight_error < 1e-12);
            // Alternate timing order to reduce systematic warm-cache bias.
            let (native_ns, portable_ns) = if continuity[1] == G::Tangency {
                let portable_ns = measure(&mut portable, repetitions);
                (measure(&mut native, repetitions), portable_ns)
            } else {
                (
                    measure(&mut native, repetitions),
                    measure(&mut portable, repetitions),
                )
            };
            records.push(serde_json::json!({
                "edge_control_count": count,
                "continuity": continuity.map(|g| g.order()),
                "repetitions": repetitions,
                "native_nanoseconds_per_construction": native_ns,
                "portable_nanoseconds_per_construction": portable_ns,
                "native_to_portable_ratio": native_ns as f64 / portable_ns as f64,
                "maximum_control_position_error": maximum_position_error,
                "maximum_control_weight_error": maximum_weight_error,
            }));
        }
    }
    println!("{}", serde_json::to_string_pretty(&records).unwrap());
}
