//! Affine maps from point prompts, without document edits or remembered defaults.
use super::*;
use crate::plane_transforms::{plane_angle, shear_map, shear_reference_angle};

#[derive(Clone, Copy, Debug)]
pub enum PointTransform {
    Scale {
        center: Point3,
        reference: Point3,
    },
    Scale1D {
        center: Point3,
        reference: Point3,
    },
    Scale2D {
        center: Point3,
        reference: Point3,
    },
    ScaleNU {
        origin: Point3,
        reference: Option<Point3>,
        distance: Option<Real>,
        factors: [Real; 3],
        axis: usize,
        plane: Frame3,
    },
    ScalePositions {
        origin: Point3,
        reference: Option<Point3>,
        factor: Option<Real>,
        mode: crate::scale_positions::ScaleMode,
    },
    Rotate {
        center: Point3,
        reference: Point3,
    },
    Rotate3D {
        start: Point3,
        end: Point3,
        reference: Point3,
    },
    Shear {
        origin: Point3,
        reference: Point3,
    },
    Scale1DDirection {
        center: Point3,
        factor: Real,
    },
}

impl PointTransform {
    /// Display maps may be singular at a zero scale, independently of whether
    /// the document's geometry can accept that placement.
    pub fn transform_at(
        self,
        plane: Frame3,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<AffineTransform3, CommandError> {
        match self {
            Self::ScalePositions {
                origin,
                reference,
                factor,
                mode,
            } => {
                let numeric = factor.is_some();
                let (factor, direction) = if let Some(factor) = factor {
                    (factor, target)
                } else {
                    let reference =
                        reference.ok_or(CommandError::Usage(crate::scale_positions::USAGE))?;
                    (
                        crate::scale_positions::reference_factor(
                            mode, origin, reference, target, tolerance,
                        )?,
                        reference,
                    )
                };
                let map_fn = if numeric {
                    crate::scale_positions::numeric_scale_map
                } else {
                    crate::scale_positions::scale_map
                };
                Ok(
                    map_fn(mode, plane, origin, factor, Some(direction), tolerance)?
                        .unwrap_or_else(AffineTransform3::identity),
                )
            }
            Self::ScaleNU {
                origin,
                reference,
                distance,
                mut factors,
                axis,
                plane,
            } => {
                if let Some(reference) = reference {
                    *factors
                        .get_mut(axis)
                        .ok_or(CommandError::Usage(nonuniform_scale::USAGE))? =
                        nonuniform_scale::constrained_reference_factor(
                            plane, origin, axis, reference, target, distance, tolerance,
                        )?;
                }
                Ok(nonuniform_scale::scale_map(plane, origin, factors)?)
            }
            Self::Scale { center, reference } => AffineTransform3::try_uniform_scale(
                center,
                scale_factor_from_reference_allow_zero(center, reference, target, tolerance)?,
            )
            .map_err(Into::into),
            Self::Scale1D { center, reference } => AffineTransform3::try_directional_scale(
                center,
                center.vector_to(reference)?.normalized(tolerance)?,
                scale1d_factor_from_reference(center, reference, target, tolerance, true)?,
            )
            .map_err(Into::into),
            Self::Scale2D { center, reference } => {
                let factor =
                    scale_factor_from_reference_allow_zero(center, reference, target, tolerance)?;
                let frame = plane.with_origin(center);
                if factor == 1. {
                    Ok(AffineTransform3::identity())
                } else {
                    AffineTransform3::try_frame_mapping(frame, frame, [factor, factor, 1.])
                        .map_err(Into::into)
                }
            }
            Self::Rotate { center, reference } => Ok(command_rotation(
                center,
                plane.z_axis(),
                plane_angle(plane, center, reference, target, tolerance)?,
            )?),
            Self::Rotate3D {
                start,
                end,
                reference,
            } => {
                let axis = start.vector_to(end)?.normalized(tolerance)?;
                Ok(command_rotation(
                    start,
                    axis,
                    axis_rotation_angle(start, axis, reference, target, tolerance)?,
                )?)
            }
            Self::Shear { origin, reference } => Ok(shear_map(
                plane,
                origin,
                reference,
                shear_reference_angle(plane, origin, reference, target, tolerance)?,
                tolerance,
            )?),
            Self::Scale1DDirection { center, factor } => {
                Ok(AffineTransform3::try_directional_scale(
                    center,
                    center.vector_to(target)?.normalized(tolerance)?,
                    factor,
                )?)
            }
        }
    }

    /// Free Scale1D mouse targets follow the reference line; typed point
    /// targets use the same projected distance when constructing the map.
    pub fn mouse_line(self, tolerance: Tolerance) -> Option<translation::DestinationConstraint> {
        if let Self::ScaleNU {
            origin,
            axis,
            plane,
            distance,
            ..
        } = self
        {
            Some(translation::DestinationConstraint {
                anchor: origin,
                direction: Some(*plane.axes().get(axis)?),
                distance,
            })
        } else if let Self::Scale1D { center, reference }
        | Self::ScalePositions {
            origin: center,
            reference: Some(reference),
            factor: None,
            mode: crate::scale_positions::ScaleMode::OneDimensional,
        } = self
        {
            Some(translation::DestinationConstraint {
                anchor: center,
                direction: Some(
                    center
                        .vector_to(reference)
                        .ok()?
                        .normalized(tolerance)
                        .ok()?,
                ),
                distance: None,
            })
        } else {
            None
        }
    }

    /// Rotate3D free mouse angles come from the viewing line's intersection
    /// with the plane perpendicular to the chosen axis through its start.
    pub fn mouse_plane(self, tolerance: Tolerance) -> Option<(Point3, UnitVector3)> {
        if let Self::Rotate3D { start, end, .. } = self {
            Some((
                start,
                start.vector_to(end).ok()?.normalized(tolerance).ok()?,
            ))
        } else {
            None
        }
    }
}
