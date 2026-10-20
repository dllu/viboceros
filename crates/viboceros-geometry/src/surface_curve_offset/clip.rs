//! Natural rectangular boundary clipping after bounded end-span continuation.
use super::*;
use crate::{Point3, surface_geodesic::chart_closes};

const MAX_EXTENSION_ATTEMPTS: usize = 8;
const MAX_CLIP_CONTACTS: usize = 8192;
mod closed_gap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// How retained portions of a closed source are returned after clipping.
pub enum SurfaceCurveOffsetGapStyle {
    #[default]
    /// Return only the natural-domain portions, joined through the old seam.
    Open,
    /// Reconnect closed-source gaps on qualified affine planar patches, as in
    /// the captured SDK results. Connector spans can leave the natural domain.
    ConnectClosed,
}

#[derive(Clone, Copy, Debug, Default)]
/// Clipping output policy and an optional shared interpolation producer.
pub struct SurfaceCurveOffsetOptions {
    pub closed_gaps: SurfaceCurveOffsetGapStyle,
    pub interpolation_solver: Option<MorphInterpolationSolver>,
}

/// Offset pieces in source traversal order. Open pieces retain source
/// intervals; a seam-crossing piece ends in the following period. Reconnection
/// shifts intervals and inserts unit bridge spans, using source-period spans
/// if unit spans cannot represent all retained geometry accurately.
/// Empty output means the fitted locus lies outside the natural boundaries.
#[derive(Clone, Debug)]
pub struct SurfaceCurveOffsetPieces {
    pub curves: Vec<NurbsCurve>,
    pub source_deviation_bound: Real,
    pub geodesic_queries: usize,
    pub integration_steps: usize,
    pub maximum_integration_estimate: Real,
    pub extension_attempts: usize,
    pub connectors: usize,
}

/// Construct and clip all smooth offset portions to the surface's natural
/// rectangle. Exact periodic seams are not borders. This shares the strict
/// offset's sampled fitting policy; it does not certify topology of the true
/// geodesic locus. Trims and cut-locus/corner handling remain separate work.
pub fn try_offset_curves_on_surface(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
) -> Result<SurfaceCurveOffsetPieces, GeometryError> {
    clipped(
        surface,
        source,
        distance,
        tolerance,
        SurfaceCurveOffsetOptions::default(),
    )
}

pub fn try_offset_curves_on_surface_with_solver(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    solver: MorphInterpolationSolver,
) -> Result<SurfaceCurveOffsetPieces, GeometryError> {
    clipped(
        surface,
        source,
        distance,
        tolerance,
        SurfaceCurveOffsetOptions {
            interpolation_solver: Some(solver),
            ..SurfaceCurveOffsetOptions::default()
        },
    )
}

pub fn try_offset_curves_on_surface_with_options(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    options: SurfaceCurveOffsetOptions,
) -> Result<SurfaceCurveOffsetPieces, GeometryError> {
    clipped(surface, source, distance, tolerance, options)
}

