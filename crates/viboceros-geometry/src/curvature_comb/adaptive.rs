//! Source-only subdivision for a smooth outline with independently spaced hairs.
use super::*;
use crate::WeightedPoint3;
use std::ops::RangeInclusive;

// Empirical default outline policy, tested against public Rhino extraction.
// This is display sampling, not a curvature or chord-error certificate.
const OUTLINE_BASE: usize = 64;
const MAX_DEPTH: usize = 32;

/// Subdivide spans whose sampled tangents turn beyond a right angle or make
/// an angle above 60 degrees with their chord, then sample each leaf uniformly.
/// The default outline minimum is 64 + 2*order,
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
        let span = unit_span(&trim(curve, a..=b)?)?;
        let mut stack = vec![(span, 0., 1., 0)];
        while let Some((leaf, first, last, depth)) = stack.pop() {
            let sampled = curvature_comb(&leaf, intervals, leaf_samples)?;
            if wide_turn(leaf.control_points(), &sampled)? {
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
                stack.push((right, mid, last, depth + 1));
                stack.push((left, first, mid, depth + 1));
                continue;
            }
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

fn wide_turn(controls: &[WeightedPoint3], sampled: &CurvatureComb) -> Result<bool, GeometryError> {
    let first = controls[0];
    let last = controls[controls.len() - 1];
    let Some(start_neighbor) = controls.iter().find(|p| p.point() != first.point()) else {
        return Ok(false); // Constant span: sampling will count its zero speeds.
    };
    let end_neighbor = controls
        .iter()
        .rev()
        .find(|p| p.point() != last.point())
        .unwrap();
    if first.point() == last.point() {
        return Ok(true);
    }
    let chord = first.point().direction_to(last.point())?.as_vector();
    // For clamped rational Bezier spans, the first/last nonzero control
    // differences give the limiting tangent directions, with the weight sign.
    // Repeated endpoint controls still produce omitted zero-speed samples in
    // the evaluator; these limiting directions only guide subdivision.
    let start = weighted_direction(first, *start_neighbor)?.as_vector();
    let end = weighted_direction(*end_neighbor, last)?.as_vector();
    let slack = 64. * Real::EPSILON;
    let mut directions = vec![start, end];
    for tangent in sampled.hairs.iter().map(|h| h.tangent.as_vector()) {
        if !directions.contains(&tangent) {
            directions.push(tangent);
        }
    }
    for (i, tangent) in directions.iter().enumerate() {
        if tangent.dot(chord)? < 0.5 - slack {
            return Ok(true);
        }
        for other in &directions[..i] {
            if tangent.dot(*other)? < -slack {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn weighted_direction(a: WeightedPoint3, b: WeightedPoint3) -> Result<UnitVector3, GeometryError> {
    let direction = a.point().direction_to(b.point())?;
    Ok(
        if a.weight().is_sign_positive() == b.weight().is_sign_positive() {
            direction
        } else {
            direction.opposite()
        },
    )
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
