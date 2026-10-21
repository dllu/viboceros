use super::*;

impl SubD {
    /// A smooth box on an orthonormal frame, via OpenNURBS.
    /// Counts describe the three box-edge directions; the native record retains
    /// its face packing and original topology independently of display meshes.
    pub fn try_box(
        frame: crate::Frame3,
        dimensions: [f64; 3],
        counts: [usize; 3],
    ) -> Result<Self, GeometryError> {
        if dimensions.iter().any(|d| !d.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "SubD box dimensions",
            });
        }
        if dimensions.iter().any(|d| *d <= 0.) {
            return Err(GeometryError::Degenerate {
                context: "SubD box dimensions",
            });
        }
        let [x, y, z] = dimensions;
        let mut corners = [[0.; 3]; 8];
        for (i, p) in [
            [0., 0., 0.],
            [x, 0., 0.],
            [x, y, 0.],
            [0., y, 0.],
            [0., 0., z],
            [x, 0., z],
            [x, y, z],
            [0., y, z],
        ]
        .into_iter()
        .enumerate()
        {
            corners[i] = frame.point_at(p)?.to_array();
        }
        if [1, 3, 4].iter().any(|&i| corners[0] == corners[i]) {
            return Err(GeometryError::Degenerate {
                context: "SubD box dimensions at the model origin",
            });
        }
        let counts = counts.map(u32::try_from);
        let [Ok(x), Ok(y), Ok(z)] = counts else {
            return Err(
                viboceros_opennurbs::SubDError("Invalid SubD box face count".into()).into(),
            );
        };
        Self::from_native(viboceros_opennurbs::subd_box(&corners, [x, y, z])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Frame3, Tolerance};
    fn frame() -> Frame3 {
        Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(1., 0., 0.).unwrap(),
            Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn box_constructor_rejects_lost_dimensions_and_excessive_topology() {
        for dimensions in [
            [0., 3., 5.],
            [-1., 3., 5.],
            [f64::NAN, 3., 5.],
            [f64::INFINITY, 3., 5.],
        ] {
            assert!(SubD::try_box(frame(), dimensions, [2; 3]).is_err());
        }
        for counts in [[0, 2, 2], [100_000; 3], [usize::MAX, 1, 1]] {
            assert!(SubD::try_box(frame(), [4., 3., 5.], counts).is_err());
        }
        let translated = frame().with_origin(Point3::try_new(1e16, 1e16, 1e16).unwrap());
        assert!(SubD::try_box(translated, [0.5, 3., 5.], [2; 3]).is_err());
    }
}
