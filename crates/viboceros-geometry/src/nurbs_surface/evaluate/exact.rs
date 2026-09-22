//! Exact-rational surface jets for range loss, signed cancellation, and continuation.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, rational, scalar, vector};
use num_traits::{Signed, Zero};

#[cfg(test)]
mod tests;

struct FirstNets {
    u: Vec<Homogeneous>,
    v: Vec<Homogeneous>,
}

struct SecondNets {
    uu: Option<Vec<Homogeneous>>,
    uv: Vec<Homogeneous>,
    vv: Option<Vec<Homogeneous>>,
}

/// One immutable active span's coefficients. Derivative nets depend on knots,
/// not the evaluation station, and are constructed only when requested.
pub(super) struct ExactJetNet<'a> {
    surface: &'a NurbsSurface,
    spans: [usize; 2],
    controls: Vec<Homogeneous>,
    first: Option<FirstNets>,
    second: Option<SecondNets>,
}

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
    pub(super) fn exact_controls(&self, [span_u, span_v]: [usize; 2]) -> Vec<Homogeneous> {
        (span_v - self.degree_v..=span_v)
            .flat_map(|j| {
                (span_u - self.degree_u..=span_u).map(move |i| {
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
            .collect()
    }

    pub(super) fn exact_point_from_homogeneous(
        &self,
        parameters: [Real; 2],
        spans: [usize; 2],
        h: &Homogeneous,
    ) -> Result<Point3, GeometryError> {
        if h[3].is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        if let Some(point) = self.interpolated_point(parameters, spans) {
            return Ok(point);
        }
        Point3::try_new(
            scalar(&(&h[0] / &h[3]))?,
            scalar(&(&h[1] / &h[3]))?,
            scalar(&(&h[2] / &h[3]))?,
        )
    }

    pub(super) fn exact_jet_at_spans(
        &self,
        parameters: [Real; 2],
        spans: [usize; 2],
        order: u8,
    ) -> Result<SurfaceJet2, GeometryError> {
        ExactJetNet::new(self, spans).evaluate(parameters, order)
    }
}

impl<'a> ExactJetNet<'a> {
    pub(super) fn new(surface: &'a NurbsSurface, spans: [usize; 2]) -> Self {
        Self {
            surface,
            spans,
            controls: surface.exact_controls(spans),
            first: None,
            second: None,
        }
    }

    fn prepare_first_nets(
        &mut self,
        u: Direction<'_>,
        v: Direction<'_>,
    ) -> Result<(), GeometryError> {
        if self.first.is_none() {
            self.first = Some(FirstNets {
                u: u.derivative_controls(&self.controls, u.degree + 1, true)?,
                v: v.derivative_controls(&self.controls, u.degree + 1, false)?,
            });
        }
        Ok(())
    }

    pub(super) fn normal(
        &mut self,
        parameters: [Real; 2],
    ) -> Result<crate::UnitVector3, GeometryError> {
        let surface = self.surface;
        let u = Direction {
            knots: &surface.knots_u,
            degree: surface.degree_u,
            span: self.spans[0],
            parameter: parameters[0],
        };
        let v = Direction {
            knots: &surface.knots_v,
            degree: surface.degree_v,
            span: self.spans[1],
            parameter: parameters[1],
        };
        let h = tensor(&self.controls, u, v)?;
        if h[3].is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        self.prepare_first_nets(u, v)?;
        let first = self
            .first
            .as_ref()
            .expect("first derivative nets initialized");
        let hu = tensor(&first.u, u.differentiated(), v)?;
        let hv = tensor(&first.v, u, v.differentiated())?;
        // Su=(Hu W-H Wu)/W², and likewise for v. W⁴ is strictly positive,
        // so its magnitude and sign need not be representable to orient Su×Sv.
        let a: [Rational; 3] = std::array::from_fn(|i| &hu[i] * &h[3] - &h[i] * &hu[3]);
        let b: [Rational; 3] = std::array::from_fn(|i| &hv[i] * &h[3] - &h[i] * &hv[3]);
        let n: [Rational; 3] = std::array::from_fn(|i| {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            &a[j] * &b[k] - &a[k] * &b[j]
        });
        let scale = n.iter().map(Signed::abs).max().unwrap();
        if scale.is_zero() {
            return Err(GeometryError::Degenerate {
                context: "surface normal",
            });
        }
        vector(&std::array::from_fn(|i| &n[i] / &scale))?.normalized_nonzero()
    }

    pub(super) fn evaluate(
        &mut self,
        parameters: [Real; 2],
        order: u8,
    ) -> Result<SurfaceJet2, GeometryError> {
        let spans = self.spans;
        let surface = self.surface;
        let u = Direction {
            knots: &surface.knots_u,
            degree: surface.degree_u,
            span: spans[0],
            parameter: parameters[0],
        };
        let v = Direction {
            knots: &surface.knots_v,
            degree: surface.degree_v,
            span: spans[1],
            parameter: parameters[1],
        };
        let h = tensor(&self.controls, u, v)?;
        if order == 0 {
            return point_jet(surface.exact_point_from_homogeneous(parameters, spans, &h)?);
        }
        let w = &h[3];
        if w.is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let p: [Rational; 3] = std::array::from_fn(|i| &h[i] / w);
        let point = if let Some(point) = surface.interpolated_point(parameters, spans) {
            point
        } else {
            Point3::try_new(scalar(&p[0])?, scalar(&p[1])?, scalar(&p[2])?)?
        };
        let mut jet = point_jet(point)?;
        self.prepare_first_nets(u, v)?;
        let first = self
            .first
            .as_ref()
            .expect("first derivative nets initialized");
        let hu = tensor(&first.u, u.differentiated(), v)?;
        let hv = tensor(&first.v, u, v.differentiated())?;
        let du: [Rational; 3] = std::array::from_fn(|i| (&hu[i] - &p[i] * &hu[3]) / w);
        let dv: [Rational; 3] = std::array::from_fn(|i| (&hv[i] - &p[i] * &hv[3]) / w);
        jet.derivative_u = vector(&du)?;
        jet.derivative_v = vector(&dv)?;
        if order == 1 {
            return Ok(jet);
        }
        if self.second.is_none() {
            self.second = Some(SecondNets {
                uu: if u.degree > 1 {
                    Some(
                        u.differentiated()
                            .derivative_controls(&first.u, u.degree, true)?,
                    )
                } else {
                    None
                },
                uv: v.derivative_controls(&first.u, u.degree, false)?,
                vv: if v.degree > 1 {
                    Some(
                        v.differentiated()
                            .derivative_controls(&first.v, u.degree + 1, false)?,
                    )
                } else {
                    None
                },
            });
        }
        let second = self
            .second
            .as_ref()
            .expect("second derivative nets initialized");
        let huu = if let Some(net) = &second.uu {
            tensor(net, u.differentiated().differentiated(), v)?
        } else {
            std::array::from_fn(|_| Rational::zero())
        };
        let hvv = if let Some(net) = &second.vv {
            tensor(net, u, v.differentiated().differentiated())?
        } else {
            std::array::from_fn(|_| Rational::zero())
        };
        let huv = tensor(&second.uv, u.differentiated(), v.differentiated())?;
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
