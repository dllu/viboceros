//! Natural UV continuation with the existing boundary extension and image proof.
use super::*;
use crate::{
    CurveExtensionSide, CurveExtensionStyle, Point2, Point3, WeightedPoint2, WeightedPoint3,
};

/// One continued spatial curve and the native times of its open chart breaks.
#[derive(Clone, Debug)]
pub struct SurfaceCurveExtension {
    pub curve: NurbsCurve,
    cuts: Vec<(Real, Real)>,
    chart: Option<(NurbsSurface, NurbsCurve2)>,
}
impl SurfaceCurveExtension {
    pub fn seam_parameters(&self) -> impl ExactSizeIterator<Item = Real> + '_ {
        self.cuts.iter().map(|p| p.0)
    }
    /// Keep the complete curve for subsequent geometry operations; emit its
    /// chart portions independently. Every trimmed representation is proved.
    pub fn parts(&self, tolerance: Tolerance) -> Result<Vec<NurbsCurve>, GeometryError> {
        if self.cuts.is_empty() {
            return Ok(vec![self.curve.clone()]);
        }
        let (surface, uv) = self
            .chart
            .as_ref()
            .ok_or_else(|| invalid("chart portions require an image reference"))?;
        let mut cuts = vec![(*self.curve.domain().start(), *uv.domain().start())];
        cuts.extend(&self.cuts);
        cuts.push((*self.curve.domain().end(), *uv.domain().end()));
        if cuts.len() > crate::MAX_CURVE_DIVISION_POINTS {
            return Err(GeometryError::TooManySurfacePullbackControlPoints {
                maximum: crate::MAX_CURVE_DIVISION_POINTS,
            });
        }
        cuts.windows(2)
            .map(|p| {
                let curve = self.curve.try_trimmed(p[0].0..=p[1].0)?;
                let image = uv.try_trimmed(p[0].1..=p[1].1)?;
                if surface
                    .parameter_curve_deviation_bound(&image, &curve, tolerance.absolute())?
                    .is_none()
                {
                    return Err(invalid("trimmed chart portion exceeded image tolerance"));
                }
                Ok(curve)
            })
            .collect()
    }
}

pub(super) fn extension(
    surface: NurbsSurface,
    uv: NurbsCurve2,
    curve: NurbsCurve,
    closed: [bool; 2],
) -> Result<SurfaceCurveExtension, GeometryError> {
    let mut cuts = Vec::new();
    for group in uv.knots().chunk_by(|a, b| a == b) {
        let t = group[0];
        if group.len() != uv.degree() + 1 || t <= *uv.domain().start() || t >= *uv.domain().end() {
            continue;
        }
        let a = uv.evaluate_on_side(t, ParameterSide::Left)?;
        let b = uv.evaluate_on_side(t, ParameterSide::Right)?;
        if !(closed[0] && (a.x() - b.x()).abs() > 0.5 || closed[1] && (a.y() - b.y()).abs() > 0.5) {
            continue;
        }
        let native = crate::remap_scalar(
            t,
            [*uv.domain().start(), *uv.domain().end()],
            [*curve.domain().start(), *curve.domain().end()],
        )?;
        let previous = cuts
            .last()
            .map_or(*curve.domain().start(), |p: &(Real, Real)| p.0);
        if native <= previous || native >= *curve.domain().end() {
            return Err(invalid(
                "chart event cannot be represented in the native curve domain",
            ));
        }
        cuts.push((native, t));
    }
    let chart = (!cuts.is_empty()).then_some((surface, uv));
    Ok(SurfaceCurveExtension { curve, cuts, chart })
}

/// Continue an open surface curve to the natural parameter rectangle.
///
/// The certified pullback supplies the endpoint spans to extrapolate. Each
/// direction stops at its first rectangle contact; periodic seams bound this
/// continuation too, so an open isoparametric arc can become a complete ring.
/// The complete spatial image is certified before returning. This is a UV
/// continuation policy, not geodesic continuation or a native parity claim.
/// Closed curves and endpoints already leaving the rectangle remain unchanged.
pub fn try_extend_curve_on_surface(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    side: CurveExtensionSide,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_extend_curve_on_surface_with_solver(
        original_surface,
        original_source,
        side,
        tolerance,
        None,
    )
}

