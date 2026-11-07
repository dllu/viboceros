//! Interpolate oriented endpoint jets with independent shape controls.
use super::{CurveBlendOptions, continuity_control_count};
use crate::{
    EndpointInterpolation, GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance,
    UnitVector3, Vector3,
};

mod normalized;
mod parameterization;
#[cfg(test)]
mod point_tests;
mod spacing;

/// An endpoint position, oriented tangent, geometric curvature, and optional
/// normalized third/fourth targets. Position-only ends may omit the tangent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBlendJet {
    position: Point3,
    tangent: Option<UnitVector3>,
    curvature: Vector3,
    higher: [Option<Vector3>; 2],
    parameterization: [Real; 3],
    spacing: Option<[Real; 3]>,
}
impl CurveBlendJet {
    pub fn new(position: Point3, tangent: Option<UnitVector3>, curvature: Vector3) -> Self {
        Self {
            position,
            tangent,
            curvature,
            higher: [None; 2],
            parameterization: [0.; 3],
            spacing: None,
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
    try_blend_curve_from_endpoint_jets_with_interpolation(
        jets,
        options.into(),
        tolerance,
        interpolate,
    )
}

/// Shape controls with independent derivative orders, from zero through four.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveBlendJetOptions {
    pub orders: [u8; 2],
    pub handles: [Option<Real>; 2],
    pub bulges: [Real; 2],
    pub endpoint_specific: bool,
}
impl From<CurveBlendOptions> for CurveBlendJetOptions {
    fn from(options: CurveBlendOptions) -> Self {
        Self {
            orders: options
                .continuity
                .map(|c| (continuity_control_count(c) - 1) as u8),
            handles: options.handles,
            bulges: options.bulges,
            endpoint_specific: options.endpoint_specific,
        }
    }
}

pub fn try_blend_curve_from_endpoint_jets(
    jets: [CurveBlendJet; 2],
    options: CurveBlendJetOptions,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    try_blend_curve_from_endpoint_jets_with_interpolation(
        jets,
        options,
        tolerance,
        &mut crate::try_interpolate_endpoints,
    )
}

pub fn try_blend_curve_from_endpoint_jets_with_interpolation<E: From<GeometryError>>(
    jets: [CurveBlendJet; 2],
    options: CurveBlendJetOptions,
    tolerance: Tolerance,
    interpolate: &mut impl FnMut(&EndpointInterpolation<'_>) -> Result<Vec<[Real; 4]>, E>,
) -> Result<NurbsCurve, E> {
    if options.orders.iter().any(|&o| o > 4) {
        return Err(GeometryError::InvalidEndpointInterpolation {
            context: "unsupported curve blend derivative order",
        }
        .into());
    }
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
    let default_handle = |order| match order {
        0 => chord_length / 3.,
        1 => chord_length,
        _ => chord_length * 0.4,
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
    let mut first_handle = options.handles[0].unwrap_or_else(|| default_handle(options.orders[0]));
    let mut second_handle = options.handles[1].unwrap_or_else(|| default_handle(options.orders[1]));
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
    if options.orders == [0; 2] {
        let domain_end = if endpoint_specific { 1.0 } else { chord_length };
        return NurbsCurve::try_new(1, vec![start, end], vec![0.0, 0.0, domain_end, domain_end])
            .map_err(E::from);
    }
    let degree = usize::from(options.orders[0]) + usize::from(options.orders[1]) + 1;
    for (jet, continuity) in jets.iter().zip(options.orders) {
        if continuity != 0 && jet.tangent.is_none() {
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
    // A lone curve direction blending to a positional point uses chord-length
    // endpoint speed, independent of the constrained order.
    if endpoint_specific && (first_direction.is_none() || second_direction.is_none()) {
        if first_direction.is_some() && options.handles[0].is_none() {
            first_handle = chord_length / degree as Real;
        }
        if second_direction.is_some() && options.handles[1].is_none() {
            second_handle = chord_length / degree as Real;
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
    let orders = options.orders;
    let mut targets = vec![[0.; 4]; degree + 1];
    targets[0] = [0., 0., 0., 1.];
    let xyz = chord.to_array();
    targets[degree] = [xyz[0], xyz[1], xyz[2], 1.];
    for (side, (jet, handle)) in jets.iter().zip([first_handle, second_handle]).enumerate() {
        if orders[side] == 0 {
            continue;
        }
        let derivatives = jet.parameterized_derivatives(handle, degree, orders[side], side == 1)?;
        for order in 1..=orders[side] {
            let derivative = derivatives[usize::from(order - 1)];
            let row = if side == 0 {
                usize::from(order)
            } else {
                degree - usize::from(order)
            };
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
        let mut actual = actual.to_vec();
        if end_order > 2 {
            let (_, derivatives) = local.evaluate_with_fourth_derivatives_on_side(
                end as Real,
                if end == 0 {
                    ParameterSide::Right
                } else {
                    ParameterSide::Left
                },
            )?;
            actual.extend([derivatives[2].to_array(), derivatives[3].to_array()]);
        }
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
