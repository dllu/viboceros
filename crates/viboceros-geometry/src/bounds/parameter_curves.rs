//! Bounds of the exact surface image of a UV curve, including knot crossings.
use super::{
    bezier::{self, Budget, Net},
    surfaces,
};
use crate::{
    BoundingBox3, BrepFace, GeometryError, NurbsCurve2, NurbsSurface, Point3, Tolerance,
    WeightedPoint3,
};

impl NurbsSurface {
    /// Tolerance-controlled bounds of S(u(t),v(t)) for a complete parameter
    /// curve. This queries the surface image, not a fitted spatial edge or a
    /// display polyline. Undefined images and exhausted budgets return errors.
    /// Homogeneous floating-point refinement is not interval certification.
    pub fn parameter_curve_bounds(
        &self,
        curve: &NurbsCurve2,
        tolerance: Tolerance,
    ) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        Ok(Prepared::new(self, &mut budget)?
            .curve_bounds(curve, &mut budget, tolerance)?
            .enclosure)
    }
}

impl BrepFace {
    /// Bounds of the surface images of all outer/inner trim curves, including
    /// seams and singular trims. This is NOT a box of the complete face:
    /// extrema attained in its interior must be handled separately.
    pub fn trim_boundary_bounds(
        &self,
        tolerance: Tolerance,
    ) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        let prepared = Prepared::new(self.surface(), &mut budget)?;
        Ok(prepared
            .boundary_bounds(self, &mut budget, tolerance)?
            .enclosure)
    }
}

pub(super) struct Prepared {
    pub(super) patches: Vec<surfaces::Patch>,
    pub(super) domains: [[f64; 2]; 2],
}

fn normalize(value: f64, domain: [f64; 2]) -> Result<f64, GeometryError> {
    crate::nurbs::interval_fraction_unbounded(value, domain[0], domain[1])
}

impl Prepared {
    pub(super) fn new(surface: &NurbsSurface, budget: &mut Budget) -> Result<Self, GeometryError> {
        let mut patches = surfaces::patches(surface, budget)?;
        let domains = [surface.domain_u(), surface.domain_v()].map(|d| [*d.start(), *d.end()]);
        for patch in &mut patches {
            for (axis, domain) in domains.iter().copied().enumerate() {
                patch.domain[axis] = [
                    normalize(patch.domain[axis][0], domain)?,
                    normalize(patch.domain[axis][1], domain)?,
                ];
                if patch.domain[axis][0] >= patch.domain[axis][1] {
                    return Err(GeometryError::BoundingBoxDidNotConverge);
                }
            }
        }
        Ok(Self { patches, domains })
    }

    pub(super) fn boundary_bounds(
        &self,
        face: &BrepFace,
        budget: &mut Budget,
        tolerance: Tolerance,
    ) -> Result<bezier::Estimate, GeometryError> {
        let mut enclosure = None;
        let mut attained = None;
        for trim in face.loops().iter().flat_map(|l| l.trims()) {
            let bounds = self.curve_bounds(trim.curve(), budget, tolerance)?;
            bezier::merge(&mut enclosure, bounds.enclosure)?;
            bezier::merge(&mut attained, bounds.attained)?;
        }
        Ok(bezier::Estimate {
            enclosure: enclosure.ok_or(GeometryError::EmptyPointSet)?,
            attained: attained.ok_or(GeometryError::EmptyPointSet)?,
        })
    }