/// Select a shared interpolation producer for smooth-image candidates;
/// source admission and complete image proofs stay in the geometry core.
pub fn try_extend_curve_on_surface_with_solver(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    side: CurveExtensionSide,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<NurbsCurve, GeometryError> {
    try_extend_curve_on_surface_with_seams(
        original_surface,
        original_source,
        side,
        tolerance,
        solver,
    )
    .map(|result| result.curve)
}

/// Continue and retain the certified UV chart events for independent output
/// portions, while preserving one complete spatial curve for later offsets.
pub fn try_extend_curve_on_surface_with_seams(
    original_surface: &NurbsSurface,
    original_source: &NurbsCurve,
    side: CurveExtensionSide,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<SurfaceCurveExtension, GeometryError> {
    if original_source.is_closed()? {
        return Ok(SurfaceCurveExtension {
            curve: original_source.clone(),
            cuts: Vec::new(),
            chart: None,
        });
    }
    let (surface, source, numerical) =
        normalized_sources(original_surface, original_source, 0., tolerance)?;
    // Offset fits can already spend part of the model allowance. Admit their
    // image within a quarter of that budget; use the tighter numerical budget
    // for the new pushup, leaving room for subsequent offset admission.
    let admission = Tolerance::try_new(
        tolerance.absolute() * 0.25,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let closed = [
        crate::surface_geodesic::chart_closes(&surface, 0)?,
        crate::surface_geodesic::chart_closes(&surface, 1)?,
    ];
    let uv = if closed.into_iter().any(|v| v) {
        let middle = source.evaluate(0.5)?;
        let (u, v) = surface.closest_parameters(middle, numerical)?;
        let middle = [u, v];
        let mut endpoints = [Point2::try_new(0., 0.)?; 2];
        let mut constrained = false;
        for (i, t) in [0., 1.].into_iter().enumerate() {
            let point = source.evaluate(t)?;
            let (u, v) = surface.closest_parameters(point, numerical)?;
            let mut q = [u, v];
            for axis in 0..2 {
                if !closed[axis] {
                    continue;
                }
                let mut seam = q;
                seam[axis] = if middle[axis] <= 0.5 { 0. } else { 1. };
                if surface.evaluate(seam[0], seam[1])?.distance_to(point)? <= admission.absolute() {
                    q = seam;
                    constrained = true;
                }
            }
            endpoints[i] = Point2::try_new(q[0], q[1])?;
        }
        if constrained {
            surface.try_pullback_curve_certified_with_endpoints(&source, endpoints, admission)?
        } else {
            surface.try_pullback_curve_certified(&source, admission)?
        }
    } else {
        surface.try_pullback_curve_certified(&source, admission)?
    };
    let parameter_tolerance = clip::uv_tolerance(&surface, [false; 2], tolerance)?;
    let mut lifted = NurbsCurve::try_new_rational(
        uv.degree(),
        uv.control_points()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                    p.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        uv.knots().to_vec(),
    )?;
    for at_start in [true, false] {
        if at_start && matches!(side, CurveExtensionSide::End)
            || !at_start && matches!(side, CurveExtensionSide::Start)
        {
            continue;
        }
        let endpoint = if at_start {
            *lifted.domain().start()
        } else {
            *lifted.domain().end()
        };
        let jet = lifted.evaluate_with_derivative(endpoint)?;
        let q = jet.0.to_array();
        let motion = jet.1.to_array().map(|x| if at_start { -x } else { x });
        if (0..2).any(|axis| {
            q[axis] <= parameter_tolerance.absolute() && motion[axis] < 0.
                || q[axis] >= 1. - parameter_tolerance.absolute() && motion[axis] > 0.
        }) {
            continue;
        }
        let mut boundaries = Vec::new();
        for axis in 0..2 {
            for value in [0., 1.] {
                // A path on a rectangle edge must reach the edge's corner,
                // rather than treating its whole collinear overlap as a hit.
                if lifted
                    .control_points()
                    .iter()
                    .all(|p| p.point().to_array()[axis] == value)
                {
                    continue;
                }
                let p = |other| {
                    Point3::try_from(std::array::from_fn(|i| {
                        if i == axis {
                            value
                        } else if i == 1 - axis {
                            other
                        } else {
                            0.
                        }
                    }))
                };
                boundaries.push(NurbsCurve::try_new(
                    1,
                    vec![p(0.)?, p(1.)?],
                    vec![0., 0., 1., 1.],
                )?);
            }
        }
        lifted = lifted.try_merged_to_curve_boundaries(
            if at_start {
                CurveExtensionSide::Start
            } else {
                CurveExtensionSide::End
            },
            CurveExtensionStyle::Smooth,
            &boundaries,
            parameter_tolerance,
        )?;
    }
    if lifted.domain() == source.domain() {
        return extension(surface, uv, original_source.clone(), closed);
    }
    let last = lifted.control_points().len() - 1;
    let controls = lifted
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut q = p.point().to_array();
            if i == 0 || i == last {
                for coordinate in &mut q[..2] {
                    if coordinate.abs() <= parameter_tolerance.absolute() {
                        *coordinate = 0.;
                    } else if (*coordinate - 1.).abs() <= parameter_tolerance.absolute() {
                        *coordinate = 1.;
                    }
                }
            }
            WeightedPoint2::try_new(Point2::try_new(q[0], q[1])?, p.weight())
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let uv = NurbsCurve2::try_new_rational(lifted.degree(), controls, lifted.knots().to_vec())?;
    let curve = smooth_image(&surface, &uv, numerical, solver)?;
    let native = [
        *original_source.domain().start(),
        *original_source.domain().end(),
    ];
    let a = crate::remap_scalar(*curve.domain().start(), [0., 1.], native)?;
    let b = crate::remap_scalar(*curve.domain().end(), [0., 1.], native)?;
    let restored = curve.try_reparameterized(a..=b)?;
    preserve_spans(curve.knots(), restored.knots(), curve.degree())?;
    // Re-encoding the native knot times can change normalized stations.
    // Prove the complete image again on the actual returned representation.
    if surface
        .parameter_curve_deviation_bound(&uv, &restored, numerical.absolute())?
        .is_none()
    {
        return Err(invalid(
            "extension parameter restoration exceeded tolerance",
        ));
    }
    extension(surface, uv, restored, closed)
}

pub(super) fn direction_gap(a: crate::Vector3, b: crate::Vector3) -> Result<Real, GeometryError> {
    let a = a.normalized_nonzero()?.as_vector().to_array();
    let b = b.normalized_nonzero()?.as_vector().to_array();
    Ok((0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<Real>().sqrt())
}

pub(super) fn smooth_image(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    tolerance: Tolerance,
    solver: Option<MorphInterpolationSolver>,
) -> Result<NurbsCurve, GeometryError> {
    let proposal = surface.try_pushup_curve_certified(uv, tolerance)?;
    let mut smooth = true;
    for t in proposal.full_order_knots() {
        let (_, a) = proposal.evaluate_with_derivative_on_side(t, ParameterSide::Left)?;
        let (_, b) = proposal.evaluate_with_derivative_on_side(t, ParameterSide::Right)?;
        if direction_gap(a, b)? > 1024. * Real::EPSILON {
            smooth = false;
            break;
        }
    }
    if smooth {
        return Ok(proposal);
    }
    // Independent certified cubic image segments can introduce small tangent
    // jumps. A surface offset differentiates its source: retain genuine input
    // breaks, but fit C1 joins where the UV image is smooth, then certify again.
    let lifted = NurbsCurve::try_new_rational(
        uv.degree(),
        uv.control_points()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                    p.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        uv.knots().to_vec(),
    )?
    .try_reparameterized(0. ..=1.)?;
    let image = |t, side| -> Result<(Point3, crate::Vector3), GeometryError> {
        let (q, d) = lifted.evaluate_with_derivative_on_side(t, side)?;
        let (p, u, v) =
            surface.evaluate_with_derivatives(q.x().clamp(0., 1.), q.y().clamp(0., 1.))?;
        let u = u.to_array();
        let v = v.to_array();
        Ok((
            p,
            crate::Vector3::try_from(std::array::from_fn(|i| u[i].mul_add(d.x(), v[i] * d.y())))?,
        ))
    };
    let mut breaks = seed_breaks(lifted.degree(), lifted.knots(), lifted.domain());
    let count = breaks.len();
    for knot in &mut breaks[1..count - 1] {
        let (a, da) = image(knot.parameter, ParameterSide::Left)?;
        let (b, db) = image(knot.parameter, ParameterSide::Right)?;
        if a.distance_to(b)? <= tolerance.absolute()
            && direction_gap(da, db)? <= tolerance.angular().min(1e-9)
        {
            knot.multiplicity = knot.multiplicity.min(2);
        }
    }
    let derivatives = Some([
        image(0., ParameterSide::Right)?.1,
        image(1., ParameterSide::Left)?.1,
    ]);
    let normalized = proposal.try_reparameterized(0. ..=1.)?;
    for fraction in [0.2, 0.02, 0.002] {
        let fitting = Tolerance::try_new(
            tolerance.absolute() * fraction,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let candidate = fit_parameterized_curve_with_derivatives(
            &normalized,
            breaks.clone(),
            fitting,
            MAX_CURVE_FIT_CONTROL_POINTS,
            solver,
            derivatives,
            |t, side| image(t, side).map(|v| v.0),
        )?
        .try_reparameterized(uv.domain())?;
        if surface
            .parameter_curve_deviation_bound(uv, &candidate, tolerance.absolute())?
            .is_some()
        {
            return Ok(candidate);
        }
    }
    Err(GeometryError::SurfacePushupDidNotConverge {
        tolerance: tolerance.absolute(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, UnitVector3};
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn cylinder(scale: Real) -> NurbsSurface {
        let frame = Frame3::try_from_unit_normal(
            p(0., 0., 0.),
            UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        )
        .unwrap();
        NurbsSurface::try_cylinder(frame, 3. * scale, 0., 6. * scale).unwrap()
    }
    #[test]
    fn generators_reach_both_boundaries_in_independent_native_charts() {
        for scale in [2_f64.powi(-100), 1., 2_f64.powi(100)] {
            let tolerance = Tolerance::try_new(scale * 1e-7, 1e-12, 1e-9).unwrap();
            for u in [0., 0.6] {
                let surface = cylinder(scale);
                let full = surface.isocurve_v(u).unwrap();
                let source = full
                    .try_trimmed(
                        full.parameter_at(0.25).unwrap()..=full.parameter_at(0.75).unwrap(),
                    )
                    .unwrap()
                    .try_reparameterized(-7. ..=11.)
                    .unwrap();
                let surface = surface
                    .try_reparameterized(
                        2_f64.powi(600)..=2_f64.powi(601),
                        -2_f64.powi(-600)..=2_f64.powi(-600),
                    )
                    .unwrap();
                let result = try_extend_curve_on_surface(
                    &surface,
                    &source,
                    CurveExtensionSide::Both,
                    tolerance,
                )
                .unwrap();
                assert!(
                    result
                        .evaluate(*result.domain().start())
                        .unwrap()
                        .distance_to(full.evaluate(*full.domain().start()).unwrap())
                        .unwrap()
                        < tolerance.absolute()
                );
                assert!(
                    result
                        .evaluate(*result.domain().end())
                        .unwrap()
                        .distance_to(full.evaluate(*full.domain().end()).unwrap())
                        .unwrap()
                        < tolerance.absolute()
                );
                assert!((*result.domain().start() + 16.).abs() < 1e-10);
                assert!((*result.domain().end() - 20.).abs() < 1e-10);
                for i in 0..=16 {
                    let t = source.parameter_at(i as Real / 16.).unwrap();
                    assert!(
                        result
                            .evaluate(t)
                            .unwrap()
                            .distance_to(source.evaluate(t).unwrap())
                            .unwrap()
                            < tolerance.absolute()
                    );
                }
            }
        }
    }
    #[test]
    fn partial_ring_continues_to_the_natural_seam_and_becomes_closed() {
        let surface = cylinder(1.);
        let source = surface
            .isocurve_u(3.)
            .unwrap()
            .try_trimmed(0.3..=1.3)
            .unwrap();
        for side in [
            CurveExtensionSide::Start,
            CurveExtensionSide::End,
            CurveExtensionSide::Both,
        ] {
            let curve =
                try_extend_curve_on_surface(&surface, &source, side, Tolerance::DEFAULT).unwrap();
            assert_eq!(
                curve.is_closed().unwrap(),
                matches!(side, CurveExtensionSide::Both)
            );
            let start = if matches!(side, CurveExtensionSide::End) {
                0.3
            } else {
                0.
            };
            let end = if matches!(side, CurveExtensionSide::Start) {
                1.3
            } else {
                std::f64::consts::TAU
            };
            assert!((*curve.domain().start() - start).abs() < 1e-10);
            assert!((*curve.domain().end() - end).abs() < 1e-10);
            for i in 0..=32 {
                let q = curve
                    .evaluate(curve.parameter_at(i as Real / 32.).unwrap())
                    .unwrap();
                assert!((q.x().hypot(q.y()) - 3.).abs() < 1e-10 && (q.z() - 3.).abs() < 1e-10);
            }
        }
    }
    #[test]
    fn certified_smooth_images_keep_continuous_tangents_across_fitted_spans() {
        let surface = cylinder(1.)
            .try_reparameterized(0. ..=1., 0. ..=1.)
            .unwrap();
        let uv = NurbsCurve2::try_new(
            2,
            vec![
                Point2::try_new(0.1, 0.2).unwrap(),
                Point2::try_new(0.2, 0.45).unwrap(),
                Point2::try_new(0.3, 0.8).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-9).unwrap();
        let curve = smooth_image(&surface, &uv, tolerance, None).unwrap();
        assert!(
            surface
                .parameter_curve_deviation_bound(&uv, &curve, tolerance.absolute())
                .unwrap()
                .is_some()
        );
        for (_, t) in curve.spans().filter(|(_, t)| *t < *curve.domain().end()) {
            let (_, a) = curve
                .evaluate_with_derivative_on_side(t, ParameterSide::Left)
                .unwrap();
            let (_, b) = curve
                .evaluate_with_derivative_on_side(t, ParameterSide::Right)
                .unwrap();
            assert!(direction_gap(a, b).unwrap() < 1e-10);
        }
    }
    #[test]
    fn nonlinear_uv_continuation_keeps_the_source_and_hits_the_first_corners() {
        let surface = NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
        ])
        .unwrap();
        let source = NurbsCurve::try_new(
            2,
            vec![p(0.25, 0.0625, 0.), p(0.5, 0.1875, 0.), p(0.75, 0.5625, 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let curve = try_extend_curve_on_surface(
            &surface,
            &source,
            CurveExtensionSide::Both,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            curve
                .evaluate(*curve.domain().start())
                .unwrap()
                .distance_to(p(0., 0., 0.))
                .unwrap()
                < 1e-10
        );
        assert!(
            curve
                .evaluate(*curve.domain().end())
                .unwrap()
                .distance_to(p(1., 1., 0.))
                .unwrap()
                < 1e-10
        );
        for i in 0..=32 {
            let t = source.parameter_at(i as Real / 32.).unwrap();
            assert!(
                curve
                    .evaluate(t)
                    .unwrap()
                    .distance_to(source.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
        let off = NurbsCurve::try_new(
            1,
            vec![p(0.25, 0.5, 1.), p(0.75, 0.5, 1.)],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert!(
            try_extend_curve_on_surface(
                &surface,
                &off,
                CurveExtensionSide::Both,
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}
