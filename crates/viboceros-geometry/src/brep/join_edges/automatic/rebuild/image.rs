//! Exact surface isocurves restricted by the original, oriented UV trim.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct BoundaryImage {
    curves: Vec<certificate::SurfaceCurve>,
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
        let fixed = 1 - varying;
        let interval = [a[varying], b[varying]];
        if interval[0] == interval[1] || interval.iter().any(|t| !domains[varying].contains(t)) {
            return Ok(None);
        }
        budget.charge(knots[0].len().saturating_add(knots[1].len()))?;
        let is_break = |axis: usize, t: Real| {
            t > *domains[axis].start()
                && t < *domains[axis].end()
                && knots[axis].partition_point(|k| *k <= t)
                    - knots[axis].partition_point(|k| *k < t)
                    == degree[axis] + 1
        };
        let mut curves = Vec::with_capacity(2);
        for left in [false, true] {
            if left && !is_break(fixed, a[fixed]) {
                break;
            }
            let Some(curve) =
                certificate::SurfaceCurve::new(surface, varying, a[fixed], left, &mut |n| {
                    budget.charge(n)
                })?
            else {
                return Ok(None);
            };
            curves.push(curve);
        }
        Ok(Some(Self {
            curves,
            interval,
            break_ends: interval.map(|t| is_break(varying, t)),
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
        let mut bound: Real = 0.;
        for curve in &self.curves {
            let Some(gap) = curve.bound(edge, interval, tighten, &mut |n| budget.charge(n))? else {
                return Ok(None);
            };
            bound = bound.max(gap);
            for end in [false, true] {
                if let Some((other, side)) = self.outside_endpoint(end ^ reversed_3d) {
                    let Some(gap) =
                        curve.edge_endpoint_bound(edge, other, [end, side], &mut |n| {
                            budget.charge(n)
                        })?
                    else {
                        return Ok(None);
                    };
                    bound = bound.max(gap);
                }
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
        let mut bound: Real = 0.;
        for curve in &self.curves {
            let Some(gap) =
                curve.point_bound(point, self.interval, end, &mut |n| budget.charge(n))?
            else {
                return Ok(None);
            };
            bound = bound.max(gap);
            if let Some((other, side)) = self.outside_endpoint(end) {
                let Some(gap) = curve.point_bound(point, other, side, &mut |n| budget.charge(n))?
                else {
                    return Ok(None);
                };
                bound = bound.max(gap);
            }
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
        let domain = self.curves[0].domain();
        Some(if t > self.interval[1 - i] {
            ([t, domain[1]], false)
        } else {
            ([domain[0], t], true)
        })
    }
}
