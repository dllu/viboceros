//! Green's theorem: integrate a surface density over an oriented trimmed UV
//! region as ∮ (∫[u0,u] density(s,v) ds) dv. Inner loops subtract naturally.
//! Both integrations use exact NURBS evaluations and bounded adaptive rules.

use super::super::floating_parameter_epsilon;
use super::{BrepFace, Measure, boundary, neumaier_add};
use crate::{
    GeometryError, NurbsSurface, Real, Vector3, integration::integrate_adaptive, require_finite,
    vector::product_three,
};

pub(super) fn integrate(
    face: &BrepFace,
    surface: &NurbsSurface,
    measure: Measure,
    absolute_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let value = integrate_density(
        face,
        surface,
        absolute_tolerance,
        relative_tolerance,
        |point, normal, sign| match measure {
            Measure::Area => {
                Ok(sign * product_three(normal.length()?, 4.0, 1.0, "trimmed area integrand")?)
            }
            Measure::Volume => {
                let triple = Vector3::try_new(point.x(), point.y(), point.z())?.dot(normal)?;
                let orientation = if face.reversed { -1.0 } else { 1.0 };
                Ok(orientation
                    * triple.signum()
                    * product_three(triple.abs(), 4.0, 1.0 / 3.0, "trimmed volume integrand")?)
            }
        },
    )?;
    if matches!(measure, Measure::Area) && value < 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    Ok(value)
}

/// Shared Green-theorem boundary traversal. The normal includes oriented
/// boundary/parameter scaling; `sign` is the UV boundary direction.
pub(super) fn integrate_density(
    face: &BrepFace,
    surface: &NurbsSurface,
    absolute_tolerance: Real,
    relative_tolerance: Real,
    mut density: impl FnMut(crate::Point3, Vector3, Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let curves = boundary::prepare(face, surface)?;
    let interval_count = curves.iter().map(|c| c.intervals.len()).sum::<usize>();
    let outer_tolerance =
        (absolute_tolerance * 0.5 / interval_count as Real).max(Real::MIN_POSITIVE);
    let spans_u = surface.spans_u().collect::<Vec<_>>();
    let inner_tolerance = (outer_tolerance * 0.25 / spans_u.len() as Real).max(Real::MIN_POSITIVE);
    let relative_tolerance = (relative_tolerance * 0.125).max(Real::MIN_POSITIVE);
    let mut remaining_evaluations = boundary::MAX_SURFACE_EVALUATIONS;
    let mut sum = 0.0;
    let mut correction = 0.0;
    for (curve, interval) in curves
        .iter()
        .flat_map(|c| c.intervals.iter().map(move |&i| (c.curve.as_ref(), i)))
    {
        let half_t = interval[1] * 0.5 - interval[0] * 0.5;
        let value = integrate_adaptive(0.0, 1.0, outer_tolerance, relative_tolerance, |t| {
            let parameter = interval[0].mul_add(1.0 - t, interval[1] * t);
            let (uv, derivative) = curve.evaluate_with_derivative(parameter)?;
            // An edge at constant V contributes zero to this boundary form.
            if derivative[1] == 0.0 {
                return Ok(0.0);
            }
            let v_scale = derivative[1].signum()
                * product_three(
                    derivative[1].abs(),
                    half_t,
                    1.0,
                    "trimmed integral boundary derivative",
                )?;
            let u = clamp_roundoff(uv.x(), surface.domain_u())?;
            let v = clamp_roundoff(uv.y(), surface.domain_v())?;
            let mut inner_sum = 0.0;
            let mut inner_correction = 0.0;
            for &(start, end) in &spans_u {
                let end = end.min(u);
                if end <= start {
                    continue;
                }
                let half_u = end * 0.5 - start * 0.5;
                let value =
                    integrate_adaptive(0.0, 1.0, inner_tolerance, relative_tolerance, |s| {
                        remaining_evaluations = remaining_evaluations
                            .checked_sub(1)
                            .ok_or(GeometryError::NumericalIntegrationDidNotConverge)?;
                        let parameter_u = start.mul_add(1.0 - s, end * s);
                        let (point, du, dv) = surface.evaluate_with_derivatives(parameter_u, v)?;
                        // Scale derivatives before the cross product: equivalent
                        // very small/large UV domains must not overflow it.
                        let normal = du.scaled(half_u)?.cross(dv.scaled(v_scale)?)?;
                        density(point, normal, v_scale.signum())
                    })?;
                neumaier_add(&mut inner_sum, &mut inner_correction, value);
            }
            let value = inner_sum + inner_correction;
            require_finite([value], "trimmed integral primitive")?;
            Ok(value)
        })?;
        neumaier_add(&mut sum, &mut correction, value);
    }
    let value = sum + correction;
    require_finite([value], "trimmed face integral")?;
    Ok(value)
}

fn clamp_roundoff(
    value: Real,
    domain: std::ops::RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    let epsilon = floating_parameter_epsilon([*domain.start(), *domain.end()]);
    if value < *domain.start() - epsilon || value > *domain.end() + epsilon {
        return Err(GeometryError::InvalidBrepTopology {
            context: "mass property trim leaves its surface domain",
        });
    }
    Ok(value.clamp(*domain.start(), *domain.end()))
}
