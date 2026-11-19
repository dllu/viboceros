//! Condition the native Boolean for a thin sweep against a warped face.
use super::*;

pub(super) fn retry(
    source: &Brep,
    boundary: &Brep,
    apex: Point3,
    split_at_tangents: bool,
    tolerance: Tolerance,
) -> Result<Option<Brep>, CommandError> {
    let [face] = source.faces() else {
        return Ok(None);
    };
    let Some(plane) = face.surface().plane(tolerance)? else {
        return Ok(None);
    };
    let height = plane.signed_distance_to(apex)?.abs();
    // The native stitcher can search up to forty times the model tolerance.
    if height == 0.
        || height > 40. * tolerance.absolute()
        || boundary.faces()[0].surface().plane(tolerance)?.is_some()
    {
        return Ok(None);
    }

    // Keep the working stitch distance below the sweep height. Modest
    // normal stretching gives the Boolean more separation without stretching
    // lateral trim features or changing any intersections on the cutter.
    for factor in [64., 256., 1.] {
        let absolute = tolerance.absolute().min(factor * height / 128.);
        if absolute == 0. || (factor == 1. && absolute == tolerance.absolute()) {
            continue;
        }
        let working = Tolerance::try_new(absolute, tolerance.relative(), tolerance.angular())?;
        let stretch =
            AffineTransform3::try_directional_scale(plane.origin(), plane.normal(), factor)?;
        let restore = AffineTransform3::try_directional_scale(
            plane.origin(),
            plane.normal(),
            factor.recip(),
        )?;
        let attempt = (|| -> Result<Brep, CommandError> {
            let source = source.transformed(stretch, working)?;
            let boundary = boundary.transformed(stretch, working)?;
            let apex = stretch.transform_point(apex)?;
            let sweep = cone(&source, apex, split_at_tangents, working)?;
            let cut = truncate(&sweep, &boundary, apex, working)?;
            Ok(cut.transformed(restore, tolerance)?)
        })();
        if let Ok(cut) = attempt {
            return Ok(Some(cut));
        }
    }
    Ok(None)
}
