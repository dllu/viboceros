//! Piecewise section parameters from matched G2 regions.
//! The later shared-basis pass retains ownership of clamping/elevation/refinement.
use super::{MAX_LOFT_SECTION_CONTROLS, MAX_LOFT_SECTIONS};
use crate::integration::cash_karp;
use crate::{CurveContinuity, GeometryError, NurbsCurve, Real, require_finite};
use std::borrow::Cow;
mod fitted;
#[cfg(test)]
mod tests;
pub(super) use fitted::region_breaks as fitted_region_breaks;

#[derive(Clone, Copy)]
enum RegionPolicy {
    Geometric,
    Fitted,
}

struct Section {
    curve: NurbsCurve,
    breaks: Vec<Real>,
    parameters: Vec<Real>,
    lengths: Vec<Real>,
    scale: Real,
}

pub(crate) fn prepare<'a, E: From<GeometryError>>(
    curves: &'a [NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
) -> Result<Cow<'a, [NurbsCurve]>, E> {
    prepare_impl(
        curves,
        clamp,
        1_f64.to_radians().cos(),
        true,
        RegionPolicy::Geometric,
    )
}

pub(super) fn prepare_refit<'a, E: From<GeometryError>>(
    curves: &'a [NurbsCurve],
    angle: Option<Real>,
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
) -> Result<Cow<'a, [NurbsCurve]>, E> {
    // The public Refit overload uses its default region angle at either end
    // of the angular range. Degree-one profiles already share their chart
    // when their knot vectors match; higher degrees still match G2 regions.
    let cosine = angle
        .filter(|a| *a > 0. && *a < std::f64::consts::PI)
        .map_or_else(|| 1_f64.to_radians().cos(), Real::cos);
    prepare_impl(
        curves,
        clamp,
        cosine,
        curves.iter().all(|c| c.degree() == 1),
        RegionPolicy::Geometric,
    )
}

/// A fitter has already retained its independent C0 region boundaries. Match
/// those regions even when all fitted profiles initially share their basis.
/// Ordinary and disabled-fitting Loft retain their geometric region policy.
pub(super) fn prepare_fitted<'a, E: From<GeometryError>>(
    curves: &'a [NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
) -> Result<Cow<'a, [NurbsCurve]>, E> {
    prepare_impl(
        curves,
        clamp,
        1_f64.to_radians().cos(),
        false,
        RegionPolicy::Fitted,
    )
}

