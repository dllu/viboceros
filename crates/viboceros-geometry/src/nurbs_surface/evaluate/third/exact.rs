//! Original-input rational recovery includes every quotient term before rounding.
use super::*;
use crate::nurbs::exact::{Direction, Rational, scalar};
use num_traits::Zero;
impl NurbsSurface {
    pub(super) fn exact_third_jet(
        &self,
        parameters: [Real; 2],
        spans: [usize; 2],
    ) -> Result<SurfaceJet3, GeometryError> {
        let degrees = [self.degree_u, self.degree_v];
        let base = [
            Direction {
                knots: &self.knots_u,
                degree: degrees[0],
                span: spans[0],
                parameter: parameters[0],
            },
            Direction {
                knots: &self.knots_v,
                degree: degrees[1],
                span: spans[1],
                parameter: parameters[1],
            },
        ];
        let directions = |order: [usize; 2]| {
            std::array::from_fn::<_, 2, _>(|axis| {
                let mut direction = base[axis];
                for _ in 0..order[axis] {
                    direction = direction.differentiated();
                }
                direction
            })
        };
        let h = homogeneous_partials(
            &self.exact_controls(spans),
            degrees,
            || std::array::from_fn(|_| Rational::zero()),
            |net, order, axis| {
                directions(order)[axis].derivative_controls(
                    net,
                    degrees[0] + 1 - order[0],
                    axis == 0,
                )
            },
            |net, order| {
                let [u, v] = directions(order);
                super::super::exact::tensor(net, u, v)
            },
        )?;
        let w = &h[0][3];
        if w.is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let mut values: [[Rational; 3]; 10] =
            std::array::from_fn(|_| std::array::from_fn(|_| Rational::zero()));
        values[0] = std::array::from_fn(|c| &h[0][c] / w);
        for total in 1..=3 {
            for v in 0..=total {
                let u = total - v;
                let k = index(u, v);
                let mut n: [Rational; 3] = std::array::from_fn(|c| h[k][c].clone());
                for (a, cu) in BINOMIAL[u].iter().enumerate().take(u + 1) {
                    for (b, cv) in BINOMIAL[v].iter().enumerate().take(v + 1) {
                        if a + b == 0 {
                            continue;
                        }
                        let factor = Rational::from_integer(((*cu as u32) * (*cv as u32)).into())
                            * &h[index(a, b)][3];
                        for (c, n) in n.iter_mut().enumerate() {
                            *n -= &values[index(u - a, v - b)][c] * &factor;
                        }
                    }
                }
                values[k] = n.map(|n| n / w);
            }
        }
        let point = self.exact_point_from_homogeneous(parameters, spans, &h[0])?;
        let mut rounded = [[0.; 3]; 10];
        for (out, value) in rounded[1..].iter_mut().zip(&values[1..]) {
            for (out, value) in out.iter_mut().zip(value) {
                *out = scalar(value)?;
            }
        }
        jet(point, rounded)
    }
}
