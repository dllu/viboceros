//! Separable natural/periodic fits and shared pole/seam control-net cleanup.
#[cfg(test)]
mod tests;
use super::*;
use crate::{NurbsCurve, NurbsSurface, UnitVector3, nurbs::clamped_uniform_knots};

impl SubD {
    /// Revolve a fitted SubD-friendly profile with equal-angle limit stations.
    /// Closed turns and closed profiles are periodic; open sides have natural
    /// end conditions. Pole rows retain their adjacent handles. Segment counts
    /// below four use four angular faces, following the native getter.
    pub fn try_revolved_curve(
        curve: &NurbsCurve,
        origin: Point3,
        axis: UnitVector3,
        start: f64,
        sweep: f64,
        segments: usize,
    ) -> Result<Self, GeometryError> {
        crate::require_finite([start], "SubD revolution start angle")?;
        if !sweep.is_finite() || sweep == 0. || sweep.abs() > std::f64::consts::TAU {
            return Err(GeometryError::InvalidRevolutionSweep);
        }
        let segments = segments.max(4);
        if segments > 100_000 {
            return Err(GeometryError::SubDInterpolationFailed);
        }
        let closed = sweep.abs() == std::f64::consts::TAU;
        let chart_start = if sweep < 0. { start + sweep } else { start };
        let samples = (0..segments + usize::from(!closed))
            .map(|i| {
                let angle = sweep.abs() * i as f64 / segments as f64;
                let (sin, cos) = angle.sin_cos();
                Point3::try_new(cos, sin, 0.)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let angular = crate::nurbs::interpolate_natural_samples(&samples, closed)?;
        let profile = curve.try_subd_friendly(curve.is_closed()?)?;
        let direction = axis.as_vector();
        let center = |p: Point3| -> Result<Point3, GeometryError> {
            origin.translated(direction.scaled(origin.vector_to(p)?.dot(direction)?)?)
        };
        let on_axis = |p: Point3| -> Result<bool, GeometryError> {
            // Projection onto an oblique unit axis can leave a few ulps of
            // artificial radius. Use a local arithmetic bound, not model units.
            Ok(p.distance_to(center(p)?)? <= 32. * f64::EPSILON * origin.vector_to(p)?.length()?)
        };
        let angular_points = net_points(&angular, false, false);
        let start_pole = on_axis(profile.control_points()[0].point())?;
        let end_pole = on_axis(profile.control_points().last().unwrap().point())?;
        let profile_points = net_points(&profile, start_pole, end_pole);
        let nu = angular_points.len();
        let nv = profile_points.len();
        let poles = if profile.is_periodic() {
            0
        } else {
            usize::from(start_pole) + usize::from(end_pole)
        };
        let unique_u = nu - usize::from(closed);
        let unique_v = nv - usize::from(profile.is_periodic()) - poles;
        if unique_u
            .checked_mul(unique_v)
            .and_then(|n| n.checked_add(poles))
            .is_none_or(|n| n > 100_000)
            || (nu - 1).checked_mul(nv - 1).is_none_or(|n| n > 100_000)
        {
            return Err(GeometryError::SubDInterpolationFailed);
        }
        let zero = Point3::try_new(0., 0., 0.)?;
        let rotation = AffineTransform3::try_rotation(
            zero,
            axis,
            chart_start.rem_euclid(std::f64::consts::TAU),
        )?;
        let mut controls = Vec::with_capacity(nu * nv);
        for profile in profile_points {
            let center = center(profile)?;
            let radial = if on_axis(profile)? {
                Vector3::try_new(0., 0., 0.)?
            } else {
                rotation.transform_vector(center.vector_to(profile)?)?
            };
            let tangent = direction.cross(radial)?;
            for angular in &angular_points {
                controls.push(
                    center
                        .translated(radial.scaled(angular.x())?)?
                        .translated(tangent.scaled(angular.y())?)?,
                );
            }
        }
        let scaffold = NurbsSurface::try_new(
            1,
            1,
            nu,
            nv,
            controls,
            clamped_uniform_knots(1, nu)?,
            clamped_uniform_knots(1, nv)?,
        )?;
        surface_conversion::convert_revolution_net(&scaffold)
    }
}

fn net_points(curve: &NurbsCurve, start_pole: bool, end_pole: bool) -> Vec<Point3> {
    let controls = curve.control_points();
    if curve.is_periodic() {
        let mut points = controls[..controls.len() - 3]
            .iter()
            .map(|p| p.point())
            .collect::<Vec<_>>();
        points.push(points[0]);
        return points;
    }
    controls
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            ((start_pole || i != 1) && (end_pole || i != controls.len() - 2)).then_some(p.point())
        })
        .collect()
}
