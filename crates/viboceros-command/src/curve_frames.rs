//! Query-boundary frame policy for Rhino-compatible command placement.
//!
//! Smooth transport is the geometry kernel's adaptive minimum-twist solver.
//! At a queried corner only, project the carried X axis into the outgoing
//! normal plane before continuing. Unqueried corners retain shortest rotation.
use viboceros_geometry::{
    AffineTransform3, CurveRef, Frame3, FrameQueryRestart, FrameTransportOptions, GeometryError,
    ParameterSide, Point3, Real, Tolerance, UnitVector3, Vector3,
};

/// Perpendicular frames with outgoing seed and incoming later query limits.
/// The evaluation budget includes the additional boundary/curvature queries.
/// This is deliberately separate from density-invariant kernel transport.
pub fn perpendicular_frames(
    curve: CurveRef<'_>,
    parameters: &[Real],
    maximum_evaluations: usize,
) -> Result<Vec<Frame3>, GeometryError> {
    if maximum_evaluations == 0 {
        return Err(GeometryError::InvalidCurveFrameOptions);
    }
    let domain = curve.domain();
    if parameters.is_empty()
        || parameters
            .iter()
            .any(|t| !t.is_finite() || !domain.contains(t))
        || parameters.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(GeometryError::InvalidCurveFrameParameters);
    }
    let remaining = maximum_evaluations
        .checked_sub(parameters.len().saturating_mul(2).saturating_add(2))
        .filter(|n| *n >= parameters.len())
        .ok_or(GeometryError::CurveFrameResourceLimit {
            maximum: maximum_evaluations,
        })?;
    let tangent = curve
        .evaluate_with_tangent_on_side(parameters[0], ParameterSide::Right)?
        .tangent();
    let initial_x = seed(curve, parameters[0], tangent)?;
    let spans = span_parameters(curve, maximum_evaluations)?;
    let end = *parameters.last().unwrap();
    let mut reseed_next = false;
    curve.frames_with_query_restarts(
        parameters,
        Some(initial_x),
        ParameterSide::Right,
        FrameTransportOptions {
            side: ParameterSide::Left,
            maximum_evaluations: remaining,
            ..Default::default()
        },
        |t, incoming, current_tangent| {
            // A singular projection leaves the next interval without an axis.
            // Public SDK captures show its next returned query is freshly
            // seeded from that query's tangent, including on curved spans.
            let query_x = if reseed_next {
                reseed_next = false;
                Some(arbitrary_seed(incoming.z_axis())?)
            } else {
                None
            };
            if t == end {
                return Ok(FrameQueryRestart {
                    query_x,
                    outgoing_x: None,
                });
            }
            let after = spans.partition_point(|p| *p <= t);
            // Public OpenNURBS ON_TuneupEvaluationParameter uses this positive-
            // side span fraction. Only continuation changes; query origins and
            // their incoming axes retain the original parameter.
            let continued = if after > 0
                && after < spans.len()
                && t >= spans[after] - (spans[after] - spans[after - 1]) * 1e-10
            {
                spans[after]
            } else {
                t
            };
            let outgoing = curve
                .evaluate_with_tangent_on_side(continued, ParameterSide::Right)?
                .tangent();
            let x = match project(query_x.unwrap_or_else(|| incoming.x_axis()), outgoing) {
                Ok(x) => x,
                Err(_) => {
                    reseed_next = true;
                    arbitrary_seed(outgoing)?
                }
            };
            if outgoing == current_tangent {
                return Ok(FrameQueryRestart {
                    query_x,
                    outgoing_x: Some(x),
                });
            }
            Ok(FrameQueryRestart {
                query_x,
                outgoing_x: Some(
                    AffineTransform3::try_rotation_between(
                        outgoing,
                        current_tangent,
                        axis_tolerance(),
                    )?
                    .transform_vector(x.as_vector())?
                    .normalized_nonzero()?,
                ),
            })
        },
    )
}
fn span_parameters(curve: CurveRef<'_>, maximum: usize) -> Result<Vec<Real>, GeometryError> {
    fn collect(
        curve: CurveRef<'_>,
        out: &mut Vec<Real>,
        maximum: usize,
    ) -> Result<(), GeometryError> {
        match curve {
            CurveRef::NurbsCurve(c) => {
                for t in &c.knots()[c.degree()..=c.control_points().len()] {
                    if out.last() != Some(t) {
                        out.push(*t);
                    }
                }
            }
            CurveRef::Polyline(c) => out.extend_from_slice(c.parameters()),
            CurveRef::PolyCurve(c) => {
                for (i, s) in c.segments().iter().enumerate() {
                    let mut child = Vec::new();
                    collect(s.as_ref(), &mut child, maximum)?;
                    for t in child {
                        out.push(c.polycurve_parameter(i, t)?);
                    }
                }
            }
            _ => out.extend([*curve.domain().start(), *curve.domain().end()]),
        }
        if out.len() > maximum {
            return Err(GeometryError::CurveFrameResourceLimit { maximum });
        }
        Ok(())
    }
    let mut spans = Vec::new();
    collect(curve, &mut spans, maximum)?;
    spans.sort_by(Real::total_cmp);
    spans.dedup();
    Ok(spans)
}

fn axis_tolerance() -> Tolerance {
    Tolerance::try_new(Real::MIN_POSITIVE, 64. * Real::EPSILON, 64. * Real::EPSILON).unwrap()
}
fn project(x: UnitVector3, z: UnitVector3) -> Result<UnitVector3, GeometryError> {
    z.as_vector()
        .cross(x.as_vector())?
        .normalized_nonzero()?
        .as_vector()
        .cross(z.as_vector())?
        .normalized_nonzero()
}
fn seed(curve: CurveRef<'_>, t: Real, tangent: UnitVector3) -> Result<UnitVector3, GeometryError> {
    if let Ok(k) = curve.curvature_vector_on_side(t, ParameterSide::Right)
        && let Ok(k) = k.normalized_nonzero()
        && let Ok(x) = project(k, tangent)
    {
        return Ok(x);
    }
    arbitrary_seed(tangent)
}
fn arbitrary_seed(tangent: UnitVector3) -> Result<UnitVector3, GeometryError> {
    let coordinates = tangent.as_vector().to_array();
    for i in [2, 0, 1] {
        if coordinates[i].abs() <= Real::EPSILON.sqrt() {
            let axis = Vector3::try_from(std::array::from_fn(|j| if i == j { 1. } else { 0. }))?
                .normalized_nonzero()?;
            return project(axis, tangent);
        }
    }
    Frame3::try_from_unit_normal(Point3::try_new(0., 0., 0.)?, tangent).map(|f| f.x_axis())
}
