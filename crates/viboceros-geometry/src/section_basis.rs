//! Common section bases and explicit rational weight policies.

use crate::{GeometryError, NurbsCurve, Real, WeightedPoint3};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub enum WeightScale {
    PerSection,
    Common,
}

pub(crate) fn prepare(
    curves: &[NurbsCurve],
    maximum: usize,
    limit: GeometryError,
    weight_scale: WeightScale,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    prepare_with_operations(
        curves,
        maximum,
        limit,
        weight_scale,
        &mut |curve| curve.clamped_to_active_domain(),
        &mut |curve, degree| curve.try_change_degree(degree, false),
        &mut |curve, knot, target| curve.try_insert_knot(knot, target),
    )
}

/// Prepare a shared normalized section basis while delegating exact clamping,
/// degree elevation and refinement to the caller's geometry backend.
pub fn prepare_with_operations<E: From<GeometryError>>(
    curves: &[NurbsCurve],
    maximum: usize,
    limit: GeometryError,
    weight_scale: WeightScale,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    elevate: &mut impl FnMut(&NurbsCurve, usize) -> Result<NurbsCurve, E>,
    refine: &mut impl FnMut(&NurbsCurve, Real, usize) -> Result<NurbsCurve, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let degree = curves
        .iter()
        .map(NurbsCurve::degree)
        .max()
        .ok_or(limit.clone())?;
    let mut sections = Vec::with_capacity(curves.len());
    let common_scale = curves
        .iter()
        .flat_map(|c| c.control_points())
        .map(|c| c.weight().abs())
        .fold(0.0, Real::max);
    for curve in curves {
        let count = degree
            + 1
            + curve
                .knots()
                .chunk_by(|a, b| a == b)
                .filter(|g| g[0] > *curve.domain().start() && g[0] < *curve.domain().end())
                .map(|g| degree - curve.degree() + g.len())
                .sum::<usize>();
        if count > maximum {
            return Err(limit.into());
        }
        let scale = match weight_scale {
            WeightScale::Common => common_scale,
            WeightScale::PerSection => curve
                .control_points()
                .iter()
                .map(|c| c.weight().abs())
                .fold(0.0, Real::max),
        };
        let normalized = NurbsCurve::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() / scale))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knots().to_vec(),
        )?;
        let clamped = clamp(&normalized)?;
        if clamped.degree() != curve.degree() || clamped.domain() != curve.domain() {
            return Err(GeometryError::InvalidControlNet {
                context: "section clamping changed degree or active domain",
            }
            .into());
        }
        let elevated = elevate(&clamped, degree)?;
        if elevated.degree() != degree || elevated.domain() != clamped.domain() {
            return Err(GeometryError::InvalidControlNet {
                context: "section elevation changed target degree or active domain",
            }
            .into());
        }
        sections.push(elevated.try_reparameterized(0.0..=1.0)?);
    }
    let mut union = sections
        .iter()
        .flat_map(|c| {
            c.knots()
                .chunk_by(|a, b| a == b)
                .filter(|g| g[0] > 0.0 && g[0] < 1.0)
                .map(|g| (g[0], g.len()))
        })
        .collect::<Vec<_>>();
    union.sort_by(|a, b| a.0.total_cmp(&b.0));
    let union = union
        .chunk_by(|a, b| a.0 == b.0)
        .map(|g| (g[0].0, g.iter().map(|x| x.1).max().unwrap()))
        .collect::<Vec<_>>();
    if degree + 1 + union.iter().map(|x| x.1).sum::<usize>() > maximum {
        return Err(limit.into());
    }
    for section in &mut sections {
        for &(knot, multiplicity) in &union {
            *section = refine(section, knot, multiplicity)?;
        }
    }
    if sections.iter().any(|s| {
        s.degree() != degree
            || s.domain() != (0.0..=1.0)
            || s.control_points().len() > maximum
            || s.knots() != sections[0].knots()
    }) {
        return Err(GeometryError::InvalidControlNet {
            context: "section refinement produced inconsistent bases",
        }
        .into());
    }
    Ok(sections)
}

/// Rhino's common-basis end-weight policy, after structural matching.
/// Applies the shared numerical end-weight normalizer, then retains the first
/// section's transformed knots, which may change later multi-span profiles' loci.
pub(crate) fn normalized_end_weights<'a>(
    curves: impl IntoIterator<Item = &'a NurbsCurve>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let mut sections = curves
        .into_iter()
        .map(NurbsCurve::try_normalized_end_weights)
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(first) = sections.first() {
        let knots = first.knots().to_vec();
        for section in &mut sections[1..] {
            *section = NurbsCurve::try_new_rational(
                section.degree(),
                section.control_points().to_vec(),
                knots.clone(),
            )?;
        }
    }
    Ok(sections)
}
