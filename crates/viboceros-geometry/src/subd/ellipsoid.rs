//! Ellipsoid cages retain the shared rational sampler and compensate its extents.
use super::*;
use crate::{Frame3, MeshCapFaceStyle, MeshEllipsoidOptions, Tolerance};
impl SubD {
    /// A smooth rationally sampled ellipsoid cage. Scale each local direction
    /// by the control-mesh span divided by its density-two limit-mesh span.
    /// This preserves the sampled extents rather than interpolating all stations.
    pub fn try_ellipsoid(
        frame: Frame3,
        radii: [f64; 3],
        counts: [usize; 2],
        cap: MeshCapFaceStyle,
    ) -> Result<Self, GeometryError> {
        Self::ellipsoid_compensated(frame, radii, counts, cap, None)
    }
    /// Native focal construction compensates spans in the captured CPlane,
    /// scaling about that reference origin rather than the ellipsoid center.
    pub fn try_ellipsoid_in_reference_frame(
        frame: Frame3,
        radii: [f64; 3],
        counts: [usize; 2],
        cap: MeshCapFaceStyle,
        reference: Frame3,
    ) -> Result<Self, GeometryError> {
        Self::ellipsoid_compensated(frame, radii, counts, cap, Some(reference))
    }
    fn ellipsoid_compensated(
        frame: Frame3,
        radii: [f64; 3],
        counts: [usize; 2],
        cap: MeshCapFaceStyle,
        reference: Option<Frame3>,
    ) -> Result<Self, GeometryError> {
        let world = reference.is_some();
        let [around, vertical] = counts;
        if around < 3
            || vertical < 2
            || around.checked_mul(vertical).is_none_or(|n| n > 100_000)
            || (vertical - 1)
                .checked_mul(around)
                .and_then(|n| n.checked_add(2))
                .is_none_or(|n| n > 100_000)
        {
            return Err(viboceros_opennurbs::SubDError(
                "Invalid or excessive SubD ellipsoid topology".into(),
            )
            .into());
        }
        if radii.iter().any(|r| !r.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "SubD ellipsoid radii",
            });
        }
        if radii.iter().any(|r| *r <= 0.) {
            return Err(GeometryError::Degenerate {
                context: "SubD ellipsoid radii",
            });
        }
        for k in 0..3 {
            let mut p = [0.; 3];
            p[k] = radii[k];
            if frame.point_at(p)? == frame.origin() {
                return Err(GeometryError::Degenerate {
                    context: "SubD ellipsoid dimensions at model origin",
                });
            }
        }
        let unit = Frame3::try_from_directions(
            frame.origin(),
            Vector3::try_new(1., 0., 0.)?,
            Vector3::try_new(0., 1., 0.)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let sampling_frame = if let Some(reference) = reference {
            let axes = [reference.x_axis(), reference.y_axis(), reference.z_axis()];
            let coordinates = |v: Vector3| -> Result<Vector3, GeometryError> {
                Vector3::try_new(
                    v.dot(axes[0].as_vector())?,
                    v.dot(axes[1].as_vector())?,
                    v.dot(axes[2].as_vector())?,
                )
            };
            Frame3::try_from_directions(
                Point3::try_from(reference.coordinates_of(frame.origin())?)?,
                coordinates(frame.x_axis().as_vector())?,
                coordinates(frame.y_axis().as_vector())?,
                Tolerance::NUMERICAL_VALIDATION,
            )?
        } else {
            unit
        };
        let mesh = TriangleMesh::try_ellipsoid_grid(
            sampling_frame,
            radii,
            MeshEllipsoidOptions {
                around_count: around,
                vertical_count: vertical,
                cap_style: cap,
            },
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let cage = Self::try_from_polygons(
            mesh.vertices().to_vec(),
            mesh.faces().iter().map(|f| f.indices().to_vec()).collect(),
        )?;
        let limit = cage.tessellate(if world { 3 } else { 2 })?;
        let spans = |points: &[Point3], float: bool| -> Result<[f64; 3], GeometryError> {
            let points = points
                .iter()
                .map(|p| {
                    if float {
                        Point3::try_from(p.to_array().map(|x| x as f32 as f64))
                    } else {
                        Ok(*p)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let bounds = BoundingBox3::from_points(points)?;
            let lo = bounds.min().to_array();
            let hi = bounds.max().to_array();
            Ok(std::array::from_fn(|k| hi[k] - lo[k]))
        };
        // Identity placement retains the sampler's double bounds. A placed
        // native mesh refreshes its source bounds from float vertices.
        let float_bounds =
            !world && (frame.origin() != Point3::try_new(0., 0., 0.)? || frame != unit);
        let control = spans(mesh.vertices(), float_bounds)?;
        let smooth = spans(limit.vertices(), false)?;
        if smooth
            .iter()
            .chain(control.iter())
            .any(|s| !s.is_finite() || *s <= 0.)
        {
            return Err(GeometryError::Degenerate {
                context: "SubD ellipsoid limit extents",
            });
        }
        let scale: [f64; 3] = std::array::from_fn(|k| control[k] / smooth[k]);
        let points = cage
            .control_points()
            .iter()
            .map(|p| {
                if let Some(reference) = reference {
                    reference.point_at(std::array::from_fn(|k| p.to_array()[k] * scale[k]))
                } else {
                    let p = unit.coordinates_of(*p)?;
                    frame.point_at(std::array::from_fn(|k| p[k] * scale[k]))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        cage.try_with_control_points(&points)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_cages_scale_with_their_semi_axes_without_bound_rounding_drift() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for counts in [[3, 2], [7, 5], [10, 10]] {
            for cap in [
                MeshCapFaceStyle::Triangles,
                MeshCapFaceStyle::Quadrilaterals,
            ] {
                let unit = SubD::try_ellipsoid(frame, [1.; 3], counts, cap).unwrap();
                let radii = [20., 3., 2.];
                let scaled = SubD::try_ellipsoid(frame, radii, counts, cap).unwrap();
                assert_eq!(unit.faces(), scaled.faces());
                assert_eq!(unit.edge_tags(), scaled.edge_tags());
                for (a, b) in unit.control_points().iter().zip(scaled.control_points()) {
                    let expected =
                        Point3::try_from(std::array::from_fn(|k| a.to_array()[k] * radii[k]))
                            .unwrap();
                    assert!(expected.distance_to(*b).unwrap() < 1e-11);
                }
            }
        }
    }
    #[test]
    fn rejects_invalid_dimensions_counts_and_lost_world_dimensions_before_sampling() {
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for counts in [[2, 3], [3, 1], [usize::MAX, 3], [100000, 100000]] {
            assert!(
                SubD::try_ellipsoid(frame, [5., 3., 2.], counts, MeshCapFaceStyle::Triangles)
                    .is_err()
            );
        }
        for r in [
            [0., 1., 1.],
            [1., -1., 1.],
            [1., 1., f64::NAN],
            [f64::INFINITY, 1., 1.],
        ] {
            assert!(SubD::try_ellipsoid(frame, r, [8, 4], MeshCapFaceStyle::Triangles).is_err());
        }
        assert!(
            SubD::try_ellipsoid(
                frame.with_origin(Point3::try_new(1e30, 1e30, 1e30).unwrap()),
                [1.; 3],
                [8, 4],
                MeshCapFaceStyle::Triangles
            )
            .is_err()
        );
    }
}
