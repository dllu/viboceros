//! Local old/new ellipse query comparison; not a Rhino benchmark.
use std::{hint::black_box, time::Instant};
use viboceros_geometry::{Ellipse3, Point3, Tolerance, Vector3};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ellipse = Ellipse3::try_new(
        Point3::try_new(1., 2., 3.)?,
        5.,
        2.,
        Vector3::try_new(1., 0., 0.)?.normalized_nonzero()?,
        Vector3::try_new(0., 1., 0.)?.normalized_nonzero()?,
        Tolerance::DEFAULT,
    )?;
    let target = Point3::try_new(-10., 5., 3.)?;
    for old in [true, false] {
        let query = || {
            let t = if old {
                ellipse
                    .to_nurbs()?
                    .closest_parameter(black_box(target), Tolerance::DEFAULT)?
            } else {
                ellipse.closest_parameter(black_box(target))?
            };
            ellipse.evaluate(t)
        };
        for _ in 0..20 {
            black_box(query()?);
        }
        let mut batches = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..500 {
                black_box(query()?);
            }
            batches.push(start.elapsed().as_secs_f64() * 1e6 / 500.);
        }
        batches.sort_by(f64::total_cmp);
        println!(
            "{}: median {:.3} us/query; point {:?}",
            if old {
                "NURBS dispatch"
            } else {
                "quadrant root"
            },
            batches[3],
            query()?.to_array()
        );
    }
    Ok(())
}
