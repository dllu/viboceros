//! Ordered tensor edits reuse curve falloffs and exact isocurve extraction.
use super::*;

#[derive(Clone, Debug)]
pub struct SurfaceSoftEdit {
    source: NurbsSurface,
    parameter: [Real; 2],
    distances: [Real; 2],
    fix_edges: bool,
    v_falloff: Vec<Real>,
    tolerance: Tolerance,
}
impl SurfaceSoftEdit {
    pub fn anchor(&self) -> Result<Point3, GeometryError> {
        self.source.evaluate(self.parameter[0], self.parameter[1])
    }
    /// V rows are updated in order. Each U metric uses the evolving tensor,
    /// so intermediate isocurves retain the effects of earlier row edits.
    pub fn apply(&self, delta: Vector3) -> Result<NurbsSurface, GeometryError> {
        if delta.x() == 0. && delta.y() == 0. && delta.z() == 0. {
            return Ok(self.source.clone());
        }
        let mut result = self.source.clone();
        let [unique_u, _] = self.source.grip_dimensions();
        for (v, &factor) in self.v_falloff.iter().enumerate() {
            if factor == 0.
                || (self.fix_edges && (v == 0 || v + 1 == self.source.control_point_count_v))
            {
                continue;
            }
            let t = stable_knot_mean(&self.source.knots_v[v + 1..v + self.source.degree_v + 1])?;
            let t = t.clamp(
                *self.source.domain_v().start(),
                *self.source.domain_v().end(),
            );
            let row = result.isocurve_u(t)?;
            let collapsed = collapsed_controls(row.control_points(), self.tolerance)?;
            let canonical_pole = collapsed
                && (v == 0 || v + 1 == result.control_point_count_v)
                && collapsed_controls(
                    &self.source.control_points[v * self.source.control_point_count_u
                        ..(v + 1) * self.source.control_point_count_u],
                    self.tolerance,
                )?;
            let influence = if collapsed {
                vec![1.; self.source.control_point_count_u]
            } else {
                row.prepare_soft_edit(
                    self.parameter[0],
                    self.distances[0],
                    self.fix_edges,
                    self.tolerance,
                )?
                .influences()
                .to_vec()
            };
            for u in 0..result.control_point_count_u {
                if self.fix_edges && (u == 0 || u + 1 == result.control_point_count_u) {
                    continue;
                }
                let influence = factor * influence[u % unique_u];
                if influence == 0. {
                    continue;
                }
                let i = result.control_index(u, v);
                let control = result.control_points[i];
                // A numerically collapsed boundary has one physical pole.
                // Keep its controls coincident rather than introducing tiny
                // boundary edges after degree elevation or rational rounding.
                let base = if canonical_pole {
                    self.source.control_points[self.source.control_index(0, v)].point()
                } else {
                    control.point()
                };
                result.control_points[i] = WeightedPoint3::try_new(
                    base.translated(delta.scaled(influence)?)?,
                    control.weight(),
                )?;
            }
        }
        Ok(result)
    }
}
fn collapsed_controls(
    controls: &[WeightedPoint3],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let first = controls[0].point();
    let scale = controls
        .iter()
        .flat_map(|c| c.point().to_array())
        .map(Real::abs)
        .fold(1., Real::max);
    let bound = (64. * Real::EPSILON * scale).min(tolerance.absolute() * 0.125);
    let sign = controls[0].weight().is_sign_positive();
    for c in controls {
        if c.weight().is_sign_positive() != sign || c.point().distance_to(first)? > bound {
            return Ok(false);
        }
    }
    Ok(true)
}

