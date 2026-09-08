//! Precision-drafting queries shared by interactive viewports.

mod object_snap;
pub mod plane;
pub use object_snap::{
    ObjectSnap, ObjectSnapKind, nearest_object_snap, nearest_object_snap_axis_aligned,
    nearest_object_snap_projected, nearest_object_snap_relative,
};
mod point_input;
pub use point_input::{PointInput, PointInputError};

use thiserror::Error;
use viboceros_geometry::{GeometryError, Point3, Real};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackAxis {
    Horizontal,
    Vertical,
    Both,
}

impl TrackAxis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "Horizontal",
            Self::Vertical => "Vertical",
            Self::Both => "Intersection",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrthogonalTrack {
    point: Point3,
    axis: TrackAxis,
}

impl OrthogonalTrack {
    pub const fn point(self) -> Point3 {
        self.point
    }

    pub const fn axis(self) -> TrackAxis {
        self.axis
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DraftingError {
    #[error("drafting cursor coordinates must be finite")]
    InvalidCursorCoordinates,

    #[error("drafting capture radius must be finite and strictly positive")]
    InvalidCaptureRadius,

    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Snaps a cursor to horizontal and vertical tracking lines through `anchor`.
/// If both coordinates are within the capture radius, the anchor itself wins.
pub fn orthogonal_track(
    cursor: Point3,
    anchor: Point3,
    capture_radius: Real,
) -> Result<Option<OrthogonalTrack>, DraftingError> {
    validate_capture_radius(capture_radius)?;
    let horizontal_distance = (cursor.y() - anchor.y()).abs();
    let vertical_distance = (cursor.x() - anchor.x()).abs();
    // An overflowing distance is outside the finite radius on that axis;
    // it does not invalidate a capturable perpendicular axis.
    let horizontal = horizontal_distance <= capture_radius;
    let vertical = vertical_distance <= capture_radius;
    let (point, axis) = match (horizontal, vertical) {
        (true, true) => (anchor, TrackAxis::Both),
        (true, false) => (
            Point3::try_new(cursor.x(), anchor.y(), anchor.z())?,
            TrackAxis::Horizontal,
        ),
        (false, true) => (
            Point3::try_new(anchor.x(), cursor.y(), anchor.z())?,
            TrackAxis::Vertical,
        ),
        (false, false) => return Ok(None),
    };
    Ok(Some(OrthogonalTrack { point, axis }))
}

fn validate_capture_radius(capture_radius: Real) -> Result<(), DraftingError> {
    if capture_radius.is_finite() && capture_radius > 0.0 {
        Ok(())
    } else {
        Err(DraftingError::InvalidCaptureRadius)
    }
}

fn validate_cursor_coordinates(cursor: [Real; 2]) -> Result<(), DraftingError> {
    if cursor.into_iter().all(Real::is_finite) {
        Ok(())
    } else {
        Err(DraftingError::InvalidCursorCoordinates)
    }
}

#[cfg(test)]
mod tests;
