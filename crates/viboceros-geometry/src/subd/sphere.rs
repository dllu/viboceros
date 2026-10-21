//! Sphere nets reuse mesh sampling and sparse native limit-point interpolation.
use super::*;
use crate::{Frame3, MeshSubdivisionSphereOptions, MeshUvSphereOptions, Tolerance};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubDSphereStyle {
    Uv { around: usize, vertical: usize },
    Quads { subdivisions: usize },
    Triangles { subdivisions: usize },
}
impl SubDSphereStyle {
    pub fn validate(self) -> Result<(), GeometryError> {
        let valid = match self {
            Self::Uv { around, vertical } => {
                around >= 3
                    && vertical >= 2
                    && around.checked_mul(vertical).is_some_and(|n| n <= 100_000)
                    && (vertical - 1)
                        .checked_mul(around)
                        .and_then(|n| n.checked_add(2))
                        .is_some_and(|n| n <= 100_000)
            }
            Self::Quads { subdivisions } => subdivisions <= 6,
            Self::Triangles { subdivisions } => subdivisions <= 5,
        };
        if valid {
            Ok(())
        } else {
            Err(
                viboceros_opennurbs::SubDError("Invalid or excessive SubD sphere topology".into())
                    .into(),
            )
        }
    }
}
impl SubD {
    /// A closed subdivision sphere from the shared sphere mesh samplers.
    /// Quad and UV nets interpolate sphere samples at the vertex limit stations.
    /// Triangular nets retain uniform control radii, with the outermost vertex
    /// limit stations on the specified sphere. The limit surface approximates
    /// a sphere between stations.
    pub fn try_sphere(
        frame: Frame3,
        radius: f64,
        style: SubDSphereStyle,
        interpolation_tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        style.validate()?;
        if !radius.is_finite() {
            return Err(GeometryError::NonFinite {
                context: "SubD sphere radius",
            });
        }
        if radius <= 0. || frame.point_at([radius, 0., 0.])? == frame.origin() {
            return Err(GeometryError::Degenerate {
                context: "SubD sphere radius at model origin",
            });
        }
        let tolerance = Tolerance::NUMERICAL_VALIDATION;
        // The SubD icosahedron has a pole and a neighboring vertex on +X.
        // Reorient the shared mesh sampler's golden-ratio seed to that frame.
        let ico_frame = if matches!(style, SubDSphereStyle::Triangles { .. }) {
            let phi = (1. + 5_f64.sqrt()) * 0.5;
            let origin = Point3::try_new(0., 0., 0.)?;
            let normal = Vector3::try_new(1., -phi, 0.)?
                .normalized_nonzero()?
                .as_vector();
            let hint = Vector3::try_new(-phi, 0., 1.)?;
            let projection = hint.dot(normal)?;
            let x = Vector3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                (-projection).mul_add(normal.to_array()[k], hint.to_array()[k])
            }))?;
            let source = Frame3::try_from_x_and_normal(origin, x, normal, tolerance)?;
            let rows = AffineTransform3::try_frame_mapping(source, frame, [1.; 3])?.linear_rows();
            Frame3::try_from_directions(
                frame.origin(),
                Vector3::try_new(rows[0][0], rows[1][0], rows[2][0])?,
                Vector3::try_new(rows[0][1], rows[1][1], rows[2][1])?,
                tolerance,
            )?
        } else {
            frame
        };
        let mesh = match style {
            SubDSphereStyle::Uv { around, vertical } => TriangleMesh::try_uv_sphere_grid(
                frame,
                radius,
                MeshUvSphereOptions {
                    around_count: around,
                    vertical_count: vertical,
                },
                tolerance,
            )?,
            SubDSphereStyle::Quads { .. } => TriangleMesh::try_quad_sphere(
                frame,
                radius,
                MeshSubdivisionSphereOptions { subdivisions: 0 },
                tolerance,
            )?,
            SubDSphereStyle::Triangles { subdivisions } => TriangleMesh::try_ico_sphere(
                ico_frame,
                radius,
                MeshSubdivisionSphereOptions { subdivisions },
                tolerance,
            )?,
        };
        let targets = mesh.vertices().to_vec();
        let control = Self::try_from_polygons(
            targets.clone(),
            mesh.faces().iter().map(|f| f.indices().to_vec()).collect(),
        )?;
        if matches!(
            style,
            SubDSphereStyle::Triangles { .. } | SubDSphereStyle::Quads { .. }
        ) {
            let outer = control.surface_points().iter().try_fold(0_f64, |r, p| {
                Ok::<_, GeometryError>(r.max(p.distance_to(frame.origin())?))
            })?;
            let scale = radius / outer;
            let points = control
                .control_points()
                .iter()
                .map(|p| {
                    frame
                        .origin()
                        .translated(frame.origin().vector_to(*p)?.scaled(scale)?)
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            let mut sphere = control.try_with_control_points(&points)?;
            if let SubDSphereStyle::Quads { subdivisions } = style {
                for _ in 0..subdivisions {
                    let sampled = sphere.tessellate(1)?;
                    let targets = sampled
                        .vertices()
                        .iter()
                        .map(|p| {
                            frame.origin().translated(
                                frame
                                    .origin()
                                    .vector_to(*p)?
                                    .normalized_nonzero()?
                                    .as_vector()
                                    .scaled(radius)?,
                            )
                        })
                        .collect::<Result<Vec<_>, GeometryError>>()?;
                    let scaffold = Self::try_from_polygons(
                        sampled.vertices().to_vec(),
                        sampled
                            .faces()
                            .iter()
                            .map(|f| f.indices().to_vec())
                            .collect(),
                    )?;
                    sphere = scaffold.try_with_limit_points(&targets, interpolation_tolerance)?;
                }
            }
            Ok(sphere)
        } else {
            control.try_with_limit_points(&targets, interpolation_tolerance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(origin: [f64; 3]) -> Frame3 {
        Frame3::try_from_normal(
            Point3::try_from(origin).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap()
    }
    #[test]
    fn invalid_sphere_topology_and_unrepresentable_radius_are_rejected() {
        for style in [
            SubDSphereStyle::Quads { subdivisions: 7 },
            SubDSphereStyle::Triangles { subdivisions: 6 },
            SubDSphereStyle::Uv {
                around: 2,
                vertical: 3,
            },
            SubDSphereStyle::Uv {
                around: 3,
                vertical: 1,
            },
            SubDSphereStyle::Uv {
                around: usize::MAX,
                vertical: usize::MAX,
            },
            SubDSphereStyle::Uv {
                around: 100000,
                vertical: 100000,
            },
        ] {
            assert!(style.validate().is_err());
        }
        let style = SubDSphereStyle::Quads { subdivisions: 0 };
        for radius in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(SubD::try_sphere(frame([0.; 3]), radius, style, Tolerance::DEFAULT).is_err());
        }
        assert!(SubD::try_sphere(frame([1e30; 3]), 1., style, Tolerance::DEFAULT).is_err());
    }
}
