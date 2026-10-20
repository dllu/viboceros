//! Sampled cubic fitting of the complete surface-distance offset locus.
use crate::{
    CurveRef, GeometryError, MAX_CURVE_FIT_CONTROL_POINTS, MorphInterpolationSolver, NurbsCurve,
    NurbsCurve2, NurbsSurface, ParameterSide, PreparedSurfaceGeodesic, Real, SurfaceGeodesicPoint,
    Tolerance,
    morph::{fit_parameterized_curve, fit_parameterized_curve_with_derivatives},
    spline_collocation::{error_fractions, seed_breaks, stable_lerp},
};

const MAX_QUERIES: usize = 65_536;
mod clip;
mod extend;
mod normal_extend;
pub use normal_extend::{
    try_extend_curve_normally_on_surface, try_extend_curve_normally_on_surface_with_operation,
};
mod through_point;
pub use clip::{
    SurfaceCurveOffsetGapStyle, SurfaceCurveOffsetOptions, SurfaceCurveOffsetPieces,
    try_offset_curves_on_surface, try_offset_curves_on_surface_with_options,
    try_offset_curves_on_surface_with_solver,
};
pub use extend::{
    SurfaceCurveExtension, try_extend_curve_on_surface, try_extend_curve_on_surface_with_seams,
    try_extend_curve_on_surface_with_solver,
};
pub use through_point::{
    SurfaceCurveOffsetThroughPoint, surface_curve_offset_distance_through_point,
};

/// A fitted geodesic offset and diagnostics. Source admission has a continuous
/// surface-image bound; integration and locus fitting retain numerical/sample
/// estimates rather than a continuous certificate for the offset itself.
#[derive(Clone, Debug)]
pub struct SurfaceCurveOffset {
    pub curve: NurbsCurve,
    pub source_deviation_bound: Real,
    pub geodesic_queries: usize,
    pub integration_steps: usize,
    pub maximum_integration_estimate: Real,
}

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidSurfaceCurveOffset { context }
}

fn preserve_spans(old: &[Real], new: &[Real], degree: usize) -> Result<(), GeometryError> {
    if old[degree..old.len() - degree]
        .windows(2)
        .zip(new[degree..new.len() - degree].windows(2))
        .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
    {
        return Err(invalid("parameter normalization collapsed a knot span"));
    }
    Ok(())
}

/// Offset a smooth curve lying on an untrimmed, regular NURBS surface.
/// Positive distance is left of the source tangent in the surface orientation.
/// Source and surface are immutable. Natural boundaries, poles, source kinks,
/// discontinuities and exhausted numerical work return errors, never a partial
/// curve. The plural producer clips to natural boundaries; trimmed faces,
/// corner joining and cut-locus removal need further qualification.
pub fn try_offset_curve_on_surface(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
) -> Result<SurfaceCurveOffset, GeometryError> {
    offset_with_solver(surface, source, distance, tolerance, None)
}

/// Use an existing homogeneous interpolation producer while retaining surface
/// admission, geodesic integration and fit validation in the shared layer.
pub fn try_offset_curve_on_surface_with_solver(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    solver: MorphInterpolationSolver,
) -> Result<SurfaceCurveOffset, GeometryError> {
    offset_with_solver(surface, source, distance, tolerance, Some(solver))
}

