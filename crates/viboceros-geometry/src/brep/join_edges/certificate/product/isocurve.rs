//! Exact tensor-product isocurves, without rounding their homogeneous controls.
use super::*;

#[cfg(test)]
mod tests;

pub(in crate::brep) enum SurfaceCurve {
    Natural(NurbsCurve),
    Tensor {
        degree: usize,
        knots: Vec<Real>,
        controls: Vec<H>,
    },
}

impl SurfaceCurve {
    pub fn new(
        surface: &NurbsSurface,
        varying: usize,
        parameter: Real,
        left: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Self>, GeometryError> {
        let knots = [surface.knots_u(), surface.knots_v()];
        let degree = [surface.degree_u(), surface.degree_v()];
        let counts = [
            surface.control_point_count_u(),
            surface.control_point_count_v(),
        ];
        let fixed = 1 - varying;
        let domain = knots[fixed][degree[fixed]]..=knots[fixed][counts[fixed]];
        if !parameter.is_finite() || !domain.contains(&parameter) {
            return Ok(None);
        }
        let control = |i, j| {
            if varying == 0 {
                surface.control_point(i, j).unwrap()
            } else {
                surface.control_point(j, i).unwrap()
            }
        };
        // A clamped natural side remains a cheap exact row/column copy.
        charge(degree[fixed].saturating_add(1).saturating_mul(2))?;
        let row = if parameter == *domain.start()
            && knots[fixed][..=degree[fixed]]
                .iter()
                .all(|k| *k == parameter)
        {
            Some(0)
        } else if parameter == *domain.end()
            && knots[fixed][counts[fixed]..]
                .iter()
                .all(|k| *k == parameter)
        {
            Some(counts[fixed] - 1)
        } else {
            None
        };
        charge(counts[varying].saturating_add(knots[varying].len()))?;
        if let Some(row) = row {
            return Ok(Some(Self::Natural(NurbsCurve::try_new_rational(
                degree[varying],
                (0..counts[varying]).map(|i| control(i, row)).collect(),
                knots[varying].to_vec(),
            )?)));
        }
        if degree.iter().any(|p| *p > MAX_DEGREE) {
            return Ok(None);
        }
        let span = if left && parameter > *domain.start() {
            (knots[fixed].partition_point(|k| *k < parameter) - 1).min(counts[fixed] - 1)
        } else {
            crate::nurbs::find_span_in_knots(knots[fixed], degree[fixed], counts[fixed], parameter)
        };
        let direction = crate::nurbs::exact::Direction {
            degree: degree[fixed],
            knots: knots[fixed],
            span,
            parameter,
        };
        let mut controls = Vec::with_capacity(counts[varying]);
        for i in 0..counts[varying] {
            charge(8 * (degree[fixed] + 1).pow(2))?;
            let row = (span - degree[fixed]..=span)
                .map(|j| {
                    let c = control(i, j);
                    let w = rational(c.weight());
                    std::array::from_fn(|axis| {
                        if axis == 3 {
                            w.clone()
                        } else {
                            rational(c.point().to_array()[axis]) * &w
                        }
                    })
                })
                .collect();
            controls.push(direction.evaluate(row)?);
        }
        // Surface weights may have mixed signs, but the resulting isocurve
        // needs a sign-coherent denominator for its convex-hull certificate.
        let zero = rational(0.);
        let negative = controls[0][3] < zero;
        if controls
            .iter()
            .any(|h| h[3] == zero || (h[3] < zero) != negative)
        {
            return Ok(None);
        }
        Ok(Some(Self::Tensor {
            degree: degree[varying],
            knots: knots[varying].to_vec(),
            controls,
        }))
    }

    pub fn knots(&self) -> &[Real] {
        match self {
            Self::Natural(c) => c.knots(),
            Self::Tensor { knots, .. } => knots,
        }
    }

    pub fn degree(&self) -> usize {
        match self {
            Self::Natural(c) => c.degree(),
            Self::Tensor { degree, .. } => *degree,
        }
    }

    pub fn domain(&self) -> [Real; 2] {
        let knots = self.knots();
        let p = self.degree();
        [knots[p], knots[knots.len() - p - 1]]
    }

    fn spline<'a>(
        &'a self,
        interval: [Real; 2],
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<extract::Spline<'a>>, GeometryError> {
        match self {
            Self::Natural(c) => extract::Spline::restricted(c, interval, charge),
            Self::Tensor {
                degree,
                knots,
                controls,
            } => extract::Spline::homogeneous(controls, *degree, knots, interval, charge),
        }
    }

    pub fn bound(
        &self,
        edge: &NurbsCurve,
        interval: [Real; 2],
        tighten: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Real>, GeometryError> {
        if let Self::Natural(c) = self {
            return restriction::restricted_curve_bound(
                edge,
                c,
                interval,
                Real::MAX,
                tighten,
                charge,
            );
        }
        if edge.degree() > MAX_DEGREE {
            return Ok(None);
        }
        let Some(a) = extract::Spline::new(edge, false, charge)? else {
            return Ok(None);
        };
        let Some(b) = self.spline(interval, charge)? else {
            return Ok(None);
        };
        splines_bound(&a, &b, Real::MAX, tighten, charge)
    }

    fn endpoint(
        &self,
        interval: [Real; 2],
        end: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<H>, GeometryError> {
        if let Self::Natural(c) = self {
            return restriction::endpoint_value(c, interval, end, charge);
        }
        let Some(spline) = self.spline(interval, charge)? else {
            return Ok(None);
        };
        let (mut net, _) = spline.end_span(end, charge)?;
        let i = if end { net.len() - 1 } else { 0 };
        Ok(Some(net.swap_remove(i)))
    }

    pub fn point_bound(
        &self,
        point: Point3,
        interval: [Real; 2],
        end: bool,
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Real>, GeometryError> {
        if let Self::Natural(c) = self {
            return restriction::restricted_endpoint_bound(
                point,
                c,
                interval,
                end,
                Real::MAX,
                charge,
            );
        }
        let Some(h) = self.endpoint(interval, end, charge)? else {
            return Ok(None);
        };
        let p = point.to_array();
        let difference = std::array::from_fn(|i| {
            if i == 3 {
                h[3].clone()
            } else {
                &h[i] - rational(p[i]) * &h[3]
            }
        });
        Ok(refine::norm_bound(&difference, Real::MAX))
    }

    pub fn edge_endpoint_bound(
        &self,
        edge: &NurbsCurve,
        interval: [Real; 2],
        ends: [bool; 2],
        charge: &mut impl FnMut(usize) -> Result<(), GeometryError>,
    ) -> Result<Option<Real>, GeometryError> {
        if let Self::Natural(c) = self {
            return restriction::curve_endpoint_bound(edge, c, interval, ends, Real::MAX, charge);
        }
        let domain = edge.domain();
        let Some(a) =
            restriction::endpoint_value(edge, [*domain.start(), *domain.end()], ends[0], charge)?
        else {
            return Ok(None);
        };
        let Some(b) = self.endpoint(interval, ends[1], charge)? else {
            return Ok(None);
        };
        let difference = std::array::from_fn(|i| {
            if i == 3 {
                &a[3] * &b[3]
            } else {
                &a[i] * &b[3] - &b[i] * &a[3]
            }
        });
        Ok(refine::norm_bound(&difference, Real::MAX))
    }
}
