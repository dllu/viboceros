//! Guarded exact-rational evaluation when binary64 homogeneous preparation loses range.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, rational, scalar, vector};
use num_traits::Zero;

#[cfg(test)]
mod tests;

fn tensor(
    net: &[Homogeneous],
    u: Direction<'_>,
    v: Direction<'_>,
) -> Result<Homogeneous, GeometryError> {
    let rows = net
        .chunks_exact(u.degree + 1)
        .map(|row| u.evaluate(row.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    v.evaluate(rows)
}

impl NurbsSurface {
    pub(super) fn exact_jet_at_spans(
        &self,
        parameters: [Real; 2],
        spans: [usize; 2],
        order: u8,
    ) -> Result<SurfaceJet2, GeometryError> {
        let u = Direction {
            knots: &self.knots_u,
            degree: self.degree_u,
            span: spans[0],
            parameter: parameters[0],
        };
        let v = Direction {
            knots: &self.knots_v,
            degree: self.degree_v,
            span: spans[1],
            parameter: parameters[1],
        };
        let net = (v.span - v.degree..=v.span)
            .flat_map(|j| {
                (u.span - u.degree..=u.span).map(move |i| {
                    let control = self.control_points[self.control_index(i, j)];
                    let p = control.point();
                    let w = rational(control.weight());
                    [
                        rational(p.x()) * &w,
                        rational(p.y()) * &w,
                        rational(p.z()) * &w,
                        w,
                    ]
                })
            })
            .collect::<Vec<_>>();
        let h = tensor(&net, u, v)?;
        let w = &h[3];
        if w.is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let p: [Rational; 3] = std::array::from_fn(|i| &h[i] / w);
        let point = if let Some(point) = self.interpolated_point(parameters, spans) {
            point
        } else {
            Point3::try_new(scalar(&p[0])?, scalar(&p[1])?, scalar(&p[2])?)?
        };
        let mut jet = point_jet(point)?;
        if order == 0 {
            return Ok(jet);
        }
        let net_u = u.derivative_controls(&net, u.degree + 1, true)?;
        let net_v = v.derivative_controls(&net, u.degree + 1, false)?;
        let hu = tensor(&net_u, u.differentiated(), v)?;
        let hv = tensor(&net_v, u, v.differentiated())?;
        let du: [Rational; 3] = std::array::from_fn(|i| (&hu[i] - &p[i] * &hu[3]) / w);
        let dv: [Rational; 3] = std::array::from_fn(|i| (&hv[i] - &p[i] * &hv[3]) / w);
        jet.derivative_u = vector(&du)?;
        jet.derivative_v = vector(&dv)?;
        if order == 1 {
            return Ok(jet);
        }
        let huu = if u.degree > 1 {
            let second = u
                .differentiated()
                .derivative_controls(&net_u, u.degree, true)?;
            tensor(&second, u.differentiated().differentiated(), v)?
        } else {
            std::array::from_fn(|_| Rational::zero())
        };
        let hvv = if v.degree > 1 {
            let second = v
                .differentiated()
                .derivative_controls(&net_v, u.degree + 1, false)?;
            tensor(&second, u, v.differentiated().differentiated())?
        } else {
            std::array::from_fn(|_| Rational::zero())
        };
        let mixed = v.derivative_controls(&net_u, u.degree, false)?;
        let huv = tensor(&mixed, u.differentiated(), v.differentiated())?;
        jet.derivative_uu = vector(&std::array::from_fn(|i| {
            (&huu[i] - &p[i] * &huu[3] - &du[i] * &hu[3] - &du[i] * &hu[3]) / w
        }))?;
        jet.derivative_uv = vector(&std::array::from_fn(|i| {
            (&huv[i] - &p[i] * &huv[3] - &du[i] * &hv[3] - &dv[i] * &hu[3]) / w
        }))?;
        jet.derivative_vv = vector(&std::array::from_fn(|i| {
            (&hvv[i] - &p[i] * &hvv[3] - &dv[i] * &hv[3] - &dv[i] * &hv[3]) / w
        }))?;
        Ok(jet)
    }
}