impl NurbsSurface {
    pub fn prepare_soft_edit(
        &self,
        parameter: [Real; 2],
        distances: [Real; 2],
        fix_edges: bool,
        tolerance: Tolerance,
    ) -> Result<SurfaceSoftEdit, GeometryError> {
        self.evaluate(parameter[0], parameter[1])?;
        if self.is_closed_v()? {
            return Err(GeometryError::Degenerate {
                context: "surface soft edit currently requires an open V direction",
            });
        }
        require_finite(distances, "surface soft edit distances")?;
        if distances.iter().any(|&d| d <= 0.) {
            return Err(GeometryError::Degenerate {
                context: "surface soft edit distances must be positive",
            });
        }
        let v_curve = self.isocurve_v(parameter[0])?;
        let v_falloff = if collapsed_controls(v_curve.control_points(), tolerance)? {
            vec![1.; self.control_point_count_v]
        } else {
            v_curve
                .prepare_soft_edit(parameter[1], distances[1], false, tolerance)?
                .influences()
                .to_vec()
        };
        Ok(SurfaceSoftEdit {
            source: self.clone(),
            parameter,
            distances,
            fix_edges,
            v_falloff,
            tolerance,
        })
    }
    pub fn soft_edit_parameters(&self) -> Result<[Vec<Real>; 2], GeometryError> {
        // Surface getters keep both sides of a seam in the parameter chart.
        let axis = |knots: &[Real],
                    degree: usize,
                    count: usize,
                    domain: std::ops::RangeInclusive<Real>|
         -> Result<Vec<Real>, GeometryError> {
            let mut values = Vec::new();
            for i in 0..count {
                let t = stable_knot_mean(&knots[i + 1..i + degree + 1])?;
                if domain.contains(&t) && values.last() != Some(&t) {
                    values.push(t);
                }
            }
            Ok(values)
        };
        Ok([
            axis(
                &self.knots_u,
                self.degree_u,
                self.control_point_count_u,
                self.domain_u(),
            )?,
            axis(
                &self.knots_v,
                self.degree_v,
                self.control_point_count_v,
                self.domain_v(),
            )?,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch() -> NurbsSurface {
        NurbsSurface::try_new(
            3,
            3,
            4,
            4,
            (0..4)
                .flat_map(|v| {
                    (0..4).map(move |u| Point3::try_new(u as Real, v as Real, 0.).unwrap())
                })
                .collect(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap()
    }
    #[test]
    fn ordered_feedback_is_nonlinear_and_preview_order_is_immutable() {
        let s = patch();
        let plan = s
            .prepare_soft_edit([1. / 3., 2. / 3.], [4., 4.], false, Tolerance::DEFAULT)
            .unwrap();
        let v = Vector3::try_new(0., 0., 2.).unwrap();
        let a = plan.apply(v).unwrap();
        let b = plan.apply(v.scaled(4.).unwrap()).unwrap();
        assert_eq!(plan.apply(v).unwrap(), a);
        assert_eq!(
            plan.apply(Vector3::try_new(0., 0., 0.).unwrap()).unwrap(),
            s
        );
        assert_eq!(a.knots_u(), s.knots_u());
        assert_eq!(a.knots_v(), s.knots_v());
        assert!(
            a.control_point(0, 1).unwrap().point().z() > a.control_point(2, 1).unwrap().point().z()
        );
        assert!(
            (b.control_point(2, 1).unwrap().point().z()
                - 4. * a.control_point(2, 1).unwrap().point().z())
            .abs()
                > 1e-3
        );
        for (x, y) in a.control_points().iter().zip(s.control_points()) {
            assert_eq!(x.weight(), y.weight());
        }
    }
    #[test]
    fn fixed_boundary_and_invalid_preparation_preserve_source() {
        let s = patch();
        let p = s
            .prepare_soft_edit([1. / 3., 2. / 3.], [4., 4.], true, Tolerance::DEFAULT)
            .unwrap();
        let a = p.apply(Vector3::try_new(1., -2., 3.).unwrap()).unwrap();
        for v in 0..4 {
            for u in 0..4 {
                if u == 0 || u == 3 || v == 0 || v == 3 {
                    assert_eq!(a.control_point(u, v), s.control_point(u, v));
                }
            }
        }
        for d in [[0., 1.], [-1., 2.], [Real::NAN, 1.], [1., Real::INFINITY]] {
            assert!(
                s.prepare_soft_edit([0.5, 0.5], d, false, Tolerance::DEFAULT)
                    .is_err()
            );
        }
        assert!(
            s.prepare_soft_edit([-0.1, 0.5], [1., 1.], false, Tolerance::DEFAULT)
                .is_err()
        );
    }
    #[test]
    fn closed_v_plans_decline_without_mutating_the_source() {
        let s = NurbsSurface::try_new(
            1,
            1,
            2,
            3,
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [1., 1., 1.],
                [0., 0., 0.],
                [1., 0., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        assert!(s.is_closed_v().unwrap());
        let original = s.clone();
        assert!(
            s.prepare_soft_edit([0.5, 1.], [2., 2.], false, Tolerance::DEFAULT)
                .is_err()
        );
        assert_eq!(s, original);
    }
    #[test]
    fn collapsed_isocurves_at_poles_have_finite_falloff() {
        let mut s = patch();
        for u in 0..4 {
            let i = s.control_index(u, 0);
            s.control_points[i] =
                WeightedPoint3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 1.).unwrap();
        }
        let a = s
            .prepare_soft_edit([1. / 3., 1. / 3.], [4., 4.], false, Tolerance::DEFAULT)
            .unwrap()
            .apply(Vector3::try_new(0., 0., 1.).unwrap())
            .unwrap();
        assert_eq!(
            s.prepare_soft_edit([1. / 3., 1. / 3.], [4., 4.], false, Tolerance::DEFAULT)
                .unwrap()
                .apply(Vector3::try_new(0., 0., 0.).unwrap())
                .unwrap(),
            s
        );
        for u in 1..4 {
            assert_eq!(
                a.control_point(u, 0).unwrap().point(),
                a.control_point(0, 0).unwrap().point()
            );
        }
    }
}
