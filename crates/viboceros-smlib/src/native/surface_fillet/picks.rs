//! Pick-side preparation shared with the rolling-ball producer.
use super::*;
use viboceros_geometry::Plane;

fn ambiguous() -> Error {
    Error::Kernel("Surface fillet picks do not determine unambiguous normal sides".into())
}

pub(super) fn options(
    first: &NurbsSurface,
    second: &NurbsSurface,
    picks: [[f64; 2]; 2],
    radius: f64,
    extend: bool,
    tolerance: Tolerance,
) -> Result<SurfaceFilletOptions, Error> {
    let uv = [
        first.normalized_parameters(picks[0][0], picks[0][1])?,
        second.normalized_parameters(picks[1][0], picks[1][1])?,
    ];
    let (surfaces, _, scale) = prepared_sources(first, second, radius)?;
    let tolerance = Tolerance::try_new(
        tolerance.absolute() / scale,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let points = [
        surfaces[0].evaluate(uv[0][0], uv[0][1])?,
        surfaces[1].evaluate(uv[1][0], uv[1][1])?,
    ];
    let mut sides = [SurfaceFilletSide::Positive; 2];
    for i in 0..2 {
        let surface = &surfaces[i];
        let target = points[1 - i];
        let (u, v) = surface.closest_parameters(target, tolerance)?;
        let point = surface.evaluate(u, v)?;
        let normal = surface.normal_at(u, v)?;
        let signed = Plane::new(point, normal).signed_distance_to(target)?;
        // Closest-point queries and filtered normals have finite uncertainty. Reject
        // nearby picks rather than increasing any construction tolerance.
        if !signed.is_finite() || signed.abs() <= 4. * tolerance.absolute().max(1e-12) {
            return Err(ambiguous());
        }
        sides[i] = if signed > 0. {
            SurfaceFilletSide::Positive
        } else {
            SurfaceFilletSide::Negative
        };
    }
    Ok(SurfaceFilletOptions {
        radius,
        sides,
        extend,
    })
}
