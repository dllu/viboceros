//! Local control displacements measured along the original curve.
use super::*;
use crate::{ArcLengthSampler, CurveRef};

/// Prepared falloff on an immutable NURBS control net. Repeated destination
/// previews only translate controls; they do not integrate the curve again.
#[derive(Clone, Debug)]
pub struct CurveSoftEdit {
    source: NurbsCurve,
    influence: Vec<Real>,
    anchor: Point3,
}

impl CurveSoftEdit {
    pub fn anchor(&self) -> Point3 {
        self.anchor
    }
    pub fn influences(&self) -> &[Real] {
        &self.influence
    }
    pub fn apply(&self, delta: Vector3) -> Result<NurbsCurve, GeometryError> {
        let mut curve = self.source.clone();
        for (control, &influence) in curve.control_points.iter_mut().zip(&self.influence) {
            if influence != 0. {
                *control = WeightedPoint3::try_new(
                    control.point().translated(delta.scaled(influence)?)?,
                    control.weight(),
                )?;
            }
        }
        Ok(curve)
    }
}

impl NurbsCurve {
    /// Prepare a Gaussian control falloff with sigma equal to one third of
    /// `distance`. Control locations are associated with their Greville sites.
    /// Stored Greville sites in the active chart contribute independently.
    /// Shared seam controls sum their contributions before synchronizing aliases.
    pub fn prepare_soft_edit(
        &self,
        parameter: Real,
        distance: Real,
        fix_ends: bool,
        tolerance: Tolerance,
    ) -> Result<CurveSoftEdit, GeometryError> {
        self.validate_parameter(parameter)?;
        require_finite([distance], "soft edit distance")?;
        if distance <= 0. {
            return Err(GeometryError::Degenerate {
                context: "soft edit distance must be positive",
            });
        }
        let lengths = ArcLengthSampler::try_new(CurveRef::NurbsCurve(self), tolerance)?;
        let center = lengths.distance_at_parameter(parameter)?;
        let domain = self.domain();
        let count = self.control_points.len();
        let mut influence = vec![0.; count];
        let periodic = self.is_periodic();
        let unique = if periodic {
            count - self.degree
        } else if self.is_closed()?
            && curve_points_coincident(
                self.control_points[0].point(),
                self.control_points[count - 1].point(),
            )
        {
            count - 1
        } else {
            count
        };
        for i in 0..count {
            let t = self.control_greville_parameter(i)?;
            if !domain.contains(&t) || (fix_ends && !periodic && (i == 0 || i + 1 == count)) {
                continue;
            }
            let gap = (lengths.distance_at_parameter(t)? - center).abs();
            let ratio = gap / distance;
            influence[i % unique] += (-4.5 * ratio * ratio).exp();
        }
        for i in unique..count {
            influence[i] = influence[i % unique];
        }
        Ok(CurveSoftEdit {
            source: self.clone(),
            influence,
            anchor: self.evaluate(parameter)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line() -> NurbsCurve {
        NurbsCurve::try_new(
            1,
            (-2..=2)
                .map(|x| Point3::try_new(x as Real, 0., 0.).unwrap())
                .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap()
    }
    #[test]
    fn straight_curve_has_analytic_gaussian_and_exact_fixed_ends() {
        let c = line();
        let plan = c
            .prepare_soft_edit(2., 3., false, Tolerance::DEFAULT)
            .unwrap();
        for (i, &f) in plan.influences().iter().enumerate() {
            let d = (i as Real - 2.) / 3.;
            assert!((f - (-4.5 * d * d).exp()).abs() < 1e-14);
        }
        let delta = Vector3::try_new(0., 2., 1.).unwrap();
        let result = plan.apply(delta).unwrap();
        assert_eq!(result.knots(), c.knots());
        assert_eq!(result.degree(), c.degree());
        for (i, cv) in result.control_points().iter().enumerate() {
            assert_eq!(cv.weight(), c.control_points()[i].weight());
            assert_eq!(
                cv.point(),
                c.control_points()[i]
                    .point()
                    .translated(delta.scaled(plan.influences()[i]).unwrap())
                    .unwrap()
            );
        }
        let fixed = c
            .prepare_soft_edit(2., 3., true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(fixed.influences()[0], 0.);
        assert_eq!(fixed.influences()[4], 0.);
        assert_eq!(fixed.influences()[1..4], plan.influences()[1..4]);
    }
    #[test]
    fn domain_changes_and_preview_order_do_not_change_displacements() {
        let c = line();
        let other = c.try_reparameterized(-1e120..=3e120).unwrap();
        let a = c
            .prepare_soft_edit(2., 2., false, Tolerance::DEFAULT)
            .unwrap();
        let b = other
            .prepare_soft_edit(1e120, 2., false, Tolerance::DEFAULT)
            .unwrap();
        for (&x, &y) in a.influences().iter().zip(b.influences()) {
            assert!((x - y).abs() < 1e-14);
        }
        let v = Vector3::try_new(1., 2., 3.).unwrap();
        let first = a.apply(v).unwrap();
        let _ = a.apply(v.scaled(-2.).unwrap()).unwrap();
        assert_eq!(a.apply(v).unwrap(), first);
        assert_eq!(a.apply(Vector3::try_new(0., 0., 0.).unwrap()).unwrap(), c);
    }
    #[test]
    fn closed_seam_sums_aliases_fixed_ends_and_invalid_distances_decline() {
        let c = NurbsCurve::try_new(
            1,
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [1., 1., 0.],
                [0., 1., 0.],
                [0., 0., 0.],
            ]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
            vec![0., 0., 1., 2., 3., 4., 4.],
        )
        .unwrap();
        let p = c
            .prepare_soft_edit(0., 2., true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(p.influences()[0], 0.);
        assert_eq!(p.influences()[4], 0.);
        let free = c
            .prepare_soft_edit(0., 2., false, Tolerance::DEFAULT)
            .unwrap();
        assert!((free.influences()[0] - (1. + (-18_f64).exp())).abs() < 1e-14);
        assert_eq!(free.influences()[0], free.influences()[4]);
        assert!(free.influences()[1] > free.influences()[3]);
        for d in [0., -1., Real::NAN, Real::INFINITY] {
            assert!(
                c.prepare_soft_edit(0., d, false, Tolerance::DEFAULT)
                    .is_err()
            );
        }
        assert!(
            c.prepare_soft_edit(-1., 2., false, Tolerance::DEFAULT)
                .is_err()
        );
    }
}
