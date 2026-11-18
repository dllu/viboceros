//! Final point constructions shared by command execution and viewport drafts.
use super::CircleSizeMode;
use viboceros_geometry::{Circle3, Frame3, GeometryError, Point3, Real, Tolerance};

#[derive(Clone, Copy, Debug)]
pub enum PointConstruction {
    Center {
        center: Point3,
        mode: CircleSizeMode,
    },
    Diameter {
        first: Point3,
        vertical: bool,
    },
    ThreePoint {
        first: Point3,
        second: Point3,
    },
    ThreePointRadius {
        first: Point3,
        second: Point3,
        radius: Option<Real>,
    },
    Vertical {
        center: Point3,
        radius: Option<Real>,
        mode: CircleSizeMode,
    },
    Oriented {
        center: Point3,
        normal_point: Point3,
        mode: CircleSizeMode,
    },
}

impl PointConstruction {
    pub fn circle(
        self,
        point: Point3,
        plane: Frame3,
        tolerance: Tolerance,
    ) -> Result<Circle3, GeometryError> {
        match self {
            Self::Center { center, mode } => {
                let direction = center.vector_to(point)?;
                let distance = direction.length()?;
                if measured_radius(mode) {
                    return Self::circle_in_frame(
                        plane.with_origin(center),
                        mode.radius(distance),
                        tolerance,
                    );
                }
                let frame = Frame3::try_from_x_and_normal(
                    center,
                    direction,
                    plane.z_axis().as_vector(),
                    tolerance,
                )
                .or_else(|_| {
                    Frame3::try_from_directions(
                        center,
                        direction,
                        plane.y_axis().as_vector(),
                        tolerance,
                    )
                })?;
                Self::circle_in_frame(frame, distance, tolerance)
            }
            Self::Diameter {
                first,
                vertical: false,
            } => Circle3::try_from_diameter_on_plane(first, point, plane, tolerance),
            Self::Diameter {
                first,
                vertical: true,
            } => Circle3::try_from_vertical_diameter(first, point, plane, tolerance),
            Self::ThreePoint { first, second } => {
                Circle3::try_from_three_points(first, second, point, tolerance)
            }
            Self::ThreePointRadius {
                first,
                second,
                radius,
            } => Circle3::try_from_two_points_radius_direction(
                first,
                second,
                radius.map_or_else(|| first.distance_to(point), Ok)?,
                point,
                tolerance,
            ),
            Self::Vertical {
                center,
                radius: None,
                mode,
            } if measured_radius(mode) => vertical_size(
                center,
                mode.radius(center.distance_to(point)?),
                plane,
                tolerance,
            ),
            Self::Vertical { center, radius, .. } => Circle3::try_from_vertical_direction(
                center,
                radius.map_or_else(|| center.distance_to(point), Ok)?,
                point,
                plane,
                tolerance,
            ),
            Self::Oriented {
                center,
                normal_point,
                mode,
            } => {
                let frame =
                    Frame3::try_from_normal(center, center.vector_to(normal_point)?, tolerance)?;
                if measured_radius(mode) {
                    Self::circle_in_frame(frame, mode.radius(center.distance_to(point)?), tolerance)
                } else {
                    Circle3::try_from_center_point(center, point, frame.z_axis(), tolerance)
                }
            }
        }
    }

    fn circle_in_frame(
        frame: Frame3,
        radius: Real,
        tolerance: Tolerance,
    ) -> Result<Circle3, GeometryError> {
        Circle3::try_from_frame(
            frame.origin(),
            radius,
            frame.x_axis(),
            frame.z_axis(),
            tolerance,
        )
    }
}

fn measured_radius(mode: CircleSizeMode) -> bool {
    matches!(mode, CircleSizeMode::Circumference | CircleSizeMode::Area)
}

pub(crate) fn vertical_size(
    center: Point3,
    radius: Real,
    plane: Frame3,
    tolerance: Tolerance,
) -> Result<Circle3, GeometryError> {
    let frame =
        Frame3::try_from_unit_directions(center, plane.x_axis(), plane.z_axis(), tolerance)?;
    PointConstruction::circle_in_frame(frame, radius, tolerance)
}
