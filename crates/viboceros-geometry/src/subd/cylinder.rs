//! Native cylinder topology and limit-radius placement.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubDCylinderCap {
    None,
    Triangles,
    Quads,
}
impl SubDCylinderCap {
    fn native(self) -> u32 {
        match self {
            Self::None => 1,
            Self::Triangles => 2,
            Self::Quads => 3,
        }
    }
}
impl SubD {
    /// Constructs along the frame's positive normal using the public native
    /// cylinder builder. Counts are around and along the axis. Odd quad caps
    /// use native triangle caps and native surface-radius placement.
    pub fn try_cylinder(
        frame: crate::Frame3,
        radius: f64,
        height: f64,
        counts: [usize; 2],
        cap: SubDCylinderCap,
    ) -> Result<Self, GeometryError> {
        if !radius.is_finite() || !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD cylinder dimensions",
            });
        }
        if radius <= 0.
            || height <= 0.
            || frame.point_at([radius, 0., 0.])? == frame.origin()
            || frame.point_at([0., 0., height])? == frame.origin()
        {
            return Err(GeometryError::Degenerate {
                context: "SubD cylinder dimensions",
            });
        }
        let [Ok(around), Ok(vertical)] = counts.map(u32::try_from) else {
            return Err(
                viboceros_opennurbs::SubDError("Invalid SubD cylinder counts".into()).into(),
            );
        };
        let basis = [
            frame.origin().to_array(),
            frame.x_axis().as_vector().to_array(),
            frame.y_axis().as_vector().to_array(),
        ];
        Self::from_native(viboceros_opennurbs::subd_cylinder(
            &basis,
            radius,
            height,
            [around, vertical],
            cap.native(),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, Tolerance};
    #[test]
    fn cylinder_admission_rejects_degenerate_dimensions_and_unbounded_allocation() {
        let frame = Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (r, h) in [
            (0., 5.),
            (-1., 5.),
            (3., 0.),
            (3., -1.),
            (f64::NAN, 5.),
            (3., f64::INFINITY),
        ] {
            assert!(SubD::try_cylinder(frame, r, h, [8, 2], SubDCylinderCap::Quads).is_err());
        }
        for counts in [[2, 2], [8, 0], [usize::MAX, 2], [100_000, 100_000]] {
            assert!(SubD::try_cylinder(frame, 3., 5., counts, SubDCylinderCap::None).is_err());
        }
        let far = frame.with_origin(Point3::try_new(1e16, 1e16, 1e16).unwrap());
        assert!(SubD::try_cylinder(far, 0.5, 5., [8, 2], SubDCylinderCap::Quads).is_err());
        assert!(SubD::try_cylinder(far, 3., 0.5, [8, 2], SubDCylinderCap::Quads).is_err());
    }
}
