//! Bounded whole-call circle fitting timings; inputs are prepared before timing.
use std::time::Instant;
use viboceros_geometry::{Circle3, Point3};
fn main() {
    for count in [100, 10000] {
        let points = (0..count)
            .map(|i| {
                let t = i as f64;
                let radius = 3. + 0.03 * (3. * t).cos();
                Point3::try_new(
                    1. + radius * (0.4 * t).cos(),
                    2. + radius * (0.4 * t).sin(),
                    0.2 * (2.3 * t).sin(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        for _ in 0..10 {
            Circle3::try_fit_to_points(&points).unwrap();
        }
        let mut times = Vec::new();
        let mut fit = None;
        for _ in 0..3 {
            let timer = Instant::now();
            fit = Circle3::try_fit_to_points(&points).unwrap();
            times.push(timer.elapsed().as_secs_f64() * 1000.);
        }
        times.sort_by(f64::total_cmp);
        let fit = fit.unwrap();
        println!(
            "count={count} median_ms={} radius={} center={:?}",
            times[1],
            fit.radius(),
            fit.center().to_array()
        );
    }
}
