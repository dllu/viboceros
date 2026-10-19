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
        self.with_transformed_grips_in_mode(indices, transform, tolerance, GripMode::Control)
    }

    /// Reconstruct edit grips in the original curve basis; control grips retain
    /// their existing direct control-net editing behavior.
    pub fn with_transformed_grips_in_mode(
        &self,
        indices: &BTreeSet<usize>,
        transform: AffineTransform3,
        tolerance: Tolerance,
        mode: GripMode,
    ) -> Result<Self, GeometryError> {
        self.with_grip_map(
            indices,
            |point| transform.transform_point(point),
            tolerance,
            mode,
            transform == AffineTransform3::identity(),
        )
    }

    /// Move control or Greville edit grips through a point map without fitting
    /// the whole object. Unselected locations, source knots and weights remain fixed.
    pub fn with_morphed_grips_in_mode(
        &self,
        indices: &BTreeSet<usize>,
        morph: &(impl viboceros_geometry::PointMorph + ?Sized),
        tolerance: Tolerance,
        mode: GripMode,
    ) -> Result<Self, GeometryError> {
        self.with_grip_map(
            indices,
            |point| morph.morph_point(point),
            tolerance,
            mode,
            false,
        )
    }

    fn with_grip_map(
        &self,
        indices: &BTreeSet<usize>,
        map_point: impl Fn(Point3) -> Result<Point3, GeometryError>,
        tolerance: Tolerance,
        mode: GripMode,
        identity: bool,
    ) -> Result<Self, GeometryError> {
        if mode == GripMode::Edit {
            let curve = self
                .curve_ref()
                .ok_or(GeometryError::UnsupportedControlPointEdit {
                    context: "edit points require a curve",
                })?
                .to_nurbs()?;
            let mut points = curve
                .edit_points()?
                .into_iter()
                .map(|p| p.point)
                .collect::<Vec<_>>();
            let count = curve.grip_count()?.min(points.len());
            if let Some(&index) = indices.last().filter(|&&index| index >= count) {
                return Err(GeometryError::InvalidControlPointIndex { index, count });
            }
            if indices.is_empty() || identity {
                return Ok(self.clone());
            }
            for &index in indices {
                points[index] = map_point(points[index])?;
            }
            // The closed end station shares the first displayed grip. The
            // interpolation system still needs both endpoint equations.
            for index in count..points.len() {
                points[index] = points[index % count];
            }
            return Ok(Self::NurbsCurve(
                curve.try_with_edit_points(&points, tolerance)?,
            ));
        }
        let count = match self {
            Self::NurbsCurve(curve) => curve.grip_count()?,
            Self::NurbsSurface(surface) => surface.grip_dimensions().into_iter().product(),
            Self::Mesh(mesh) => mesh.vertices().len(),
            _ => self.grip_locations(GripMode::Control)?.len(),
        };
        if let Some(&index) = indices.last().filter(|&&index| index >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices.is_empty() || identity {
            return Ok(self.clone());
        }
        let map = |points: &[Point3], closed: bool| -> Result<Vec<Point3>, GeometryError> {
            let unique = points.len() - usize::from(closed);
            let mapped = points[..unique]
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    if indices.contains(&i) {
                        map_point(p)
                    } else {
                        Ok(p)
                    }
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            if mapped == points[..unique] {
                return Ok(points.to_vec());
            }
            Ok((0..points.len())
                .map(|i| mapped[i % unique])
                .collect::<Vec<_>>())
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
                let result = curve.with_mapped_grips(indices, &map_point)?;
                if result != *curve
                    && result.degree() == 1
                    && !result.is_rational()
                    && result.knots()[1..result.knots().len() - 1]
                        .windows(2)
                        .all(|p| p[0] < p[1])
                {
                    Self::Polyline(Polyline3::try_with_parameters(
                        result.control_points().iter().map(|c| c.point()).collect(),
                        result.knots()[1..result.knots().len() - 1].to_vec(),
                        tolerance,
                    )?)
                } else {
                    Self::NurbsCurve(result)
                }
            }
            Self::PolyCurve(curve) => {
                Self::PolyCurve(curve.with_mapped_grips(indices, &map_point)?)
            }
            Self::Circle(_) | Self::Arc(_) | Self::Ellipse(_) => Self::NurbsCurve(
                self.curve_ref()
                    .unwrap()
                    .to_nurbs()?
                    .with_mapped_grips(indices, &map_point)?,
            ),
            Self::NurbsSurface(surface) => {
                Self::NurbsSurface(surface.with_mapped_grips(indices, &map_point)?)
            }
            Self::Brep(brep) if brep.faces().len() == 1 => {
                let face = &brep.faces()[0];
                let bounds = face.rectangular_trim_bounds(tolerance)?.ok_or(
                    GeometryError::UnsupportedControlPointEdit {
                        context: "nonrectangular trimmed surfaces",
                    },
                )?;
                Self::Brep(Brep::try_rectangular_surface_face_with_orientation(
                    face.surface().with_mapped_grips(indices, &map_point)?,
                    bounds[0][0]..=bounds[0][1],
                    bounds[1][0]..=bounds[1][1],
                    face.is_reversed(),
                    tolerance,
                )?)
            }
            Self::Mesh(mesh) => {
                Self::Mesh(mesh.try_with_edited_vertices(map(mesh.vertices(), false)?)?)
            }
            _ => {
                return Err(GeometryError::UnsupportedControlPointEdit {
                    context: "this geometry type",
                });
            }
        })
    }
}
