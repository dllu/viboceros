//! Bounded radius target search, inferred from independent native observations.
use super::join::Frame;
use crate::{GeometryError, Point3, Real, Vector3};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcBlendRadiusSolution {
    pub ratio: Real,
    pub radius_difference: Real,
    pub reached_target: bool,
}
const ZERO: Real = 1. / 4_294_967_296.;

/// Search chord fractions in [1/101,100/101]. Each half interval is chosen by
/// comparing its two endpoint errors; keep the best sampled fraction. This
/// reproduces native failed-target previews as well as attained targets.
/// It is deliberately not a global root finder: a nonmonotone radius function
/// can have a closer solution outside the retained interval.
/// Directions are normalized and trials reuse the analytic tangent equation;
/// no curves or rational conversions are allocated during the search.
pub fn try_arc_blend_radius_ratio(
    start: Point3,
    start_direction: Vector3,
    end: Point3,
    end_direction: Vector3,
    difference: Real,
    alternate: bool,
) -> Result<ArcBlendRadiusSolution, GeometryError> {
    crate::require_finite([difference], "arc blend radius difference")?;
    let frame = Frame::new(start, start_direction, end, end_direction)?;
    let (mut left, mut right) = (1. / 101., 100. / 101.);
    let (mut lv, mut rv) = (
        frame.radius_difference(left, alternate)?,
        frame.radius_difference(right, alternate)?,
    );
    let mut best = if (lv - difference).abs() < (rv - difference).abs() {
        (left, lv)
    } else {
        (right, rv)
    };
    for _ in 0..64 {
        let middle = 0.5 * (left + right);
        if middle == left || middle == right {
            break;
        }
        let mv = frame.radius_difference(middle, alternate)?;
        if (mv - difference).abs() < (best.1 - difference).abs() {
            best = (middle, mv);
        }
        if (mv - difference).abs() <= ZERO {
            best = (middle, mv);
            break;
        }
        if (lv - difference).abs() < (rv - difference).abs() {
            right = middle;
            rv = mv;
        } else {
            left = middle;
            lv = mv;
        }
    }
    Ok(ArcBlendRadiusSolution {
        ratio: best.0 / (1. - best.0),
        radius_difference: best.1,
        reached_target: (best.1 - difference).abs() <= ZERO,
    })
}
