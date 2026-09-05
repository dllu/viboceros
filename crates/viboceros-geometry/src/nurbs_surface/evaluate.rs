//! Native-parameter surface evaluation and rational differential geometry.

use super::{NurbsSurface, checked_span, extended_span};
use crate::nurbs::project_homogeneous;
use crate::{GeometryError, ParameterSide, Point3, Real, Vector3, require_finite};
mod tensor;
use tensor::{
    derivative_controls_u, derivative_controls_v, evaluate_tensor_product, project_derivative,
};

#[cfg(test)]
mod tests;

/// A native-parameter surface point and all partial derivatives through order
/// two. The mixed derivative is d²S/(du dv), without factorial scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceJet2 {
    pub point: Point3,
    pub derivative_u: Vector3,
    pub derivative_v: Vector3,
    pub derivative_uu: Vector3,
    pub derivative_uv: Vector3,
    pub derivative_vv: Vector3,
}

impl NurbsSurface {
    pub fn evaluate(&self, u: Real, v: Real) -> Result<Point3, GeometryError> {
        self.evaluate_on_sides(u, v, ParameterSide::Right, ParameterSide::Right)
    }

    /// Exact one-sided point limit at the supplied U/V parameters. At an outer
    /// domain endpoint either side chooses the only available interior span.
    pub fn evaluate_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<Point3, GeometryError> {
        Ok(self.evaluate_jet([u, v], [side_u, side_v], false, 0)?.point)
    }

    /// Rational continuation of the nearest nonempty boundary knot span.
    pub fn evaluate_extended(&self, u: Real, v: Real) -> Result<Point3, GeometryError> {
        Ok(self
            .evaluate_jet([u, v], [ParameterSide::Right; 2], true, 0)?
            .point)
    }

