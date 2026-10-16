//! Source-only subdivision for a smooth outline with independently spaced hairs.
use super::*;
use crate::WeightedPoint3;
use std::ops::RangeInclusive;

// Empirical default outline policy, tested against public Rhino extraction.
// This is display sampling, not a curvature or chord-error certificate.
const OUTLINE_BASE: usize = 64;
const MAX_DEPTH: usize = 32;

/// Subdivide active control polygons that turn beyond a right angle, then
/// sample each leaf uniformly. The default outline minimum is 64 + 2*order,
/// rounded upward to a multiple of the number of hair intervals (density+1).
/// Both sides of knots and every subdivision boundary remain explicit.
///
/// The supplied exact trim producer allows native builds to reuse usd-brep.
/// Evaluation uses unit-domain single-span curves so curvature does not depend
/// on representability of derivatives in the source parameter chart. Original
/// parameter tags are retained for adjacency, including closed seams.
pub fn curvature_comb_with_subdivision<E: From<GeometryError>>(
    curve: &NurbsCurve,
    density: u8,
    maximum_samples: usize,
    trim: &mut impl FnMut(&NurbsCurve, RangeInclusive<Real>) -> Result<NurbsCurve, E>,
) -> Result<CurvatureComb, E> {
    if density > 10 {
        return Err(GeometryError::CurvatureCombLimit.into());
    }
    let hair_intervals = usize::from(density) + 1;
    let minimum = OUTLINE_BASE + 2 * (curve.degree() + 1);
    let intervals = minimum.div_ceil(hair_intervals) * hair_intervals;
    let leaf_samples = intervals + 1;
    let mut reserved = curve
        .spans()
        .count()
        .checked_mul(leaf_samples)
        .filter(|n| *n <= maximum_samples)
        .ok_or(GeometryError::CurvatureCombLimit)?;
    let mut out = CurvatureComb {
        closed: curve.is_closed()?,
        domain: [*curve.domain().start(), *curve.domain().end()],
        ..CurvatureComb::default()
    };
    for i in curve.degree()..curve.control_points().len() {
        let (a, b) = (curve.knots()[i], curve.knots()[i + 1]);
        if a == b {
            continue;
        }
        let split = wide_cone(&curve.control_points()[i - curve.degree()..=i])?;
        let span = unit_span(&trim(curve, a..=b)?)?;
        let mut stack = vec![(span, 0., 1., split, 0)];
        while let Some((leaf, first, last, split, depth)) = stack.pop() {
            if split {
                reserved = reserved
                    .checked_add(leaf_samples)
                    .filter(|n| *n <= maximum_samples)
                    .ok_or(GeometryError::CurvatureCombLimit)?;
                if depth == MAX_DEPTH {
                    return Err(GeometryError::CurvatureCombLimit.into());
                }
                let mid = (first + last) * 0.5;
                let left = unit_span(&trim(&leaf, 0. ..=0.5)?)?;
                let right = unit_span(&trim(&leaf, 0.5..=1.)?)?;
                let left_split = wide_cone(left.control_points())?;
                let right_split = wide_cone(right.control_points())?;
                stack.push((right, mid, last, right_split, depth + 1));
                stack.push((left, first, mid, left_split, depth + 1));
                continue;
            }
            let sampled = curvature_comb(&leaf, intervals, leaf_samples)?;
            let offset = out.hairs.len();
            let stride = intervals / hair_intervals;
            for mut hair in sampled.hairs {
                // A singular omission must not shift the displayed hair grid.
                let station = (hair.parameter * intervals as Real).round() as usize;
                hair.display_hair = station.is_multiple_of(stride);
                let fraction =
                    crate::parameter::map_parameter(hair.parameter, 0. ..=1., first..=last)?;
                hair.parameter = crate::parameter::map_parameter(fraction, 0. ..=1., a..=b)?;
                out.hairs.push(hair);
            }
            out.strips.extend(
                sampled
                    .strips
                    .into_iter()
                    .map(|r| (r.start + offset)..(r.end + offset)),
            );
            out.singular_samples += sampled.singular_samples;
        }
    }
    Ok(out)
}

fn wide_cone(controls: &[WeightedPoint3]) -> Result<bool, GeometryError> {
    let mut directions = Vec::with_capacity(controls.len().saturating_sub(1));
    for edge in controls.windows(2) {
        let delta = edge[0].point().vector_to(edge[1].point())?;
        if delta.length()? != 0. {
            directions.push(delta.normalized_nonzero()?.as_vector());
        }
    }
    for (i, a) in directions.iter().enumerate() {
        for b in &directions[i + 1..] {
            // A right angle can acquire a few ulps of negative dot product
            // after rotation, trimming or degree elevation.
            if a.dot(*b)? < -64. * Real::EPSILON {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn unit_span(curve: &NurbsCurve) -> Result<NurbsCurve, GeometryError> {
    let c = curve.clamped_to_active_domain()?;
    // Only two distinct knots are mapped. No interior knot can be rounded
    // away by a whole-domain normalization.
    if c.control_points().len() != c.degree() + 1 {
        return Err(GeometryError::CurvatureCombLimit);
    }
    let mut knots = vec![0.; c.degree() + 1];
    knots.resize(2 * (c.degree() + 1), 1.);
    NurbsCurve::try_new_rational(c.degree(), c.control_points().to_vec(), knots)
}
