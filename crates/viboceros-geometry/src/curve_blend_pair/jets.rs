//! Interpolate oriented endpoint jets with independent shape controls.
use super::{CurveBlendContinuity, CurveBlendOptions, continuity_control_count};
use crate::{
    EndpointInterpolation, GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance,
    UnitVector3, Vector3,
};

/// An endpoint position, the tangent in the blend's direction, and its geometric
/// curvature vector. Position-only endpoints may omit the tangent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBlendJet {
    position: Point3,
    tangent: Option<UnitVector3>,
    curvature: Vector3,
}
impl CurveBlendJet {
    pub fn new(position: Point3, tangent: Option<UnitVector3>, curvature: Vector3) -> Self {
        Self {
            position,
            tangent,
            curvature,
        }
    }
}

pub fn try_blend_curve_from_jets(
    jets: [CurveBlendJet; 2],
    options: CurveBlendOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_blend_curve_from_jets_with_interpolation(
        jets,
        options,
        tolerance,
        &mut crate::try_interpolate_endpoints,
    )
}

/// The same handle/default and homogeneous interpolation path used by Blend,
/// accepting already oriented jets for movable contacts and flipped tangents.
pub fn try_blend_curve_from_jets_with_interpolation<E: From<GeometryError>>(
    jets: [CurveBlendJet; 2],
    options: CurveBlendOptions,
    tolerance: Tolerance,
    interpolate: &mut impl FnMut(&EndpointInterpolation<'_>) -> Result<Vec<[Real; 4]>, E>,
) -> Result<NurbsCurve, E> {
    let [start, end] = jets.map(|j| j.position);
    let endpoint_specific = options.endpoint_specific;
    let chord = start.vector_to(end)?;
    let chord_length = chord.length()?;
    if chord_length <= tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "curve blend endpoints",
        }
        .into());
    }
    let default_handle = |continuity| match continuity {
        CurveBlendContinuity::Position => chord_length / 3.0,
        CurveBlendContinuity::Tangency => chord_length,
        CurveBlendContinuity::Curvature => chord_length * 0.4,
    };
    if options
        .bulges
        .iter()
        .any(|bulge| !bulge.is_finite() || *bulge <= 0.0)
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend bulge",
        }
        .into());
    }
    let mut first_handle =
        options.handles[0].unwrap_or_else(|| default_handle(options.continuity[0]));
    let mut second_handle =
        options.handles[1].unwrap_or_else(|| default_handle(options.continuity[1]));
    if !first_handle.is_finite()
        || !second_handle.is_finite()
        || first_handle <= 0.0
        || second_handle <= 0.0
    {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        }
        .into());
    }
    if options.continuity == [CurveBlendContinuity::Position; 2] {
        let domain_end = if endpoint_specific { 1.0 } else { chord_length };
        return NurbsCurve::try_new(1, vec![start, end], vec![0.0, 0.0, domain_end, domain_end])
            .map_err(E::from);
    }
    let degree = continuity_control_count(options.continuity[0])
        + continuity_control_count(options.continuity[1])
        - 1;
    for (jet, continuity) in jets.iter().zip(options.continuity) {
        if continuity != CurveBlendContinuity::Position && jet.tangent.is_none() {
            return Err(GeometryError::Degenerate {
                context: "constrained curve blend tangent",
            }
            .into());
        }
    }
    let [first_direction, second_direction] = jets.map(|j| j.tangent);
    if endpoint_specific
        && let (Some(first_direction), Some(second_direction)) = (first_direction, second_direction)
    {
        let chord_direction = chord.normalized_nonzero()?.as_vector();
        // The endpoint-specific Rhino blend uses the absolute chord projections
        // on both source tangent lines. The resulting endpoint speeds are
        // independent of the blend degree; divide by degree for Bézier handles.
        let a = chord_direction.dot(first_direction.as_vector())?.abs();
        let b = chord_direction.dot(second_direction.as_vector())?.abs();
        let speed_scale = 2.0 / degree as Real * chord_length;
        if options.handles[0].is_none() {
            first_handle = speed_scale * (1.4 - a + (a - b) * (0.3 + 0.5 * b));
        }
        if options.handles[1].is_none() {
            second_handle = speed_scale * (1.4 - b + (b - a) * (0.3 + 0.5 * a));
        }
    }
    if options.handles[0].is_none() {
        first_handle *= options.bulges[0];
    }
    if options.handles[1].is_none() {
        second_handle *= options.bulges[1];
    }
    if !first_handle.is_finite() || !second_handle.is_finite() {
        return Err(GeometryError::Degenerate {
            context: "curve blend handle length",
        }
        .into());
    }
    let orders = options
        .continuity
        .map(|g| (continuity_control_count(g) - 1) as u8);
    let mut targets = vec![[0.; 4]; degree + 1];
    targets[0] = [0., 0., 0., 1.];
    let xyz = chord.to_array();
    targets[degree] = [xyz[0], xyz[1], xyz[2], 1.];
    for (side, (jet, handle)) in jets.iter().zip([first_handle, second_handle]).enumerate() {
        let direction = jet.tangent;
        if orders[side] == 0 {
            continue;
        }
        let direction = direction.expect("constrained blend end has a tangent");
        let derivative = direction
            .as_vector()
            .scaled(handle)?
            .scaled(degree as Real)?
            .to_array();
        let row = if side == 0 { 1 } else { degree - 1 };
        targets[row] = [derivative[0], derivative[1], derivative[2], 0.];
        if orders[side] == 2 {
            let speed = degree as Real * handle;
            let derivative = jet.curvature.to_array().map(|c| c * speed * speed);
            let row = if side == 0 { 2 } else { degree - 2 };
            targets[row] = [derivative[0], derivative[1], derivative[2], 0.];
        }
    }
    let input = EndpointInterpolation {
        orders,
        column_count: 1,
        targets: &targets,
    };
    input.validate()?;
    let net = interpolate(&input)?;
    if net.len() != degree + 1
        || net
            .iter()
            .any(|h| h.iter().any(|v| !v.is_finite()) || (h[3] - 1.).abs() > 1e-12)
    {
        return Err(GeometryError::InvalidEndpointInterpolation {
            context: "curve backend returned invalid polynomial controls",
        }
        .into());
    }
    let controls = net
        .iter()
        .map(|h| Point3::try_new(h[0], h[1], h[2]))
        .collect::<Result<Vec<_>, _>>()?;
    let knots = std::iter::repeat_n(0., degree + 1)
        .chain(std::iter::repeat_n(1., degree + 1))
        .collect();
    let local = NurbsCurve::try_new(degree, controls, knots)?;
    for (end, &end_order) in orders.iter().enumerate() {
        let (point, first, second) = local.evaluate_with_second_derivative_on_side(
            end as Real,
            if end == 0 {
                ParameterSide::Right
            } else {
                ParameterSide::Left
            },
        )?;
        let actual = [point.to_array(), first.to_array(), second.to_array()];
        for order in 0..=usize::from(end_order) {
            let row = if end == 0 { order } else { degree - order };
            for (k, actual) in actual[order].into_iter().enumerate() {
                let target = targets[row][k];
                if (actual - target).abs() > 1e-9 * actual.abs().max(target.abs()).max(1.) {
                    return Err(GeometryError::InvalidEndpointInterpolation {
                        context: "curve backend did not preserve the requested endpoint jets",
                    }
                    .into());
                }
            }
        }
    }
    let mut controls = local
        .control_points()
        .iter()
        .map(|h| {
            let p = h.point();
            Point3::try_new(start.x() + p.x(), start.y() + p.y(), start.z() + p.z())
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Retain the original endpoint bits after the common-origin solve.
    controls[0] = start;
    controls[degree] = end;
    let mut knots = vec![0.0; degree + 1];
    knots.extend(vec![1.0; degree + 1]);
    let blend = NurbsCurve::try_new(degree, controls, knots)?;
    if endpoint_specific {
        Ok(blend)
    } else {
        blend
            .try_reparameterized(0.0..=blend.length(tolerance)?)
            .map_err(E::from)
    }
}
