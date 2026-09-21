//! Exact natural surface rows restricted by the original, oriented UV trim.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct BoundaryImage {
    curve: NurbsCurve,
    interval: [Real; 2],
    break_ends: [bool; 2],
}

impl BoundaryImage {
    pub fn new(
        surface: &NurbsSurface,
        trim: &BrepTrim,
        budget: &mut Budget,
    ) -> Result<Option<Self>, GeometryError> {
        budget.charge(trim.curve.control_points().len())?;
        let uv = crate::brep::trim_image::LiftedTrim::new(trim, surface)?.curve;
        let Some([start, end]) = certificate::linear_endpoints(&uv) else {
            return Ok(None);
        };
        let [a, b] = [start, end].map(Point3::to_array);
        let varying = if a[1] == b[1] {
            0
        } else if a[0] == b[0] {
            1
        } else {
            return Ok(None);
        };
        let domains = [surface.domain_u(), surface.domain_v()];
        let knots = [surface.knots_u(), surface.knots_v()];
        let degree = [surface.degree_u(), surface.degree_v()];
        let counts = [
            surface.control_point_count_u(),
            surface.control_point_count_v(),
        ];
        let fixed = 1 - varying;
        let row = if a[fixed] == *domains[fixed].start()
            && knots[fixed][..=degree[fixed]]
                .iter()
                .all(|k| *k == a[fixed])
        {
            0
        } else if a[fixed] == *domains[fixed].end()
            && knots[fixed][knots[fixed].len() - degree[fixed] - 1..]
                .iter()
                .all(|k| *k == a[fixed])
        {
            counts[fixed] - 1
        } else {
            return Ok(None);
        };
        let interval = [a[varying], b[varying]];
        if interval[0] == interval[1] || interval.iter().any(|t| !domains[varying].contains(t)) {
            return Ok(None);
        }
        budget.charge(counts[varying])?;
        let curve = NurbsCurve::try_new_rational(
            degree[varying],
            (0..counts[varying])
                .map(|i| {
                    if varying == 0 {
                        surface.control_point(i, row).unwrap()
                    } else {
                        surface.control_point(row, i).unwrap()
                    }
                })
                .collect(),
            knots[varying].to_vec(),
        )?;
        // The fixed direction is clamped, so this row/column is exact. The
        // varying direction need not be clamped: exact span extraction handles
        // its actual endpoints and every part of the restricted interval.
        budget.charge(curve.knots().len())?;
        let mut break_ends = [false; 2];
        for knot in curve.full_order_knots() {
            for i in 0..2 {
                break_ends[i] |= interval[i] == knot;
            }
        }
        Ok(Some(Self {
            curve,
            interval,
            break_ends,
        }))
    }

    pub fn bound(
        &self,
        edge: &NurbsCurve,
        reversed_3d: bool,
        tighten: bool,
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        let mut interval = self.interval;
        if reversed_3d {
            interval.reverse();
        }
        let Some(mut bound) = certificate::restricted_curve_bound(
            edge,
            &self.curve,
            interval,
            Real::MAX,
            tighten,
            |n| budget.charge(n),
        )?
        else {
            return Ok(None);
        };
        for end in [false, true] {
            if let Some((other, side)) = self.outside_endpoint(end ^ reversed_3d) {
                let Some(gap) = certificate::curve_endpoint_bound(
                    edge,
                    &self.curve,
                    other,
                    [end, side],
                    Real::MAX,
                    |n| budget.charge(n),
                )?
                else {
                    return Ok(None);
                };
                bound = bound.max(gap);
            }
        }
        Ok(Some(bound))
    }

    pub fn endpoint_bound(
        &self,
        point: Point3,
        end: bool,
        budget: &mut Budget,
    ) -> Result<Option<Real>, GeometryError> {
        let Some(mut bound) = certificate::restricted_endpoint_bound(
            point,
            &self.curve,
            self.interval,
            end,
            Real::MAX,
            |n| budget.charge(n),
        )?
        else {
            return Ok(None);
        };
        if let Some((other, side)) = self.outside_endpoint(end) {
            let Some(gap) = certificate::restricted_endpoint_bound(
                point,
                &self.curve,
                other,
                side,
                Real::MAX,
                |n| budget.charge(n),
            )?
            else {
                return Ok(None);
            };
            bound = bound.max(gap);
        }
        Ok(Some(bound))
    }

    /// Surface evaluation and the interior trim limit can disagree at a jump.
    /// Bound both values, not merely the side reached by the curve restriction.
    fn outside_endpoint(&self, end: bool) -> Option<([Real; 2], bool)> {
        let i = usize::from(end);
        if !self.break_ends[i] {
            return None;
        }
        let t = self.interval[i];
        let domain = self.curve.domain();
        Some(if t > self.interval[1 - i] {
            ([t, *domain.end()], false)
        } else {
            ([*domain.start(), t], true)
        })
    }
}