fn offset_with_solver(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<SurfaceCurveOffset, GeometryError> {
    let (surface, source, numerical) =
        normalized_sources(original_surface, original_source, distance, tolerance)?;
    if distance == 0. {
        let (_, source_deviation_bound) =
            surface.try_pullback_curve_certified_with_bound(&source, None, numerical)?;
        return Ok(SurfaceCurveOffset {
            curve: original_source.clone(),
            source_deviation_bound,
            geodesic_queries: 0,
            integration_steps: 0,
            maximum_integration_estimate: 0.,
        });
    }
    let mut sampler = OffsetSampler::new(&surface, &source, distance, numerical)?;
    let fitted = fit_complete(&mut sampler, tolerance, solver)?;
    Ok(SurfaceCurveOffset {
        curve: restore_domain(&fitted, original_source, tolerance)?,
        source_deviation_bound: sampler.source_deviation_bound,
        geodesic_queries: sampler.diagnostics.queries,
        integration_steps: sampler.diagnostics.steps,
        maximum_integration_estimate: sampler.diagnostics.estimate,
    })
}

fn normalized_sources(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    distance: Real,
    tolerance: Tolerance,
) -> Result<(NurbsSurface, NurbsCurve, Tolerance), GeometryError> {
    if !distance.is_finite() {
        return Err(invalid("offset distance must be finite"));
    }
    let surface = original_surface.try_reparameterized(0. ..=1., 0. ..=1.)?;
    preserve_spans(
        original_surface.knots_u(),
        surface.knots_u(),
        surface.degree_u(),
    )?;
    preserve_spans(
        original_surface.knots_v(),
        surface.knots_v(),
        surface.degree_v(),
    )?;
    let source = original_source.try_reparameterized(0. ..=1.)?;
    preserve_spans(original_source.knots(), source.knots(), source.degree())?;
    let numerical = Tolerance::try_new(
        tolerance.absolute() * 0.05,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    Ok((surface, source, numerical))
}

#[derive(Clone, Copy, Default)]
struct Diagnostics {
    queries: usize,
    steps: usize,
    estimate: Real,
}
impl Diagnostics {
    fn include(&mut self, other: Self) {
        self.queries += other.queries;
        self.steps += other.steps;
        self.estimate = self.estimate.max(other.estimate);
    }
}

struct OffsetSampler<'a> {
    surface: &'a NurbsSurface,
    source: &'a NurbsCurve,
    uv: NurbsCurve2,
    prepared: PreparedSurfaceGeodesic<'a>,
    distance: Real,
    numerical: Tolerance,
    source_deviation_bound: Real,
    diagnostics: Diagnostics,
}
impl<'a> OffsetSampler<'a> {
    fn new(
        surface: &'a NurbsSurface,
        source: &'a NurbsCurve,
        distance: Real,
        numerical: Tolerance,
    ) -> Result<Self, GeometryError> {
        let (uv, source_deviation_bound) =
            surface.try_pullback_curve_certified_with_bound(source, None, numerical)?;
        Ok(Self {
            surface,
            source,
            uv,
            prepared: PreparedSurfaceGeodesic::new(surface)?,
            distance,
            numerical,
            source_deviation_bound,
            diagnostics: Diagnostics::default(),
        })
    }
    fn sample(
        &mut self,
        t: Real,
        side: ParameterSide,
    ) -> Result<SurfaceGeodesicPoint, GeometryError> {
        if self.diagnostics.queries >= MAX_QUERIES {
            return Err(GeometryError::SurfaceCurveOffsetWorkLimit {
                maximum: MAX_QUERIES,
            });
        }
        self.diagnostics.queries += 1;
        let uv = self.uv.evaluate_on_side(t, side)?;
        let normal = self.surface.normal_at(uv.x(), uv.y())?;
        let tangent = CurveRef::NurbsCurve(self.source)
            .evaluate_with_tangent_on_side(t, side)?
            .tangent();
        let direction = normal
            .as_vector()
            .cross(tangent.as_vector())?
            .normalized_nonzero()?;
        let shot = self
            .prepared
            .point(uv, direction, self.distance, self.numerical)?;
        self.diagnostics.steps += shot.accepted_steps;
        self.diagnostics.estimate = self.diagnostics.estimate.max(shot.estimated_error);
        Ok(shot)
    }
}

fn fit_complete(
    sampler: &mut OffsetSampler<'_>,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<NurbsCurve, GeometryError> {
    let source = sampler.source;
    let uv = &sampler.uv;
    // The offset uses the first derivative of the source, so its guaranteed
    // continuity is one order lower. Retain pullback breaks as well.
    let mut breaks = seed_breaks(
        source.degree().saturating_sub(1),
        source.knots(),
        source.domain(),
    );
    breaks.extend(seed_breaks(
        uv.degree().saturating_sub(1),
        uv.knots(),
        uv.domain(),
    ));
    breaks.sort_by(|a, b| a.parameter.total_cmp(&b.parameter));
    breaks.dedup_by(|a, b| {
        if a.parameter == b.parameter {
            b.multiplicity = a.multiplicity.max(b.multiplicity);
            true
        } else {
            false
        }
    });
    let closed = source.evaluate(0.)? == source.evaluate(1.)?;
    let derivatives = if !closed && sampler.prepared.is_affine() {
        // Lift the admitted UV curve through the exact affine chart. This
        // retains the projected tangent even when an admitted spatial source
        // lies slightly off the surface within model tolerance.
        let controls = sampler
            .uv
            .control_points()
            .iter()
            .map(|c| {
                crate::WeightedPoint3::try_new(
                    sampler
                        .surface
                        .evaluate_extended(c.point().x(), c.point().y())?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let projected = NurbsCurve::try_new_rational(
            sampler.uv.degree(),
            controls,
            sampler.uv.knots().to_vec(),
        )?;
        let mut derivatives = [crate::Vector3::try_new(0., 0., 0.)?; 2];
        for (index, t) in [0., 1.].into_iter().enumerate() {
            let sample = CurveRef::NurbsCurve(&projected).evaluate_with_tangent(t)?;
            let uv = sampler.uv.evaluate(t)?;
            let left = sampler
                .surface
                .normal_at(uv.x(), uv.y())?
                .as_vector()
                .cross(sample.tangent().as_vector())?
                .normalized_nonzero()?;
            let curvature = CurveRef::NurbsCurve(&projected)
                .curvature_vector(t)?
                .dot(left.as_vector())?;
            derivatives[index] = projected
                .derivative_at(t)?
                .scaled((-sampler.distance).mul_add(curvature, 1.))?;
        }
        Some(derivatives)
    } else if !closed {
        // A normal geodesic family has a transverse Jacobi field. Its
        // direction is perpendicular to the arriving travel tangent within
        // the destination tangent plane. Retain that direction exactly: a
        // small free-fit endpoint tangent error grows over a long subsequent
        // normal section. Only the signed speed uses a one-sided estimate,
        // so focal-point reversals are retained rather than forced positive.
        let mut derivatives = [crate::Vector3::try_new(0., 0., 0.)?; 2];
        for (i, t, side) in [(0, 0., ParameterSide::Right), (1, 1., ParameterSide::Left)] {
            let span = source
                .knots()
                .iter()
                .copied()
                .filter(|k| if i == 0 { *k > 0. } else { *k < 1. })
                .map(|k| (k - t).abs())
                .fold(1., Real::min);
            let h = (span * 0.1).min(1e-4);
            let sign = if i == 0 { 1. } else { -1. };
            if h == 0. || t + sign * h == t {
                return Err(invalid(
                    "offset endpoint derivative has no representable interval",
                ));
            }
            let shot = sampler.sample(t, side)?;
            let normal = sampler.surface.normal_at(shot.uv.x(), shot.uv.y())?;
            let direction = shot
                .tangent
                .as_vector()
                .cross(normal.as_vector())?
                .normalized_nonzero()?
                .as_vector()
                .scaled(sampler.distance.signum())?;
            let p1 = sampler.sample(t + sign * h, side)?.point;
            let p2 = sampler.sample(t + sign * 2. * h, side)?.point;
            let a = shot.point.vector_to(p1)?.dot(direction)?;
            let b = shot.point.vector_to(p2)?.dot(direction)?;
            derivatives[i] = direction.scaled(sign * (4. * a - b) / (2. * h))?;
        }
        Some(derivatives)
    } else {
        None
    };
    let numerical = sampler.numerical;
    let mut evaluate = |t, side| sampler.sample(t, side).map(|p| p.point);
    let count = breaks.len();
    for knot in &mut breaks[1..count - 1] {
        let left = evaluate(knot.parameter, ParameterSide::Left)?;
        let right = evaluate(knot.parameter, ParameterSide::Right)?;
        if left.distance_to(right)? > numerical.absolute() {
            return Err(invalid(
                "source offset is discontinuous at an interior knot",
            ));
        }
        knot.multiplicity = knot.multiplicity.min(3);
    }
    let first = evaluate(0., ParameterSide::Right)?;
    let last = evaluate(1., ParameterSide::Left)?;
    if closed && first.distance_to(last)? > numerical.absolute() {
        return Err(invalid("source offset is discontinuous at the closed seam"));
    }
    let fit_tolerance = Tolerance::try_new(
        tolerance.absolute() * 0.5,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let fitted = fit_parameterized_curve_with_derivatives(
        source,
        breaks,
        fit_tolerance,
        MAX_CURVE_FIT_CONTROL_POINTS,
        solver,
        derivatives,
        |t, side| {
            if t == 0. || closed && t == 1. {
                Ok(first)
            } else if t == 1. {
                Ok(last)
            } else {
                evaluate(t, side)
            }
        },
    )
    .map_err(|e| match e {
        GeometryError::CurveMorphDidNotConverge {
            tolerance,
            deviation,
            maximum,
        } => GeometryError::SurfaceCurveOffsetDidNotConverge {
            tolerance,
            deviation,
            maximum,
        },
        GeometryError::TooManyMorphCurveControlPoints { maximum } => {
            GeometryError::SurfaceCurveOffsetDidNotConverge {
                tolerance: fit_tolerance.absolute(),
                deviation: Real::INFINITY,
                maximum,
            }
        }
        e => e,
    })?;
    Ok(fitted)
}

fn restore_domain(
    fitted: &NurbsCurve,
    original_source: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let parameter = |t: Real| {
        if t <= 1. {
            original_source.parameter_at(t)
        } else {
            crate::parameter::shifted_parameter(
                original_source.parameter_at(t - 1.)?,
                &original_source.domain(),
            )
        }
    };
    let domain = fitted.domain();
    let restored =
        fitted.try_reparameterized(parameter(*domain.start())?..=parameter(*domain.end())?)?;
    preserve_spans(fitted.knots(), restored.knots(), fitted.degree())?;
    // Knot restoration and native parameter encoding must not consume the fit
    // allowance, especially in a narrow chart translated far from zero.
    for (a, b) in fitted.spans() {
        for fraction in error_fractions(16) {
            let t = stable_lerp(a, b, fraction)?;
            let fraction = crate::nurbs::interval_fraction(t, *domain.start(), *domain.end())?;
            let native = restored.parameter_at(fraction)?;
            if fitted
                .evaluate(t)?
                .distance_to(restored.evaluate(native)?)?
                > tolerance.absolute() * 0.1
            {
                return Err(invalid(
                    "native curve parameters cannot represent the fitted locus accurately",
                ));
            }
        }
    }
    Ok(restored)
}

#[cfg(test)]
mod tests;
