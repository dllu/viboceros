//! Ordered tensor edits reuse curve falloffs and exact isocurve extraction.
use super::*;

#[derive(Clone, Debug)]
pub struct SurfaceSoftEdit {
    source: NurbsSurface,
    parameter: [Real; 2],
    distances: [Real; 2],
    fix_edges: bool,
    v_falloff: Vec<Real>,
    site_parameters: Vec<Real>,
    v_unique: usize,
    periodic_v: bool,
    tolerance: Tolerance,
}
impl SurfaceSoftEdit {
    pub fn anchor(&self) -> Result<Point3, GeometryError> {
        self.source.evaluate(self.parameter[0], self.parameter[1])
    }
    /// Visit each stored V site, starting with its own row and then its seam
    /// aliases. Every row visit measures U on the evolving surface. Both
    /// contributions at a repeated seam remain distinct operations.
    pub fn apply(&self, delta: Vector3) -> Result<NurbsSurface, GeometryError> {
        if delta.x() == 0. && delta.y() == 0. && delta.z() == 0. {
            return Ok(self.source.clone());
        }
        let mut result = self.source.clone();
        for (site, &factor) in self.v_falloff.iter().enumerate() {
            if factor == 0.
                || self.fix_edges
                    && !self.periodic_v
                    && (site == 0 || site + 1 == self.source.control_point_count_v)
            {
                continue;
            }
            let aliases = std::iter::once(site).chain(
                (site % self.v_unique..self.source.control_point_count_v)
                    .step_by(self.v_unique)
                    .filter(|&v| v != site),
            );
            for v in aliases {
                let row = result.isocurve_u(self.site_parameters[site])?;
                let collapsed = collapsed_controls(row.control_points(), self.tolerance)?;
                let influence = if collapsed {
                    let mut factors = vec![1.; self.source.control_point_count_u];
                    if self.fix_edges && !row.is_periodic() {
                        factors[0] = 0.;
                        let last = factors.len() - 1;
                        factors[last] = 0.;
                    }
                    factors
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
                let canonical_pole = collapsed
                    && (v == 0 || v + 1 == result.control_point_count_v)
                    && collapsed_controls(
                        &result.control_points[v * self.source.control_point_count_u
                            ..(v + 1) * self.source.control_point_count_u],
                        self.tolerance,
                    )?;
                let pole = result.control_points[result.control_index(0, v)].point();
                for (u, &u_factor) in influence.iter().enumerate() {
                    let amount = factor * u_factor;
                    if amount == 0. {
                        continue;
                    }
                    let i = result.control_index(u, v);
                    let control = result.control_points[i];
                    let base = if canonical_pole {
                        pole
                    } else {
                        control.point()
                    };
                    result.control_points[i] = WeightedPoint3::try_new(
                        base.translated(delta.scaled(amount)?)?,
                        control.weight(),
                    )?;
                }
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
        require_finite(distances, "surface soft edit distances")?;
        if distances.iter().any(|&d| d <= 0.) {
            return Err(GeometryError::Degenerate {
                context: "surface soft edit distances must be positive",
            });
        }
        let v_curve = self.isocurve_v(parameter[0])?;
        let collapsed_v = collapsed_controls(v_curve.control_points(), tolerance)?;
        let v_falloff = if collapsed_v {
            vec![1.; self.control_point_count_v]
        } else {
            v_curve.soft_edit_site_falloff(parameter[1], distances[1], tolerance)?
        };
        // Alias layout belongs to the selected V isocurve, not to the
        // complete tensor. After a seam-opening edit another isocurve can
        // remain closed or periodic, and must retain its own local aliases.
        let periodic_v = v_curve.is_periodic();
        let v_unique = if collapsed_v {
            self.control_point_count_v
        } else {
            v_curve.grip_count()?
        };
        let domain = self.domain_v();
        let site_parameters = (0..self.control_point_count_v)
            .map(|v| {
                stable_knot_mean(&self.knots_v[v + 1..v + self.degree_v + 1])
                    .map(|t| t.clamp(*domain.start(), *domain.end()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SurfaceSoftEdit {
            source: self.clone(),
            parameter,
            distances,
            fix_edges,
            v_falloff,
            site_parameters,
            v_unique,
            periodic_v,
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
    fn closed_v_keeps_unfolded_sites_and_visits_both_aliases() {
        let s = NurbsSurface::try_new(
            1,
            1,
            2,
            4,
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [1., 1., 0.],
                [0., 0., 1.],
                [1., 0., 1.],
                [0., 0., 0.],
                [1., 0., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 2., 3., 3.],
        )
        .unwrap();
        assert!(s.is_closed_v().unwrap());
        let original = s.clone();
        let start = s
            .prepare_soft_edit([0.5, 0.25], [2., 2.], false, Tolerance::DEFAULT)
            .unwrap();
        // Both U controls translate equally, so every evolving U isocurve
        // keeps length one. The closing V rows must each receive the two
        // distinct endpoint Gaussian contributions around the triangular path.
        let u_factor = (-4.5_f64 * (0.5_f64 / 2.).powi(2)).exp();
        let start_factor = (-4.5_f64 * (0.25_f64 / 2.).powi(2)).exp();
        let end_gap = 2. + 2_f64.sqrt() - 0.25;
        let end_factor = (-4.5 * (end_gap / 2.).powi(2)).exp();
        let expected_z = u_factor * (start_factor + end_factor);
        let edited = start.apply(Vector3::try_new(0., 0., 1.).unwrap()).unwrap();
        for u in 0..2 {
            for v in [0, 3] {
                let p = edited.control_point(u, v).unwrap().point();
                assert!((p.z() - expected_z).abs() < 1e-13);
                assert_eq!(p.x(), u as Real);
                assert_eq!(p.y(), 0.);
            }
        }
        assert_eq!(
            start.apply(Vector3::try_new(0., 0., 0.).unwrap()).unwrap(),
            original
        );
        assert_eq!(s, original);
    }
    #[test]
    fn periodic_alias_visits_follow_the_selected_isocurve_after_a_seam_opens() {
        let points = [
            [0., 0., 0.],
            [1., 3., 0.],
            [3., -1., 0.],
            [5., 0., 0.],
            [0., 0., 0.],
            [1., 3., 0.],
            [3., -1., 0.],
        ];
        let s = NurbsSurface::try_new(
            1,
            3,
            2,
            7,
            points
                .into_iter()
                .flat_map(|p| {
                    [
                        Point3::try_from(p).unwrap(),
                        Point3::try_new(p[0], p[1], 6.).unwrap(),
                    ]
                })
                .collect(),
            vec![0., 0., 6., 6.],
            (-3..=7).map(|x| x as Real).collect(),
        )
        .unwrap();
        let original = s.clone();
        assert!(s.is_periodic_v());
        let q = s
            .prepare_soft_edit([0., 2.], [4., 4.], false, Tolerance::DEFAULT)
            .unwrap()
            .apply(Vector3::try_new(-1., 0.2, 2.2).unwrap())
            .unwrap();
        assert_eq!(s, original);
        assert_eq!(q.control_point(0, 0), q.control_point(0, 4));
        assert_ne!(q.control_point(1, 0), q.control_point(1, 4));
        assert!(!q.is_periodic_v());
        let next = q
            .prepare_soft_edit([0., 2.], [4., 4.], false, Tolerance::DEFAULT)
            .unwrap();
        assert!(next.periodic_v);
        assert_eq!(next.v_unique, 4);
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