fn prepare_impl<'a, E: From<GeometryError>>(
    curves: &'a [NurbsCurve],
    clamp: &mut impl FnMut(&NurbsCurve) -> Result<NurbsCurve, E>,
    cosine: Real,
    preserve_matching_basis: bool,
    policy: RegionPolicy,
) -> Result<Cow<'a, [NurbsCurve]>, E> {
    if curves.is_empty() || curves.len() > MAX_LOFT_SECTIONS {
        return Err(GeometryError::LoftResourceLimit {
            context: "section count",
            maximum: MAX_LOFT_SECTIONS,
        }
        .into());
    }
    let control_limit = GeometryError::LoftResourceLimit {
        context: "compatible section controls",
        maximum: MAX_LOFT_SECTION_CONTROLS,
    };
    if curves
        .iter()
        .any(|c| c.control_points().len() > MAX_LOFT_SECTION_CONTROLS)
    {
        return Err(control_limit.into());
    }
    if preserve_matching_basis
        && curves
            .iter()
            .all(|c| c.degree() == curves[0].degree() && c.knots() == curves[0].knots())
    {
        return Ok(Cow::Borrowed(curves));
    }
    // Native connected-curve queries cannot represent positional jumps. Keep
    // the existing structural correspondence for these segmented kernel curves.
    // Degrees above the bounded native speed-query range use that path as well.
    if curves.iter().any(|c| {
        c.degree() > 64
            || c.knots().chunk_by(|a, b| a == b).any(|g| {
                g[0] > *c.domain().start() && g[0] < *c.domain().end() && g.len() > c.degree()
            })
    }) {
        return Ok(Cow::Borrowed(curves));
    }
    let mut sections = Vec::with_capacity(curves.len());
    let mut remaining_steps = cash_karp::MAX_STEPS;
    for source in curves {
        let curve = clamp(source)?;
        if curve.degree() != source.degree() || curve.domain() != source.domain() {
            return Err(GeometryError::InvalidControlNet {
                context: "section clamping changed degree or active domain",
            }
            .into());
        }
        if curve.control_points().len() > MAX_LOFT_SECTION_CONTROLS {
            return Err(control_limit.clone().into());
        }
        let domain = curve.domain();
        if curve.knots().first() != Some(domain.start())
            || curve.knots().last() != Some(domain.end())
        {
            return Err(GeometryError::InvalidControlNet {
                context: "section clamping did not clamp the active domain",
            }
            .into());
        }
        let scale = coordinate_scale(&curve);
        let mut breaks = vec![*domain.start()];
        breaks.extend(match policy {
            RegionPolicy::Geometric => region_breaks(&curve, scale, cosine)?,
            RegionPolicy::Fitted => independent_breaks(&curve),
        });
        breaks.push(*domain.end());
        sections.push(Section {
            curve,
            breaks,
            parameters: Vec::new(),
            lengths: Vec::new(),
            scale,
        });
    }
    if sections.iter().all(|s| s.breaks.len() == 2) {
        // One region only needs an affine domain change, already provided by
        // the exact shared-basis pass. Avoid quadrature and knot roundoff here.
        return Ok(Cow::Borrowed(curves));
    }
    for section in &mut sections {
        for interval in section.breaks.windows(2) {
            let piece = section.curve.try_trimmed(interval[0]..=interval[1])?;
            let length = coarse_length(&piece, section.scale, &mut remaining_steps)?;
            if length <= 0. {
                return Err(GeometryError::Degenerate {
                    context: "Loft section length interval",
                }
                .into());
            }
            section.lengths.push(length);
        }
        section.parameters = prefix(&section.lengths)?;
    }
    let count = sections.iter().map(|s| s.lengths.len()).max().unwrap();
    // Only sections with the largest region count define the reference chart.
    // A common binary unit permits direct maxima without overflowing model units.
    let scale = sections
        .iter()
        .filter(|s| s.lengths.len() == count)
        .map(|s| s.scale)
        .fold(0_f64, Real::max);
    let mut lengths = vec![0_f64; count];
    for section in sections.iter().filter(|s| s.lengths.len() == count) {
        let ratio = section.scale / scale;
        for (length, source_length) in lengths.iter_mut().zip(&section.lengths) {
            *length = length.max(source_length * ratio);
        }
    }
    let parameters = prefix(&lengths)?;
    let end = *parameters.last().unwrap();
    let mut result = Vec::with_capacity(sections.len());
    for section in &sections {
        let Section {
            curve,
            breaks,
            parameters: source_parameters,
            ..
        } = section;
        let matched = match_breaks(source_parameters, &parameters);
        let domain = curve.domain();
        let knots = curve
            .knots()
            .iter()
            .map(|&t| {
                if t == *domain.start() {
                    return 0.;
                }
                if t == *domain.end() {
                    return end;
                }
                let index = breaks
                    .partition_point(|&b| b <= t)
                    .saturating_sub(1)
                    .min(breaks.len() - 2);
                let fraction = (t - breaks[index]) / (breaks[index + 1] - breaks[index]);
                matched[index] + fraction * (matched[index + 1] - matched[index])
            })
            .collect();
        let normalized =
            NurbsCurve::try_new_rational(curve.degree(), curve.control_points().to_vec(), knots)?
                .try_reparameterized(0. ..=1.)?;
        if curve
            .knots()
            .windows(2)
            .zip(normalized.knots().windows(2))
            .any(|(before, after)| before[0] < before[1] && after[0] >= after[1])
        {
            return Err(GeometryError::InvalidControlNet {
                context: "Loft parameter mapping collapsed an interval",
            }
            .into());
        }
        result.push(normalized);
    }
    consolidate_roundoff(&mut result)?;
    Ok(Cow::Owned(result))
}

