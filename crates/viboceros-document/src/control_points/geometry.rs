//! Exact source control edits, independent of document display or history.
use super::*;
use viboceros_geometry::{Brep, Polyline3};

impl Geometry {
    pub fn with_transformed_grips(
        &self,
        indices: &BTreeSet<usize>,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let count = match self {
            Self::NurbsCurve(curve) => curve.grip_count()?,
            Self::NurbsSurface(surface) => surface.grip_dimensions().into_iter().product(),
            Self::Mesh(mesh) => mesh.vertices().len(),
            _ => self.grip_locations()?.len(),
        };
        if let Some(&index) = indices.last().filter(|&&index| index >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        let map = |points: &[Point3], closed: bool| {
            let unique = points.len() - usize::from(closed);
            points
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    if indices.contains(&(index % unique)) {
                        transform.transform_point(*point)
                    } else {
                        Ok(*point)
                    }
                })
                .collect::<Result<Vec<_>, GeometryError>>()
        };
        Ok(match self {
            Self::Line(line) => Self::Polyline(Polyline3::try_with_parameters(
                map(&[line.start(), line.end()], false)?,
                vec![*line.domain().start(), *line.domain().end()],
                tolerance,
            )?),
            Self::Polyline(curve) => Self::Polyline(Polyline3::try_with_parameters(
                map(curve.vertices(), curve.is_closed())?,
                curve.parameters().to_vec(),
                tolerance,
            )?),
            Self::NurbsCurve(curve) => {
                Self::NurbsCurve(curve.with_transformed_grips(indices, transform)?)
            }
            Self::Circle(_) | Self::Arc(_) | Self::Ellipse(_) => Self::NurbsCurve(
                self.curve_ref()
                    .unwrap()
                    .to_nurbs()?
                    .with_transformed_grips(indices, transform)?,
            ),
            Self::NurbsSurface(surface) => {
                Self::NurbsSurface(surface.with_transformed_grips(indices, transform)?)
            }
            Self::Brep(brep) if brep.faces().len() == 1 => {
                let face = &brep.faces()[0];
                let bounds = face.rectangular_trim_bounds(tolerance)?.ok_or(
                    GeometryError::UnsupportedControlPointEdit {
                        context: "nonrectangular trimmed surfaces",
                    },
                )?;
                Self::Brep(Brep::try_rectangular_surface_face_with_orientation(
                    face.surface().with_transformed_grips(indices, transform)?,
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    face.is_reversed(),
                    tolerance,
                )?)
            }
            Self::Mesh(mesh) => {
                Self::Mesh(mesh.try_with_mapped_vertices(map(mesh.vertices(), false)?, tolerance)?)
            }
            _ => {
                return Err(GeometryError::UnsupportedControlPointEdit {
                    context: "this geometry type",
                });
            }
        })
    }
}
