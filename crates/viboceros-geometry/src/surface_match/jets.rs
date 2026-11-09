//! Match tangent length and normal curvature at one correspondence site.
use super::*;
use crate::SurfaceJet2;
use nalgebra::Vector3 as V;
pub(super) type Vec3 = V<Real>;
fn v(x: crate::Vector3) -> Vec3 {
    Vec3::from_row_slice(&x.to_array())
}
fn unit(x: Vec3) -> Result<Vec3, GeometryError> {
    let length = x.norm();
    if !length.is_finite() || length == 0. {
        return Err(error("singular boundary tangent frame"));
    }
    Ok(x / length)
}
pub(super) fn desired(
    source: SurfaceJet2,
    target: SurfaceJet2,
    iso: SurfaceMatchIso,
    continuity: SurfaceBlendContinuity,
    normal_scale: Real,
) -> Result<[Vec3; 2], GeometryError> {
    if continuity == SurfaceBlendContinuity::Position {
        return Ok([Vec3::zeros(); 2]);
    }
    let old = v(source.derivative_u);
    let target_u = v(target.derivative_u);
    let target_v = v(target.derivative_v);
    let normal = unit(target_u.cross(&target_v))?;
    let direction = unit(match iso {
        SurfaceMatchIso::Automatic | SurfaceMatchIso::MatchTarget => target_u,
        SurfaceMatchIso::Perpendicular => {
            let tangent = unit(target_v)?;
            target_u - tangent * target_u.dot(&tangent)
        }
        SurfaceMatchIso::Preserve => old - normal * old.dot(&normal),
    })?;
    let first = direction * old.norm();
    if continuity == SurfaceBlendContinuity::Tangency {
        return Ok([first, Vec3::zeros()]);
    }
    // Resolve the matched physical tangent in the target's U/V frame. The
    // second fundamental form supplies the required normal acceleration.
    let e = target_u.dot(&target_u);
    let f = target_u.dot(&target_v);
    let g = target_v.dot(&target_v);
    let det = e * g - f * f;
    if !det.is_finite() || det <= 0. {
        return Err(error("singular target first fundamental form"));
    }
    let a = (g * target_u.dot(&first) - f * target_v.dot(&first)) / det;
    let b = (e * target_v.dot(&first) - f * target_u.dot(&first)) / det;
    let normal_acceleration = normal.dot(
        &(v(target.derivative_uu) * a * a
            + v(target.derivative_uv) * (2. * a * b)
            + v(target.derivative_vv) * b * b),
    );
    let tangential_acceleration = v(source.derivative_uu).dot(&unit(old)?);
    Ok([
        first,
        direction * tangential_acceleration + normal * (normal_acceleration * normal_scale),
    ])
}

/// The averaged G2 strip uses the source U-curve's curvature after moving only
/// its endpoint to the midpoint. Its other half is the target's normal
/// curvature in the requested iso direction. Keep the mean of the unit
/// directions unnormalized: this also determines the shortened G1 handle.
pub(super) fn average_curvature(
    original: SurfaceJet2,
    reset: SurfaceJet2,
    target: SurfaceJet2,
    matched: [Vec3; 2],
    weights: &[Real],
    coefficients: [Real; 3],
) -> Result<[Vec3; 2], GeometryError> {
    let [d0, d1, dd] = coefficients;
    let [w0, w1, w2] = [weights[0], weights[1], weights[2]];
    let old = v(original.derivative_u);
    let old_unit = unit(old)?;
    let length = old.norm();
    let target_direction = matched[0] / length;
    let direction = (old_unit + target_direction) * 0.5;
    let first = direction * length;
    unit(first)?;
    let gap = v(original.point.vector_to(target.point)?) * 0.5;
    let wdu = d0 * (w0 - w1);
    let wdd = dd * (d0 * w0 - (d0 + d1) * w1 + d1 * w2);
    let moved_first = old + gap * (d0 * w1 / w0);
    let moved_second =
        v(original.derivative_uu) + gap * (dd * d0 - wdd / w0 - 2. * d0 * w1 * wdu / (w0 * w0));
    let moved_unit = unit(moved_first)?;
    let source_curvature =
        (moved_second - moved_unit * moved_second.dot(&moved_unit)) / moved_first.norm_squared();
    let old_tangent_acceleration = v(reset.derivative_uu).dot(&old_unit);
    let target_curvature =
        (matched[1] - target_direction * old_tangent_acceleration) / old.norm_squared();
    // The adjacent/last cross-knot ratio scales both halves of the mean.
    // `desired` has already applied that factor to the target half.
    let curvature = (source_curvature * (d1 / d0) + target_curvature) * 0.5;
    // Reconstructing the second row with the mean direction changes its
    // tangential acceleration by the G1 handle's squared shortening factor.
    // The rational U weight derivative contributes to the same coefficient.
    let tangent_coefficient = 2. * (dd - wdu / w0) * d1 / d0;
    let tangent_acceleration =
        old_tangent_acceleration + tangent_coefficient * length * (direction.norm_squared() - 1.);
    let second = (curvature - direction * curvature.dot(&direction)) * first.norm_squared()
        + direction * tangent_acceleration;
    Ok([first, second])
}
