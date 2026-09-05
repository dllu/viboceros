use super::interpolation::{Break, control_count, seed_breaks, stable_lerp};
use super::*;
use crate::ParameterSide;
use std::collections::HashMap;

mod tensor;
mod validate;

#[cfg(test)]
mod tests;

pub(super) fn fit(
    morph: &(impl PointMorph + ?Sized),
    source: &NurbsSurface,
    tolerance: Tolerance,
    maximum: usize,
    maximum_samples: usize,
) -> Result<NurbsSurface, GeometryError> {
    if source.control_point_count_u() > maximum || source.control_point_count_v() > maximum {
        return Err(GeometryError::TooManyMorphSurfaceControlPoints { maximum });
    }
    let mut cache = HashMap::new();
    let mut point_at = |uv: [Real; 2], sides: [ParameterSide; 2]| {
        let key = (
            uv.map(Real::to_bits),
            sides.map(|s| s == ParameterSide::Left),
        );
        if let Some(point) = cache.get(&key) {
            return Ok(*point);
        }
        if cache.len() >= maximum_samples {
            return Err(GeometryError::TooManyMorphSurfaceSamples {
                maximum: maximum_samples,
            });
        }
        let point =
            morph.morph_point(source.evaluate_on_sides(uv[0], uv[1], sides[0], sides[1])?)?;
        cache.insert(key, point);
        Ok::<_, GeometryError>(point)
    };
    let mut breaks = [
        seed_breaks(source.degree_u(), source.knots_u(), source.domain_u()),
        seed_breaks(source.degree_v(), source.knots_v(), source.domain_v()),
    ];
    // Moving controls is exact for affine maps and may suffice for simpler
    // nonlinear images. It is only a candidate, checked against the actual
    // mapped surface before it can be returned. A map need not be defined at
    // off-surface controls, so failure here does not preclude a valid fit.
    if let Ok(candidate) = mapped_controls(morph, source)
        && let Ok(errors) =
            validate::errors(&mut point_at, &candidate, &breaks, tolerance.absolute())
        && errors.deviation <= tolerance.absolute()
    {
        return Ok(candidate);
    }
    loop {
        if breaks.iter().any(|b| control_count(b) > maximum) {
            return Err(GeometryError::TooManyMorphSurfaceControlPoints { maximum });
        }
        let fitted = tensor::interpolate(&mut point_at, &breaks)?;
        let errors = validate::errors(&mut point_at, &fitted, &breaks, tolerance.absolute() * 0.8)?;
        if errors.deviation <= tolerance.absolute() * 0.8 {
            return Ok(fitted);
        }
        let mut added = 0;
        for (axis, axis_breaks) in breaks.iter_mut().enumerate() {
            added += refine(axis_breaks, &errors.directions[axis], maximum)?;
        }
        if added == 0 {
            if errors.deviation <= tolerance.absolute() {
                return Ok(fitted);
            }
            return Err(GeometryError::SurfaceMorphDidNotConverge {
                tolerance: tolerance.absolute(),
                deviation: errors.deviation,
                maximum,
            });
        }
    }
}

fn mapped_controls(
    morph: &(impl PointMorph + ?Sized),
    source: &NurbsSurface,
) -> Result<NurbsSurface, GeometryError> {
    let controls = source
        .control_points()
        .iter()
        .map(|control| {
            WeightedPoint3::try_new(morph.morph_point(control.point())?, control.weight())
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        source.control_point_count_u(),
        source.control_point_count_v(),
        controls,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )
}

fn refine(
    breaks: &mut Vec<Break>,
    errors: &[Real],
    maximum: usize,
) -> Result<usize, GeometryError> {
    let mut candidates = errors
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, error)| *error > 0.0)
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    candidates.truncate(maximum.saturating_sub(control_count(breaks)));
    let mut added = Vec::with_capacity(candidates.len());
    for (index, _) in candidates {
        let (a, b) = (breaks[index].parameter, breaks[index + 1].parameter);
        let t = stable_lerp(a, b, 0.5)?;
        if t > a && t < b {
            added.push(Break {
                parameter: t,
                multiplicity: 1,
            });
        }
    }
    let count = added.len();
    breaks.extend(added);
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    Ok(count)
}