// Piecewise affine knot changes preserve controls only across independent
// Bezier boundaries (full multiplicity). Remapping a simple smooth knot would
// deform the profile, even when curvature changes there.
fn region_breaks(
    curve: &NurbsCurve,
    scale: Real,
    cosine: Real,
) -> Result<Vec<Real>, GeometryError> {
    let candidates = independent_breaks(curve);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    Ok(geometric_breaks(curve, scale, cosine, CurveContinuity::G2)?
        .into_iter()
        .filter(|t| candidates.binary_search_by(|c| c.total_cmp(t)).is_ok())
        .collect())
}

fn independent_breaks(curve: &NurbsCurve) -> Vec<Real> {
    curve
        .knots()
        .chunk_by(|a, b| a == b)
        .filter(|g| {
            g.len() >= curve.degree()
                && g[0] > *curve.domain().start()
                && g[0] < *curve.domain().end()
        })
        .map(|g| g[0])
        .collect()
}

pub(super) fn has_tangent_breaks(curve: &NurbsCurve, angle: Real) -> Result<bool, GeometryError> {
    Ok(!geometric_breaks(
        curve,
        coordinate_scale(curve),
        angle.cos(),
        CurveContinuity::G1,
    )?
    .is_empty())
}

fn geometric_breaks(
    curve: &NurbsCurve,
    scale: Real,
    cosine: Real,
    continuity: CurveContinuity,
) -> Result<Vec<Real>, GeometryError> {
    // Classification is geometric: remove the world offset and scale before
    // testing curvature. This prevents distant, exact circles gaining breaks
    // from cancellation in homogeneous derivative evaluation.
    let origin = curve.control_points()[0]
        .point()
        .to_array()
        .map(|x| x / scale);
    let mut controls = curve
        .control_points()
        .iter()
        .map(|p| {
            let q = p.point().to_array();
            [
                q[0] / scale - origin[0],
                q[1] / scale - origin[1],
                q[2] / scale - origin[2],
                p.weight(),
            ]
        })
        .collect::<Vec<_>>();
    let extent = controls
        .iter()
        .flat_map(|p| &p[..3])
        .map(|x| x.abs())
        .fold(0_f64, Real::max);
    let local_scale = binary_scale(extent);
    for p in &mut controls {
        for x in &mut p[..3] {
            *x /= local_scale;
        }
    }
    Ok(viboceros_opennurbs::curve_continuity_breaks(
        curve.degree(),
        &controls,
        curve.knots(),
        continuity,
        cosine,
    )?)
}

// Binary scaling preserves ordinary evaluation operation boundaries and keeps
// extremely small/large coordinate units away from quadrature's absolute floor.
fn coordinate_scale(curve: &NurbsCurve) -> Real {
    let maximum = curve
        .control_points()
        .iter()
        .flat_map(|p| p.point().to_array())
        .map(Real::abs)
        .fold(0_f64, Real::max);
    binary_scale(maximum)
}

fn binary_scale(maximum: Real) -> Real {
    let exponent = maximum.to_bits() & 0x7ff0_0000_0000_0000;
    if exponent == 0 {
        Real::MIN_POSITIVE
    } else {
        Real::from_bits(exponent)
    }
}

fn prefix(lengths: &[Real]) -> Result<Vec<Real>, GeometryError> {
    let mut result = Vec::with_capacity(lengths.len() + 1);
    result.push(0.);
    for &length in lengths {
        let previous = *result.last().unwrap();
        let next = previous + length;
        if !next.is_finite() || next <= previous {
            return Err(GeometryError::InvalidControlNet {
                context: "Loft length intervals are not distinguishable",
            });
        }
        result.push(next);
    }
    Ok(result)
}

