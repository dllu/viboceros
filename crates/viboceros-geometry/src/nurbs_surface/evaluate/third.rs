//! Third partials retain rational quotient terms even above the homogeneous degree.
use super::*;
mod exact;
#[cfg(test)]
mod tests;

/// Native-parameter partials through total order three, without factorial scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceJet3 {
    pub second: SurfaceJet2,
    pub derivative_uuu: Vector3,
    pub derivative_uuv: Vector3,
    pub derivative_uvv: Vector3,
    pub derivative_vvv: Vector3,
}
impl SurfaceJet3 {
    /// Su, Sv, Suu, Suv, Svv, Suuu, Suuv, Suvv, Svvv.
    pub fn partials(self) -> [Vector3; 9] {
        let s = self.second;
        [
            s.derivative_u,
            s.derivative_v,
            s.derivative_uu,
            s.derivative_uv,
            s.derivative_vv,
            self.derivative_uuu,
            self.derivative_uuv,
            self.derivative_uvv,
            self.derivative_vvv,
        ]
    }
}
fn index(u: usize, v: usize) -> usize {
    let order = u + v;
    order * (order + 1) / 2 + v
}
const BINOMIAL: [[Real; 4]; 4] = [
    [1., 0., 0., 0.],
    [1., 1., 0., 0.],
    [1., 2., 1., 0.],
    [1., 3., 3., 1.],
];

/// Construct each derivative net once. Missing homogeneous derivatives are zero;
/// the subsequent quotient recurrence still computes their Euclidean partials.
fn homogeneous_partials<T: Clone>(
    controls: &[T],
    degrees: [usize; 2],
    zero: impl Fn() -> T,
    differentiate: impl Fn(&[T], [usize; 2], usize) -> Result<Vec<T>, GeometryError>,
    evaluate: impl Fn(&[T], [usize; 2]) -> Result<T, GeometryError>,
) -> Result<[T; 10], GeometryError> {
    let mut result = std::array::from_fn(|_| zero());
    let mut u_net = controls.to_vec();
    for u in 0..=3.min(degrees[0]) {
        let mut net = u_net.clone();
        let last = (3 - u).min(degrees[1]);
        for v in 0..=last {
            result[index(u, v)] = evaluate(&net, [u, v])?;
            if v < last {
                net = differentiate(&net, [u, v], 1)?;
            }
        }
        if u < 3.min(degrees[0]) {
            u_net = differentiate(&u_net, [u, 0], 0)?;
        }
    }
    Ok(result)
}
fn jet(point: Point3, values: [[Real; 3]; 10]) -> Result<SurfaceJet3, GeometryError> {
    let mut vectors = [Vector3::try_new(0., 0., 0.)?; 9];
    for (v, p) in vectors.iter_mut().zip(&values[1..]) {
        *v = Vector3::try_from(*p)?;
    }
    let [u, v, uu, uv, vv, uuu, uuv, uvv, vvv] = vectors;
    Ok(SurfaceJet3 {
        second: SurfaceJet2 {
            point,
            derivative_u: u,
            derivative_v: v,
            derivative_uu: uu,
            derivative_uv: uv,
            derivative_vv: vv,
        },
        derivative_uuu: uuu,
        derivative_uuv: uuv,
        derivative_uvv: uvv,
        derivative_vvv: vvv,
    })
}
impl NurbsSurface {
    pub fn evaluate_with_third_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<SurfaceJet3, GeometryError> {
        self.evaluate_with_third_derivatives_on_sides(
            u,
            v,
            ParameterSide::Right,
            ParameterSide::Right,
        )
    }
    pub fn evaluate_with_third_derivatives_on_sides(
        &self,
        u: Real,
        v: Real,
        side_u: ParameterSide,
        side_v: ParameterSide,
    ) -> Result<SurfaceJet3, GeometryError> {
        self.third_jet([u, v], [side_u, side_v], false)
    }
    pub fn evaluate_extended_with_third_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<SurfaceJet3, GeometryError> {
        self.third_jet([u, v], [ParameterSide::Right; 2], true)
    }
    fn third_jet(
        &self,
        parameters: [Real; 2],
        sides: [ParameterSide; 2],
        extended: bool,
    ) -> Result<SurfaceJet3, GeometryError> {
        let (spans, continuation) = self.evaluation_spans(parameters, sides, extended)?;
        if continuation && self.constant_span_weight(spans).is_none() {
            return self.exact_third_jet(parameters, spans);
        }
        let controls = match self.evaluation_controls(spans) {
            Ok(controls) if !controls.needs_exact => controls,
            _ => return self.exact_third_jet(parameters, spans),
        };
        let value = if continuation {
            self.third_at_spans::<true>(parameters, spans, &controls)
        } else {
            self.third_at_spans::<false>(parameters, spans, &controls)
        };
        value.or_else(|_| self.exact_third_jet(parameters, spans))
    }
    fn third_at_spans<const POLYNOMIAL: bool>(
        &self,
        parameters: [Real; 2],
        spans: [usize; 2],
        controls: &EvaluationControls,
    ) -> Result<SurfaceJet3, GeometryError> {
        let degrees = [self.degree_u, self.degree_v];
        let knots = [&self.knots_u[..], &self.knots_v[..]];
        let h = homogeneous_partials(
            &controls.homogeneous,
            degrees,
            || [0.; 4],
            |net, order, axis| {
                let degree = [degrees[0] - order[0], degrees[1] - order[1]];
                let k = &knots[axis][order[axis]..knots[axis].len() - order[axis]];
                if axis == 0 {
                    derivative_controls_u(net, degree[0], degree[1], spans[0] - order[0], k)
                } else {
                    derivative_controls_v(net, degree[0], degree[1], spans[1] - order[1], k)
                }
            },
            |net, [u, v]| {
                evaluate_tensor_product::<POLYNOMIAL>(
                    net,
                    degrees[0] + 1 - u,
                    &knots[0][u..knots[0].len() - u],
                    degrees[0] - u,
                    spans[0] - u,
                    parameters[0],
                    &knots[1][v..knots[1].len() - v],
                    degrees[1] - v,
                    spans[1] - v,
                    parameters[1],
                )
            },
        )?;
        let local = project_homogeneous(h[0])?;
        let mut values = [[0.; 3]; 10];
        values[0] = local.to_array();
        for total in 1..=3 {
            for v in 0..=total {
                let u = total - v;
                let k = index(u, v);
                let mut numerator = [h[k][0], h[k][1], h[k][2]];
                for (a, binomial_u) in BINOMIAL[u].iter().enumerate().take(u + 1) {
                    for (b, binomial_v) in BINOMIAL[v].iter().enumerate().take(v + 1) {
                        if a + b == 0 {
                            continue;
                        }
                        let weight = binomial_u * binomial_v * h[index(a, b)][3];
                        for (c, n) in numerator.iter_mut().enumerate() {
                            *n = (-values[index(u - a, v - b)][c]).mul_add(weight, *n);
                        }
                    }
                }
                values[k] = numerator.map(|n| n / h[0][3]);
                require_finite(values[k], "third-order surface quotient")?;
            }
        }
        jet(
            self.restore_point(parameters, spans, controls.origin, local)?,
            values,
        )
    }
}
