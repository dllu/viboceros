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
