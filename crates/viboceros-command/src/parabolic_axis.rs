//! Axial parabola construction shared by curves and surfaces.
use viboceros_geometry::{Frame3, GeometryError, Point3, Real, Tolerance, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Construction {
    Focus,
    VertexFocus,
    VertexDirection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Dimensions {
    pub frame: Frame3,
    pub radius: Real,
    pub height: Real,
    pub focus: Point3,
    pub focal_distance: Real,
}

impl Construction {
    pub fn dimensions(
        self,
        first: Point3,
        second: Point3,
        end: Point3,
        tolerance: Tolerance,
    ) -> Result<Dimensions, GeometryError> {
        let axis_offset = first.vector_to(second)?;
        let mut axis = axis_offset.normalized(tolerance)?;
        let offset = first.vector_to(end)?;
        let axial_coordinate = offset.dot(axis.as_vector())?;
        let axial = axis.as_vector().scaled(axial_coordinate)?;
        let radial = Vector3::try_new(
            offset.x() - axial.x(),
            offset.y() - axial.y(),
            offset.z() - axial.z(),
        )?;
        let radius = radial.length()?;
        let seam = radial.normalized(tolerance)?;
        let (vertex, focus, focal_distance, height) = match self {
            Self::Focus => {
                let half_sum = 0.5 * axial_coordinate.hypot(radius) + 0.5 * axial_coordinate.abs();
                let short_leg = 0.25 * radius * (radius / half_sum);
                let (focal_distance, height) = if axial_coordinate >= 0.0 {
                    (short_leg, half_sum)
                } else {
                    (half_sum, short_leg)
                };
                let vertex = first.translated(axis.as_vector().scaled(-focal_distance)?)?;
                (vertex, first, focal_distance, height)
            }
            Self::VertexFocus => {
                let focal_distance = axis_offset.length()?;
                let height = 0.25 * radius * (radius / focal_distance);
                (first, second, focal_distance, height)
            }
            Self::VertexDirection => {
                let height = axial_coordinate.abs();
                // Captures across document tolerances use OpenNURBS' fixed
                // zero tolerance to reject an end in the vertex plane.
                const ZERO_TOLERANCE: Real = 1.0 / 4_294_967_296.0;
                if height <= ZERO_TOLERANCE {
                    return Err(GeometryError::Degenerate {
                        context: "paraboloid end is in the vertex plane",
                    });
                }
                if axial_coordinate < 0.0 {
                    axis = axis.opposite();
                }
                let focal_distance = 0.25 * radius * (radius / height);
                let focus = first.translated(axis.as_vector().scaled(focal_distance)?)?;
                (first, focus, focal_distance, height)
            }
        };
        Ok(Dimensions {
            frame: Frame3::try_from_unit_x_and_normal(vertex, seam, axis, tolerance)?,
            radius,
            height,
            focus,
            focal_distance,
        })
    }
}