    pub fn evaluate_with_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        self.evaluate_with_derivatives_on_sides(u, v, ParameterSide::Right, ParameterSide::Right)
    }

    pub fn evaluate_with_derivatives_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let jet = self.evaluate_jet([u, v], [side_u, side_v], false, 1)?;
        Ok((jet.point, jet.derivative_u, jet.derivative_v))
    }

    pub fn evaluate_extended_with_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let jet = self.evaluate_jet([u, v], [ParameterSide::Right; 2], true, 1)?;
        Ok((jet.point, jet.derivative_u, jet.derivative_v))
    }

    pub fn evaluate_with_second_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<SurfaceJet2, GeometryError> {
        self.evaluate_with_second_derivatives_on_sides(
            u,
            v,
            ParameterSide::Right,
            ParameterSide::Right,
        )
    }

    pub fn evaluate_with_second_derivatives_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<SurfaceJet2, GeometryError> {
        self.evaluate_jet([u, v], [side_u, side_v], false, 2)
    }

    pub fn evaluate_extended_with_second_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<SurfaceJet2, GeometryError> {
        self.evaluate_jet([u, v], [ParameterSide::Right; 2], true, 2)
    }

    fn evaluate_jet(
        &self,
        parameters: [Real; 2],
        sides: [ParameterSide; 2],
        extended: bool,
        order: u8,
    ) -> Result<SurfaceJet2, GeometryError> {
        let degrees = [self.degree_u, self.degree_v];
        let counts = [self.control_point_count_u, self.control_point_count_v];
        let knots = [&self.knots_u[..], &self.knots_v[..]];
        let mut spans = [0; 2];
        for axis in 0..2 {
            let t = parameters[axis];
            let k = knots[axis];
            spans[axis] = if extended {
                extended_span(degrees[axis], counts[axis], k, t)?
            } else {
                checked_span(degrees[axis], counts[axis], k, t)?
            };
            if sides[axis] == ParameterSide::Left && t > k[degrees[axis]] && t < k[counts[axis]] {
                spans[axis] = k.partition_point(|value| *value < t) - 1;
            }
        }
        let (origin, controls) = self.evaluation_controls(spans, true)?;
        let result = self.jet_at_spans(parameters, spans, origin, &controls, order);
        if matches!(result, Err(GeometryError::NonFinite { .. })) && origin.to_array() != [0.0; 3] {
            // Signed surfaces can leave their control hull. A local offset can
            // overflow while the final world-space point is representable.
            let (origin, controls) = self.evaluation_controls(spans, false)?;
            self.jet_at_spans(parameters, spans, origin, &controls, order)
        } else {
            result
        }
    }

    fn jet_at_spans(
        &self,
        [u, v]: [Real; 2],
        [span_u, span_v]: [usize; 2],
        origin: Point3,
        active: &[[Real; 4]],
        order: u8,
    ) -> Result<SurfaceJet2, GeometryError> {
        let tensor = |net: &[[Real; 4]], du: usize, dv: usize| {
            evaluate_tensor_product(
                net,
                self.degree_u + 1 - du,
                &self.knots_u[du..self.knots_u.len() - du],
                self.degree_u - du,
                span_u - du,
                u,
                &self.knots_v[dv..self.knots_v.len() - dv],
                self.degree_v - dv,
                span_v - dv,
                v,
            )
        };
        let h = tensor(active, 0, 0)?;
        let local = project_homogeneous(h)?;
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let mut jet = SurfaceJet2 {
            point: local,
            derivative_u: zero,
            derivative_v: zero,
            derivative_uu: zero,
            derivative_uv: zero,
            derivative_vv: zero,
        };
        if order != 0 {
            let net_u =
                derivative_controls_u(active, self.degree_u, self.degree_v, span_u, &self.knots_u)?;
            let net_v =
                derivative_controls_v(active, self.degree_u, self.degree_v, span_v, &self.knots_v)?;
            let h_u = tensor(&net_u, 1, 0)?;
            let h_v = tensor(&net_v, 0, 1)?;
            jet.derivative_u = project_derivative(local, h, h_u)?;
            jet.derivative_v = project_derivative(local, h, h_v)?;
            if order == 2 {
                let h_uu = if self.degree_u > 1 {
                    let net = derivative_controls_u(
                        &net_u,
                        self.degree_u - 1,
                        self.degree_v,
                        span_u - 1,
                        &self.knots_u[1..self.knots_u.len() - 1],
                    )?;
                    tensor(&net, 2, 0)?
                } else {
                    [0.0; 4]
                };
                let h_vv = if self.degree_v > 1 {
                    let net = derivative_controls_v(
                        &net_v,
                        self.degree_u,
                        self.degree_v - 1,
                        span_v - 1,
                        &self.knots_v[1..self.knots_v.len() - 1],
                    )?;
                    tensor(&net, 0, 2)?
                } else {
                    [0.0; 4]
                };
                let net_uv = derivative_controls_v(
                    &net_u,
                    self.degree_u - 1,
                    self.degree_v,
                    span_v,
                    &self.knots_v,
                )?;
                let h_uv = tensor(&net_uv, 1, 1)?;
                let project_second =
                    |second: [Real; 4], a: Vector3, weight_a: Real, b: Vector3, weight_b: Real| {
                        let p = local.to_array();
                        let a = a.to_array();
                        let b = b.to_array();
                        Vector3::try_from(std::array::from_fn(|i| {
                            let numerator = (-p[i]).mul_add(second[3], second[i]);
                            let numerator = (-a[i]).mul_add(weight_a, numerator);
                            (-b[i]).mul_add(weight_b, numerator) / h[3]
                        }))
                    };
                // Degree-one homogeneous pure second partials vanish; their
                // rational Euclidean counterparts generally do not.
                jet.derivative_uu =
                    project_second(h_uu, jet.derivative_u, h_u[3], jet.derivative_u, h_u[3])?;
                jet.derivative_uv =
                    project_second(h_uv, jet.derivative_u, h_v[3], jet.derivative_v, h_u[3])?;
                jet.derivative_vv =
                    project_second(h_vv, jet.derivative_v, h_v[3], jet.derivative_v, h_v[3])?;
            }
        }
        jet.point = if let (Some(i), Some(j)) = (
            interpolated_control(&self.knots_u, self.degree_u, span_u, u),
            interpolated_control(&self.knots_v, self.degree_v, span_v, v),
        ) {
            self.control_points[self.control_index(i, j)].point()
        } else {
            Point3::try_new(
                local.x() + origin.x(),
                local.y() + origin.y(),
                local.z() + origin.z(),
            )?
        };
        Ok(jet)
    }

    fn evaluation_controls(
        &self,
        [span_u, span_v]: [usize; 2],
        center: bool,
    ) -> Result<(Point3, Vec<[Real; 4]>), GeometryError> {
        let first_u = span_u - self.degree_u;
        let first_v = span_v - self.degree_v;
        let candidate = self.control_points[self.control_index(first_u, first_v)].point();
        let active = || {
            (first_v..=span_v).flat_map(|v| {
                self.control_points[self.control_index(first_u, v)..=self.control_index(span_u, v)]
                    .iter()
            })
        };
        let mut can_center = center;
        let mut weight_scale: Real = 0.0;
        for control in active() {
            weight_scale = weight_scale.max(control.weight().abs());
            can_center &= control
                .point()
                .to_array()
                .into_iter()
                .zip(candidate.to_array())
                .all(|(a, b)| (a - b).is_finite());
        }
        let origin = if can_center {
            candidate
        } else {
            Point3::try_new(0.0, 0.0, 0.0)?
        };
        let controls = active()
            .map(|control| {
                let weight = control.weight() / weight_scale;
                let point = control.point();
                let h = [
                    (point.x() - origin.x()) * weight,
                    (point.y() - origin.y()) * weight,
                    (point.z() - origin.z()) * weight,
                    weight,
                ];
                require_finite(h, "local homogeneous NURBS surface control")?;
                Ok(h)
            })
            .collect::<Result<_, GeometryError>>()?;
        Ok((origin, controls))
    }
}

fn interpolated_control(knots: &[Real], degree: usize, span: usize, t: Real) -> Option<usize> {
    if t == knots[span] && knots[span + 1 - degree..=span].iter().all(|k| *k == t) {
        Some(span - degree)
    } else if t == knots[span + 1] && knots[span + 1..=span + degree].iter().all(|k| *k == t) {
        Some(span)
    } else {
        None
    }
}
