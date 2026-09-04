//! Green's theorem: integrate a surface density over an oriented trimmed UV
//! region as ∮ (∫[u0,u] density(s,v) ds) dv. Inner loops subtract naturally.
//! Both integrations use exact NURBS evaluations and bounded adaptive rules.

use super::super::{collect_bernstein_roots, floating_parameter_epsilon, scalar_bezier_spans};
use super::{BrepFace, Measure, neumaier_add};
use crate::{
    GeometryError, NurbsCurve2, NurbsSurface, Real, Vector3, integration::integrate_adaptive,
    require_finite, vector::product_three,
};

const MAX_BOUNDARY_INTERVALS: usize = 65_536;
const MAX_SURFACE_EVALUATIONS: usize = 2_000_000;

pub(super) fn integrate(
    face: &BrepFace,
    surface: &NurbsSurface,
    measure: Measure,
    absolute_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let mut intervals = Vec::new();
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        for interval in boundary_intervals(&trim.curve, surface)? {
            if intervals.len() == MAX_BOUNDARY_INTERVALS {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            intervals.push((&trim.curve, interval));
        }
    }
    if intervals.is_empty() {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let outer_tolerance =
        (absolute_tolerance * 0.5 / intervals.len() as Real).max(Real::MIN_POSITIVE);
    let spans_u = surface.spans_u().collect::<Vec<_>>();
    let inner_tolerance = (outer_tolerance * 0.25 / spans_u.len() as Real).max(Real::MIN_POSITIVE);
    let relative_tolerance = (relative_tolerance * 0.125).max(Real::MIN_POSITIVE);
    let mut remaining_evaluations = MAX_SURFACE_EVALUATIONS;
    let mut sum = 0.0;
    let mut correction = 0.0;
    for (curve, interval) in intervals {
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
                        match measure {
                            Measure::Area => Ok(v_scale.signum()
                                * product_three(
                                    normal.length()?,
                                    4.0,
                                    1.0,
                                    "trimmed area integrand",
                                )?),
                            Measure::Volume => {
                                let triple = Vector3::try_new(point.x(), point.y(), point.z())?
                                    .dot(normal)?;
                                let orientation = if face.reversed { -1.0 } else { 1.0 };
                                Ok(orientation
                                    * triple.signum()
                                    * product_three(
                                        triple.abs(),
                                        4.0,
                                        1.0 / 3.0,
                                        "trimmed volume integrand",
                                    )?)
                            }
                        }
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
    if matches!(measure, Measure::Area) && value < 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
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

fn boundary_intervals(
    curve: &NurbsCurve2,
    surface: &NurbsSurface,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    let domain = curve.domain();
    let mut breaks = vec![*domain.start(), *domain.end()];
    breaks.extend(curve.spans().map(|(_, end)| end));
    for (axis, knots) in [
        (0, surface.spans_u().map(|(_, end)| end).collect::<Vec<_>>()),
        (1, surface.spans_v().map(|(_, end)| end).collect::<Vec<_>>()),
    ] {
        // Natural-domain endpoints cannot be crossed by a valid trim.
        for &knot in knots.iter().take(knots.len().saturating_sub(1)) {
            for span in scalar_bezier_spans(curve, axis, knot)? {
                if !span.coefficients.iter().all(|value| *value == 0.0) {
                    collect_bernstein_roots(
                        &span.coefficients,
                        span.parameter,
                        0,
                        true,
                        true,
                        &mut breaks,
                    );
                    if breaks.len() > MAX_BOUNDARY_INTERVALS {
                        return Err(GeometryError::NumericalIntegrationDidNotConverge);
                    }
                }
            }
        }
    }
    breaks.sort_by(Real::total_cmp);
    breaks.dedup();
    Ok(breaks
        .windows(2)
        .filter_map(|pair| (pair[0] < pair[1]).then_some([pair[0], pair[1]]))
        .collect())
}
