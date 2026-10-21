//! Rounded cone control nets reuse the polygonal cone sampler.
use super::*;
use crate::{Frame3, MeshCapFaceStyle, MeshConeOptions, SubDCylinderCap, Tolerance};
impl SubD {
    /// A smooth cone with an ordinary smooth apex and a smooth optional base.
    /// Dimensions locate the control cage; the rounded limit surface stays
    /// inside it. An open base retains the native boundary crease tags.
    pub fn try_cone(
        frame: Frame3,
        radius: f64,
        height: f64,
        counts: [usize; 2],
        cap: SubDCylinderCap,
    ) -> Result<Self, GeometryError> {
        if !radius.is_finite() || !height.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD cone dimensions",
            });
        }
        if radius <= 0.
            || height <= 0.
            || frame.point_at([radius, 0., 0.])? == frame.origin()
            || frame.point_at([0., 0., height])? == frame.origin()
        {
            return Err(GeometryError::Degenerate {
                context: "SubD cone dimensions at model origin",
            });
        }
        let [around, vertical] = counts;
        let closed = cap != SubDCylinderCap::None;
        let simple_cap = around == 3 || around == 4 && cap == SubDCylinderCap::Quads;
        let cap_faces = if !closed {
            0
        } else if simple_cap {
            1
        } else if cap == SubDCylinderCap::Quads && around.is_multiple_of(2) {
            around / 2
        } else {
            around
        };
        let vertices = around
            .checked_mul(vertical)
            .and_then(|n| n.checked_add(1 + usize::from(closed && !simple_cap)));
        let faces = around
            .checked_mul(vertical)
            .and_then(|n| n.checked_add(cap_faces));
        if around < 3
            || vertical < 2
            || vertices.is_none_or(|n| n > 100000)
            || faces.is_none_or(|n| n > 100000)
        {
            return Err(viboceros_opennurbs::SubDError(
                "Invalid or excessive SubD cone topology".into(),
            )
            .into());
        }
        let apex = frame.point_at([0., 0., height])?;
        let apex_frame = Frame3::try_from_directions(
            apex,
            frame.x_axis().as_vector(),
            frame.y_axis().as_vector().scaled(-1.)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let mesh = TriangleMesh::try_cone_grid(
            apex_frame,
            radius,
            height,
            MeshConeOptions {
                around_count: around,
                vertical_count: vertical,
                solid: closed,
                cap_style: if cap == SubDCylinderCap::Quads {
                    MeshCapFaceStyle::Quadrilaterals
                } else {
                    MeshCapFaceStyle::Triangles
                },
            },
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let mesh = if closed && simple_cap {
            mesh
        } else {
            mesh.combined_identical_vertices().0
        };
        let mut points = mesh.vertices().to_vec();
        let mut faces = mesh
            .faces()
            .iter()
            .map(|f| f.indices().to_vec())
            .collect::<Vec<_>>();
        // Native SubDs use a single face for three sides, or four with Quad.
        // MeshCone's three-sided fan instead carries a redundant center.
        if closed && simple_cap {
            points.truncate(around * vertical + 1);
            faces.truncate(around * vertical);
            let offset = 1 + around * (vertical - 1);
            faces.push((offset..offset + around).map(|i| i as u32).collect());
        }
        Self::try_from_polygons(points, faces)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cone_rejects_invalid_dimensions_counts_and_lost_radii_before_allocation() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
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
            assert!(SubD::try_cone(frame, r, h, [8, 2], SubDCylinderCap::Quads).is_err());
        }
        for counts in [[2, 2], [8, 1], [usize::MAX, 2], [100000, 100000]] {
            assert!(SubD::try_cone(frame, 3., 5., counts, SubDCylinderCap::None).is_err());
        }
        assert!(
            SubD::try_cone(
                frame.with_origin(Point3::try_new(1e30, 1e30, 1e30).unwrap()),
                1.,
                5.,
                [8, 2],
                SubDCylinderCap::Quads
            )
            .is_err()
        );
    }
}