fn clipped(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    options: SurfaceCurveOffsetOptions,
) -> Result<SurfaceCurveOffsetPieces, GeometryError> {
    let (surface, source, numerical) =
        normalized_sources(original_surface, original_source, distance, tolerance)?;
    // Admission always refers to the original natural domain, even when the
    // subsequent construction needs temporary endpoint-span continuation.
    let (_, source_deviation_bound) =
        surface.try_pullback_curve_certified_with_bound(&source, None, numerical)?;
    let mut result = SurfaceCurveOffsetPieces {
        curves: Vec::new(),
        source_deviation_bound,
        geodesic_queries: 0,
        integration_steps: 0,
        maximum_integration_estimate: 0.,
        extension_attempts: 0,
        connectors: 0,
    };
    if distance == 0. {
        result.curves.push(original_source.clone());
        return Ok(result);
    }
    let closed = [chart_closes(&surface, 0)?, chart_closes(&surface, 1)?];
    let origin = surface.control_points()[0].point();
    let mut scale: Real = 0.;
    for cp in surface.control_points() {
        scale = scale.max(origin.distance_to(cp.point())?);
    }
    if scale == 0. {
        return Err(invalid("constant surface"));
    }
    let initial_margin = (distance.abs() / scale).max(0.25);
    let mut diagnostics = Diagnostics::default();
    for attempt in 0..=MAX_EXTENSION_ATTEMPTS {
        let extended = if attempt == 0 {
            surface.clone()
        } else {
            let margin = initial_margin * 2_f64.powi(attempt as i32);
            let interval = -margin..=1. + margin;
            surface.extended_for_offset(closed.map(|closed| (!closed).then(|| interval.clone())))?
        };
        let mut sampler = OffsetSampler::new(&extended, &source, distance, numerical)?;
        let fitting = Tolerance::try_new(
            tolerance.absolute() * 0.2,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let solver = options.interpolation_solver;
        let fitted = fit_complete(&mut sampler, fitting, solver);
        let pieces = fitted.and_then(|fitted| {
            clip_curve(&surface, &fitted, closed, &mut sampler, tolerance, solver)
        });
        diagnostics.include(sampler.diagnostics);
        match pieces {
            Ok(pieces) => {
                result.curves = pieces
                    .iter()
                    .map(|c| restore_domain(c, original_source, tolerance))
                    .collect::<Result<Vec<_>, _>>()?;
                if options.closed_gaps == SurfaceCurveOffsetGapStyle::ConnectClosed
                    && source.evaluate(0.)? == source.evaluate(1.)?
                {
                    (result.curves, result.connectors) = closed_gap::connect(
                        &surface,
                        result.curves,
                        tolerance,
                        original_source.domain().end() - original_source.domain().start(),
                    )?;
                }
                result.geodesic_queries = diagnostics.queries;
                result.integration_steps = diagnostics.steps;
                result.maximum_integration_estimate = diagnostics.estimate;
                result.extension_attempts = attempt;
                return Ok(result);
            }
            Err(GeometryError::ParameterCurveOutsideSurfaceDomain)
                if attempt < MAX_EXTENSION_ATTEMPTS =>
            {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("the final failed attempt returns its error")
}

/// Same-sign weights give |S'| <= 3 B p/(min_span * min_weight),
/// after max_weight=1 and centering controls in a radius-B ball. This bound
/// selects a model-scaled UV fit budget; contacts still get model-space checks.
pub(super) fn uv_tolerance(
    surface: &NurbsSurface,
    closed: [bool; 2],
    tolerance: Tolerance,
) -> Result<Tolerance, GeometryError> {
    let cp = surface.control_points();
    let sign = cp[0].weight().is_sign_positive();
    if cp.iter().any(|p| p.weight().is_sign_positive() != sign) {
        return Err(invalid(
            "boundary clipping requires same-sign surface weights",
        ));
    }
    let gauge = cp.iter().map(|p| p.weight().abs()).fold(0., Real::max);
    let min_weight = cp
        .iter()
        .map(|p| p.weight().abs() / gauge)
        .fold(1., Real::min);
    let origin = cp[0].point();
    let mut radius: Real = 0.;
    for p in cp {
        radius = radius.max(origin.distance_to(p.point())?);
    }
    let mut derivative: Real = 0.;
    if !closed[0] {
        derivative += surface.degree_u() as Real
            / surface
                .spans_u()
                .map(|(a, b)| b - a)
                .fold(Real::INFINITY, Real::min);
    }
    if !closed[1] {
        derivative += surface.degree_v() as Real
            / surface
                .spans_v()
                .map(|(a, b)| b - a)
                .fold(Real::INFINITY, Real::min);
    }
    let absolute = tolerance.absolute() / radius * min_weight / (3. * derivative) * 0.05;
    if !absolute.is_finite() || absolute <= 0. {
        return Err(invalid("boundary parameter accuracy is unrepresentable"));
    }
    Tolerance::try_new(absolute, Real::MIN_POSITIVE, tolerance.angular())
}

fn boundary_proposal(
    surface: &NurbsSurface,
    fitted: &NurbsCurve,
    closed: [bool; 2],
    sampler: &mut OffsetSampler<'_>,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<(NurbsCurve, Tolerance), GeometryError> {
    let tolerance = uv_tolerance(surface, closed, tolerance)?;
    let breaks = seed_breaks(fitted.degree(), fitted.knots(), fitted.domain());
    let source = sampler.source;
    let curve = fit_parameterized_curve(
        source,
        breaks,
        tolerance,
        MAX_CURVE_FIT_CONTROL_POINTS,
        solver,
        |t, side| {
            let uv = sampler.sample(t, side)?.uv.to_array();
            Point3::try_new(
                if closed[0] { 0. } else { uv[0] },
                if closed[1] { 0. } else { uv[1] },
                0.,
            )
        },
    )?;
    Ok((curve, tolerance))
}

fn contains(
    surface: &NurbsSurface,
    sample: SurfaceGeodesicPoint,
    closed: [bool; 2],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let uv = sample.uv.to_array();
    let clamped = std::array::from_fn::<_, 2, _>(|i| {
        if closed[i] {
            uv[i]
        } else {
            uv[i].clamp(0., 1.)
        }
    });
    let on_surface = surface.evaluate(clamped[0], clamped[1])?;
    let residual = on_surface.distance_to(sample.point)?;
    if uv == clamped && residual > tolerance.absolute() * 0.1 {
        return Err(invalid(
            "temporary extension changed the original surface beyond tolerance",
        ));
    }
    Ok(uv == clamped || residual <= tolerance.absolute() * 0.5)
}

fn clip_curve(
    surface: &NurbsSurface,
    fitted: &NurbsCurve,
    closed: [bool; 2],
    sampler: &mut OffsetSampler<'_>,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    if closed == [true, true] {
        return Ok(vec![fitted.clone()]);
    }
    // Intersect the known UV family: two slightly separated spatial fits can
    // miss a transverse chart-border crossing even with a distance tolerance.
    let (proposal, parameter_tolerance) =
        boundary_proposal(surface, fitted, closed, sampler, tolerance, solver)?;
    let intersection_tolerance = Tolerance::try_new(
        parameter_tolerance.absolute() * 0.2,
        Real::MIN_POSITIVE,
        tolerance.angular(),
    )?;
    let mut cuts = vec![0., 1.];
    for axis in 0..2 {
        if closed[axis] {
            continue;
        }
        for value in [0., 1.] {
            let other = 1 - axis;
            let lo = proposal
                .control_points()
                .iter()
                .map(|p| p.point().to_array()[other])
                .fold(0., Real::min)
                - 1.;
            let hi = proposal
                .control_points()
                .iter()
                .map(|p| p.point().to_array()[other])
                .fold(1., Real::max)
                + 1.;
            let point = |q| {
                Point3::try_from(std::array::from_fn(|i| {
                    if i == axis {
                        value
                    } else if i == other {
                        q
                    } else {
                        0.
                    }
                }))
            };
            let border =
                NurbsCurve::try_new(1, vec![point(lo)?, point(hi)?], vec![0., 0., 1., 1.])?;
            for contact in proposal.intersections_with_curve(&border, intersection_tolerance)? {
                let t = contact.first_parameter();
                let shot = sampler.sample(t, ParameterSide::Right)?;
                let mut uv = shot.uv.to_array();
                if (uv[axis] - value).abs() > parameter_tolerance.absolute() * 3. {
                    return Err(invalid(
                        "UV boundary proposal did not reach the true offset family",
                    ));
                }
                if (0. ..=1.).contains(&uv[other]) {
                    uv[axis] = value;
                    if fitted
                        .evaluate(t)?
                        .distance_to(surface.evaluate(uv[0], uv[1])?)?
                        > tolerance.absolute() * 0.25
                    {
                        return Err(invalid(
                            "boundary contact cannot be represented accurately in world coordinates",
                        ));
                    }
                }
                cuts.push(t);
                if cuts.len() > MAX_CLIP_CONTACTS {
                    return Err(invalid("too many natural-boundary contacts"));
                }
            }
        }
    }
    cuts.sort_by(Real::total_cmp);
    cuts.dedup();
    let mut intervals: Vec<[Real; 2]> = Vec::new();
    for pair in cuts.windows(2) {
        let t = pair[0].midpoint(pair[1]);
        if t <= pair[0] || t >= pair[1] {
            return Err(invalid("clipping interval is below parameter resolution"));
        }
        if contains(
            surface,
            sampler.sample(t, ParameterSide::Right)?,
            closed,
            tolerance,
        )? {
            if let Some(last) = intervals.last_mut().filter(|last| last[1] == pair[0]) {
                last[1] = pair[1];
            } else {
                intervals.push([pair[0], pair[1]]);
            }
        }
    }
    let mut pieces = Vec::new();
    for &[a, b] in &intervals {
        for (lo, hi) in fitted.spans() {
            let (lo, hi) = (lo.max(a), hi.min(b));
            if lo >= hi {
                continue;
            }
            for fraction in error_fractions(8) {
                let t = stable_lerp(lo, hi, fraction)?;
                let shot = sampler.sample(
                    t,
                    if t == hi {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    },
                )?;
                let inside = contains(surface, shot, closed, tolerance)?;
                let error = fitted.evaluate(t)?.distance_to(shot.point)?;
                if !inside || error > tolerance.absolute() * 0.2 {
                    return Err(invalid(
                        "boundary contacts did not isolate an accurate interior offset portion",
                    ));
                }
            }
        }
    }
    let cyclic = fitted.evaluate(0.)? == fitted.evaluate(1.)?;
    if cyclic && intervals.len() > 1 && intervals[0][0] == 0. && intervals.last().unwrap()[1] == 1.
    {
        let first = intervals.remove(0);
        let last = intervals.pop().unwrap();
        pieces.push(
            fitted
                .try_change_closed_seam(last[0])?
                .try_trimmed(last[0]..=1. + first[1])?,
        );
    }
    for [a, b] in intervals {
        pieces.push(fitted.try_trimmed(a..=b)?);
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests;