fn coarse_length(
    curve: &NurbsCurve,
    scale: Real,
    remaining_steps: &mut usize,
) -> Result<Real, GeometryError> {
    let mut total: Real = 0.;
    let mut correction = 0.;
    for (a, b) in curve.spans() {
        let normalized = curve.try_trimmed(a..=b)?.try_reparameterized(0. ..=1.)?;
        let controls = normalized
            .control_points()
            .iter()
            .map(|p| {
                let q = p.point();
                [q.x() / scale, q.y() / scale, q.z() / scale, p.weight()]
            })
            .collect::<Vec<_>>();
        if *remaining_steps == 0 {
            return Err(GeometryError::LoftResourceLimit {
                context: "length integration steps",
                maximum: cash_karp::MAX_STEPS,
            });
        }
        let trace = cash_karp::integrate(1e-3, *remaining_steps, |parameter| {
            Ok(viboceros_opennurbs::curve_speeds(
                normalized.degree(),
                &controls,
                normalized.knots(),
                &[parameter],
            )?[0])
        })?;
        *remaining_steps -= trace.attempted_steps;
        let length = trace.checkpoints.last().unwrap().integral;
        let next = total + length;
        correction += if total.abs() >= length.abs() {
            (total - next) + length
        } else {
            (length - next) + total
        };
        total = next;
    }
    let length = total + correction;
    require_finite([length], "Loft section length estimate")?;
    Ok(length)
}

fn match_breaks(source: &[Real], reference: &[Real]) -> Vec<Real> {
    let n = source.len() - 2;
    let m = reference.len() - 2;
    if n == m {
        return reference.to_vec();
    }
    let source_end = *source.last().unwrap();
    let reference_end = *reference.last().unwrap();
    let mut result = vec![0.];
    let mut previous = 0;
    for (i, &parameter) in source[1..source.len() - 1].iter().enumerate() {
        let low = previous + 1;
        let high = m - (n - i - 1);
        let target = (parameter / source_end) * reference_end;
        let candidates = &reference[low..=high];
        let right = candidates
            .partition_point(|&p| p < target)
            .min(candidates.len() - 1);
        let left = right.saturating_sub(1);
        let chosen = if (candidates[left] - target).abs() <= (candidates[right] - target).abs() {
            left
        } else {
            right
        };
        previous = low + chosen;
        result.push(reference[previous]);
    }
    result.push(reference_end);
    result
}

fn consolidate_roundoff(curves: &mut [NurbsCurve]) -> Result<(), GeometryError> {
    let mut knots = curves
        .iter()
        .enumerate()
        .flat_map(|(i, c)| c.knots().chunk_by(|a, b| a == b).map(move |g| (g[0], i)))
        .collect::<Vec<_>>();
    knots.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut replacements = vec![Vec::new(); curves.len()];
    for group in knots.chunk_by(|a, b| (a.0 - b.0).abs() <= 32. * Real::EPSILON) {
        let distinct = group
            .iter()
            .map(|p| p.1)
            .collect::<std::collections::BTreeSet<_>>();
        if distinct.len() != group.len() {
            continue;
        }
        let reference = group.iter().min_by_key(|p| p.1).unwrap().0;
        for &(value, index) in group {
            replacements[index].push((value, reference));
        }
    }
    for (curve, replacements) in curves.iter_mut().zip(replacements) {
        let knots = curve
            .knots()
            .iter()
            .map(|t| {
                replacements
                    .binary_search_by(|p| p.0.total_cmp(t))
                    .map_or(*t, |i| replacements[i].1)
            })
            .collect();
        *curve =
            NurbsCurve::try_new_rational(curve.degree(), curve.control_points().to_vec(), knots)?;
    }
    Ok(())
}