    fn curve_bounds(
        &self,
        curve: &NurbsCurve2,
        budget: &mut Budget,
        tolerance: Tolerance,
    ) -> Result<bezier::Estimate, GeometryError> {
        let p = curve.degree();
        let mut pending = spans(curve, self.domains, budget)?;
        let mut composed = Vec::new();
        let mut attained = None;
        let mut enclosure = None;
        while let Some(uv) = pending.pop() {
            budget.visit()?;
            budget.charge((p + 1).saturating_pow(2))?;
            let constant: [bool; 2] =
                std::array::from_fn(|axis| uv.controls.iter().all(|h| h[axis] == 0.));
            // At a jump, sub-ULP UV uncertainty can become a macroscopic
            // spatial error. Keep both neighboring hulls in the candidate set
            // and do not use an ambiguous branch selection as an attained
            // witness. Exact constant coordinates retain their native side.
            let uv_roundoff = 64. * f64::EPSILON * ((p + 1) as f64).powi(2);
            let points = [
                uv.project(uv.controls[0])?,
                uv.center()?,
                uv.project(*uv.controls.last().unwrap())?,
            ];
            for point in points {
                for coordinate in [point.x(), point.y()] {
                    if !(0. ..=1.).contains(&coordinate) {
                        return Err(GeometryError::ParameterCurveOutsideSurfaceDomain);
                    }
                }
            }
            if let Some(hull) = uv.hull() {
                let min = hull.min().to_array();
                let max = hull.max().to_array();
                let inside = min[0] >= 0. && max[0] <= 1. && min[1] >= 0. && max[1] <= 1.;
                budget.charge(self.patches.len())?;
                let candidates = self
                    .patches
                    .iter()
                    .filter(|patch| {
                        (0..2).all(|i| {
                            // A curve lying entirely on a knot uses the native
                            // right-hand patch, or the left limit at domain end.
                            if constant[i] {
                                min[i] >= patch.domain[i][0]
                                    && (min[i] < patch.domain[i][1] || patch.domain[i][1] == 1.)
                            } else {
                                let low = patch.domain[i][0]
                                    - if patch.full_order_sides[i][0] {
                                        uv_roundoff
                                    } else {
                                        0.
                                    };
                                let high = patch.domain[i][1]
                                    + if patch.full_order_sides[i][1] {
                                        uv_roundoff
                                    } else {
                                        0.
                                    };
                                max[i] >= low && min[i] <= high
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                if inside && !candidates.is_empty() {
                    let mut combined = None;
                    let mut valid = true;
                    let single_patch = candidates.len() == 1;
                    for patch in candidates {
                        let net = patch.net.compose(&uv, patch.domain, budget)?;
                        for (index, point) in points.iter().enumerate() {
                            if (0..2).all(|i| {
                                let t = point.to_array()[i];
                                let ambiguous = !constant[i]
                                    && (0..2).any(|side| {
                                        patch.full_order_sides[i][side]
                                            && (t - patch.domain[i][side]).abs() <= uv_roundoff
                                    });
                                !ambiguous
                                    && t >= patch.domain[i][0]
                                    && (t < patch.domain[i][1] || patch.domain[i][1] == 1.)
                            }) {
                                let point = match index {
                                    0 => net.project(net.controls[0])?,
                                    1 => net.center()?,
                                    _ => net.project(*net.controls.last().unwrap())?,
                                };
                                bezier::merge(&mut attained, BoundingBox3::from_points([point])?)?;
                            }
                        }
                        if single_patch {
                            budget.initial(net.controls.len())?;
                            composed.push(net);
                            break;
                        }
                        if let Some(hull) = net.hull() {
                            bezier::merge(&mut combined, hull)?;
                        } else {
                            valid = false;
                            break;
                        }
                    }
                    if single_patch {
                        continue;
                    }
                    if valid
                        && attained
                            .is_some_and(|a| bezier::resolved(combined.unwrap(), a, tolerance))
                    {
                        bezier::merge(&mut enclosure, combined.unwrap())?;
                        continue;
                    }
                }
            }
            if uv.depth == bezier::MAX_DEPTH {
                return Err(GeometryError::BoundingBoxDidNotConverge);
            }
            budget.charge((p + 1).saturating_pow(2))?;
            let (left, right) = uv.split(0);
            pending.push(right);
            pending.push(left);
        }
        if !composed.is_empty() {
            let result = bezier::estimate(composed, budget, tolerance)?;
            bezier::merge(&mut enclosure, result.enclosure)?;
            bezier::merge(&mut attained, result.attained)?;
        }
        // Include selected-side values at surface-knot boundaries. Samples are
        // evaluated in homogeneous composition, not by rounding UV to a large
        // native domain and losing its continuous sub-ULP parameter geometry.
        bezier::merge(
            &mut enclosure,
            attained.ok_or(GeometryError::EmptyPointSet)?,
        )?;
        Ok(bezier::Estimate {
            enclosure: enclosure.ok_or(GeometryError::EmptyPointSet)?,
            attained: attained.ok_or(GeometryError::EmptyPointSet)?,
        })
    }
}

/// Extract raw rational trim spans directly into the surface's unit UV domain.
pub(super) fn spans(
    curve: &NurbsCurve2,
    domains: [[f64; 2]; 2],
    budget: &mut Budget,
) -> Result<Vec<Net>, GeometryError> {
    let p = curve.degree();
    let mut result = Vec::new();
    for span in p..curve.control_points().len() {
        if curve.knots()[span] == curve.knots()[span + 1] {
            continue;
        }
        budget.initial(p + 1)?;
        let controls = curve.control_points()[span - p..=span]
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_new(
                        normalize(c.point().x(), domains[0])?,
                        normalize(c.point().y(), domains[1])?,
                        0.,
                    )?,
                    c.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        // Keep boundary zeros and constant knot coordinates exact.
        let origin = std::array::from_fn(|axis| {
            let x = controls[0].point().to_array()[axis];
            if controls.iter().all(|c| c.point().to_array()[axis] == x) {
                x
            } else {
                0.
            }
        });
        let mut net = Net::new_at_origin([p, 0], &controls, origin)?;
        net.extract_axis(0, curve.knots(), span, budget)?;
        result.push(net);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
