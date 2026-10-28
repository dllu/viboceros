//! Roadlike defaults share the existing OpenNURBS support-plane query.
use super::*;

impl SweepFrameStyle {
    /// Use a planar rail's support-plane normal, otherwise world Z.
    /// Explicit `Roadlike(axis)` construction never invokes this inference.
    pub fn roadlike_default(
        rail: CurveRef<'_>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let axis = match rail.supporting_plane_normal(tolerance)? {
            Some(normal) => normal,
            // A fixed direction is dimensionless, independent of model units.
            None => Vector3::try_new(0., 0., 1.)?.normalized_nonzero()?,
        };
        Ok(Self::Roadlike(axis))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planar_defaults_follow_support_planes_and_spatial_defaults_use_world_z() {
        let tol = Tolerance::DEFAULT;
        let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let arc = crate::CircularArc3::try_from_three_points(
            p(3., 0., 0.),
            p(3. / 2_f64.sqrt(), 0., 3. / 2_f64.sqrt()),
            p(0., 0., 3.),
            tol,
        )
        .unwrap();
        let SweepFrameStyle::Roadlike(axis) =
            SweepFrameStyle::roadlike_default(CurveRef::Arc(&arc), tol).unwrap()
        else {
            panic!()
        };
        assert!(axis.y().abs() > 1. - 1e-12);
        let spatial = NurbsCurve::try_new(
            3,
            vec![p(0., 0., 0.), p(2., 1., 1.), p(-1., 3., 4.), p(0., 5., 5.)],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let SweepFrameStyle::Roadlike(axis) =
            SweepFrameStyle::roadlike_default(CurveRef::NurbsCurve(&spatial), tol).unwrap()
        else {
            panic!()
        };
        assert_eq!(axis.as_vector().to_array(), [0., 0., 1.]);
    }

    #[test]
    fn fixed_directions_are_independent_of_large_model_tolerances() {
        let tol = Tolerance::try_new(2., 1e-12, 1e-10).unwrap();
        let rail = crate::LineSegment::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(10., 0., 0.).unwrap(),
            tol,
        )
        .unwrap();
        let SweepFrameStyle::Roadlike(axis) =
            SweepFrameStyle::roadlike_default(CurveRef::Line(&rail), tol).unwrap()
        else {
            panic!()
        };
        assert!(axis.x().abs() < 1e-12);
        assert!((axis.as_vector().length().unwrap() - 1.).abs() < 1e-12);
    }
}
