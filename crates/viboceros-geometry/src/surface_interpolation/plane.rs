//! Infinite-plane support is private and measured in orthonormal chart units.
use super::*;
use crate::Frame3;
#[cfg(test)]
mod tests;

/// Orthogonally project locations onto an infinite plane and interpolate there.
/// A finite private chart encloses the inputs; the shared image producer extends
/// it exactly if the interpolant overshoots. No finite boundary clips the curve.
pub fn try_interpolate_curve_on_plane(
    plane: Plane,
    points: &[Point3],
    closed: bool,
    tolerance: Tolerance,
) -> Result<SurfaceInterpolatedCurve, GeometryError> {
    if !(2..=MAX_CURVE_INTERPOLATION_POINTS).contains(&points.len()) {
        return Err(invalid("requires two through 256 interpolation locations"));
    }
    let frame = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let uv = points
        .iter()
        .map(|p| {
            let [u, v] = frame.projected_coordinates_of(*p)?;
            Point2::try_new(u, v)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for p in &uv {
        for (axis, value) in [p.x(), p.y()].into_iter().enumerate() {
            bounds[axis][0] = bounds[axis][0].min(value);
            bounds[axis][1] = bounds[axis][1].max(value);
        }
    }
    for range in &mut bounds {
        let padding = 1_f64
            .max(tolerance.absolute() * 16.)
            .max(range[0].abs().max(range[1].abs()) * Real::EPSILON * 16.);
        range[0] -= padding;
        range[1] += padding;
    }
    let [[u0, u1], [v0, v1]] = bounds;
    let controls = [[u0, v0, 0.], [u1, v0, 0.], [u0, v1, 0.], [u1, v1, 0.]]
        .map(|p| frame.point_at(p))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        controls,
        vec![u0, u0, u1, u1],
        vec![v0, v0, v1, v1],
    )?;
    let mut result = try_interpolate_curve_on_surface(&surface, &uv, closed, tolerance)?;
    let direction = plane.normal().as_vector();
    let positive = result.curve.control_points()[0].weight() > 0.;
    for control in result.curve.control_points() {
        if (control.weight() > 0.) != positive
            || !plane.directional_projection_within_tolerance(
                control.point(),
                control.point(),
                direction,
                tolerance.absolute() * 0.5,
            )?
        {
            return Err(invalid("infinite-plane curve could not be certified"));
        }
    }
    // The control hull certifies the entire plane image. Independently check
    // each interpolation station against the requested orthogonal projection,
    // rather than only against the rounded private support surface.
    let mut parameter = *result.curve.domain().start();
    for i in 0..points.len() {
        if i > 0 {
            parameter += (uv[i].x() - uv[i - 1].x()).hypot(uv[i].y() - uv[i - 1].y());
        }
        let parameter =
            parameter.clamp(*result.curve.domain().start(), *result.curve.domain().end());
        if !plane.directional_projection_within_tolerance(
            points[i],
            result.curve.evaluate(parameter)?,
            direction,
            tolerance.absolute(),
        )? {
            return Err(invalid(
                "infinite-plane interpolation exceeded model tolerance",
            ));
        }
    }
    result.surface_image_bound = result.surface_image_bound.max(tolerance.absolute() * 0.5);
    Ok(result)
}
