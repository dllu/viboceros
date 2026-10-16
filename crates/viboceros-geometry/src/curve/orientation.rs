//! Supporting-plane orientation from the existing OpenNURBS curve constructor.
use super::*;

impl CurveRef<'_> {
    /// Oriented support-plane normal, or None for a nonplanar curve. Includes
    /// the OpenNURBS line-plane and closed-curve winding conventions. Only a
    /// private NURBS copy is queried; source geometry and parameters remain.
    pub fn supporting_plane_normal(
        self,
        tolerance: Tolerance,
    ) -> Result<Option<UnitVector3>, GeometryError> {
        let curve = self.to_nurbs()?;
        let controls = curve
            .control_points()
            .iter()
            .map(|v| {
                let [x, y, z] = v.point().to_array();
                [x, y, z, v.weight()]
            })
            .collect::<Vec<_>>();
        let normal = viboceros_opennurbs::curve_plane_normal(
            curve.degree(),
            &controls,
            curve.knots(),
            tolerance.absolute(),
        )?;
        normal
            .map(|v| Vector3::try_from(v)?.normalized_nonzero())
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_supporting_plane_keeps_closed_curve_orientation_and_reversal() {
        let circle = Circle3::try_from_center_point(
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let a = CurveRef::NurbsCurve(&circle)
            .supporting_plane_normal(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        let reversed = circle.reversed().unwrap();
        let b = CurveRef::NurbsCurve(&reversed)
            .supporting_plane_normal(Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(a.z() > 0.999999999999);
        assert!(b.z() < -0.999999999999);
    }
}
