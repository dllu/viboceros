//! Bounded adaptive proposals for sources with certified straight regions.
use super::*;
use crate::{ArcLengthSampler, CurveRef, ParameterSide, Point3, Tolerance};

/// Certified C0 cubic pieces before native Loft joining and region matching.
/// This does not establish Rhino's final control net or parameter chart.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveLinearRefitProposal {
    pub curve: NurbsCurve,
    /// Normalized arc-length intervals used to construct the raw pieces.
    pub arc_length_breaks: Vec<Real>,
    /// Complete symmetric image-distance bound against the original source.
    pub maximum_deviation_bound: Real,
}

const MAX_PIECES: usize = 128;
const MAX_DEPTH: usize = 24;

struct Interval {
    start: Real,
    end: Real,
    from: Real,
    to: Real,
    depth: usize,
}

/// Propose bounded adaptive cubics for exactly extracted straight regions with
/// no resolved one-degree tangent breaks. The source image and limiting frames
/// are retained through an exact, uniformly parameterized polygon proxy.
/// Seven interior stations select pieces using half the requested tolerance;
/// acceptance requires a complete image bound within the requested tolerance.
/// The result retains C0 joins and is not wired into Loft's native-compatible
/// command path while joining remains unqualified. Limits are 128 pieces,
/// depth 24, 512 source controls, and the shared exact-arithmetic work limits.
/// `None` means unsupported or unproved; no source mutation or partial output
/// escapes failure.
pub fn try_propose_adaptive_linear_refit(
    source: &NurbsCurve,
    tolerance: Real,
) -> Result<Option<AdaptiveLinearRefitProposal>, GeometryError> {
    if !tolerance.is_finite() || tolerance <= 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if source.control_points().len() > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "adaptive source controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    let Some(proxy) = linear_image_proxy(source)? else {
        return Ok(None);
    };
    if proxy.is_closed()? || super::super::parameterize::has_legacy_tangent_breaks(&proxy)? {
        return Ok(None);
    }
    // Exact region extraction retains the complete directed image and its
    // limiting tangent frames, while removing nonlinear rational span clocks.
    let source = &proxy;
    let domain = source.domain();
    let mut pending = vec![Interval {
        start: *domain.start(),
        end: *domain.end(),
        from: 0.,
        to: 1.,
        depth: 0,
    }];
    let mut pieces = Vec::new();
    let mut attempts = 0;
    while let Some(interval) = pending.pop() {
        attempts += 1;
        if attempts > 2 * MAX_PIECES - 1 {
            return Ok(None);
        }
        let Interval {
            start,
            end,
            from,
            to,
            depth,
        } = interval;
        let piece = crate::curve_fit::g1::initial_cubic_from_frames(
            source.evaluate(start)?,
            source.evaluate(end)?,
            source.tangent_at_on_side(start, ParameterSide::Right)?,
            source.tangent_at_on_side(
                end,
                if end == *domain.end() {
                    ParameterSide::Left
                } else {
                    ParameterSide::Right
                },
            )?,
        )?;
        let trimmed = source.try_trimmed(start..=end)?;
        let sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&trimmed), Tolerance::DEFAULT)?;
        let fractions = std::array::from_fn::<_, 9, _>(|i| i as Real / 8.);
        let parameters = sampler.parameters_at_incremental_normalized_lengths(&fractions, 1e-4)?;
        let search = piece.prepare_closest_parameter_search()?;
        let mut fits = true;
        for &t in &parameters[1..8] {
            let point = source.evaluate(t)?;
            let u = search.closest_parameter(point, Tolerance::DEFAULT)?;
            if point.distance_to(piece.evaluate(u)?)? > tolerance * 0.5 {
                fits = false;
                break;
            }
        }
        if fits {
            if pieces.len() == MAX_PIECES {
                return Ok(None);
            }
            pieces.push((to, piece));
        } else {
            if depth == MAX_DEPTH {
                return Ok(None);
            }
            let mut middle = parameters[4];
            // Retain an exact natural boundary when inversion differs only by
            // rounding; its original right-hand tangent remains authoritative.
            let roundoff = 16. * Real::EPSILON * (end - start);
            if let Some(&knot) = source
                .knots()
                .iter()
                .find(|&&k| k > start && k < end && (k - middle).abs() <= roundoff)
            {
                middle = knot;
            }
            let fraction = from + (to - from) * 0.5;
            if middle <= start || middle >= end || fraction <= from || fraction >= to {
                return Ok(None);
            }
            pending.push(Interval {
                start: middle,
                end,
                from: fraction,
                to,
                depth: depth + 1,
            });
            pending.push(Interval {
                start,
                end: middle,
                from,
                to: fraction,
                depth: depth + 1,
            });
        }
    }
    let mut controls = Vec::with_capacity(3 * pieces.len() + 1);
    let mut knots = vec![0.; 4];
    let mut breaks = vec![0.];
    for (end, curve) in pieces {
        controls.extend(
            curve
                .control_points()
                .iter()
                .copied()
                .skip(usize::from(!controls.is_empty())),
        );
        knots.extend([end; 3]);
        breaks.push(end);
    }
    knots.push(1.);
    let curve = NurbsCurve::try_new_rational(3, controls, knots)?;
    let maximum_deviation_bound =
        if let Some(bound) = proxy.parameter_curve_deviation_bound(&curve, tolerance)? {
            bound
        } else if crate::curve_fit::g1::within_deviation(&proxy, &curve, tolerance)? {
            tolerance
        } else {
            return Ok(None);
        };
    Ok(Some(AdaptiveLinearRefitProposal {
        curve,
        arc_length_breaks: breaks,
        maximum_deviation_bound,
    }))
}

/// Exact straight-region extraction proves equality of complete images, even
/// for nonlinear rational clocks. Require exactly representable proxy vertices;
/// the existing polygon path separately handles rounded active vertices.
fn linear_image_proxy(source: &NurbsCurve) -> Result<Option<NurbsCurve>, GeometryError> {
    let Some(vertices) = crate::surface_pullback::exact_linear_curve_vertices(source)? else {
        return Ok(None);
    };
    let mut points = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        let [x, y, z] = vertex.each_ref().map(scalar);
        let xyz = [x?, y?, z?];
        if (0..3).any(|i| rational(xyz[i]) != vertex[i]) {
            return Ok(None);
        }
        points.push(Point3::try_from(xyz)?);
    }
    let mut knots = vec![0., 0.];
    let mut length = 0.;
    for pair in points.windows(2) {
        let next = length + pair[0].distance_to(pair[1])?;
        if !next.is_finite() || next <= length {
            return Ok(None);
        }
        length = next;
        knots.push(length);
    }
    knots.push(length);
    Ok(Some(NurbsCurve::try_new(1, points, knots)?))
}

#[cfg(test)]
mod tests;
