//! Cache expensive arc-length features independently of camera projection.
use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::{CurveRef, NurbsCurve, Tolerance};

#[derive(Debug)]
struct Midpoints {
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    points: Vec<Point3>,
}

/// Reusable camera-independent snap data. Analytic features and indexed point
/// clouds keep their existing cheap queries; only NURBS arc-length midpoints
/// are retained. B-reps retain edge curves, not copies of their surfaces/trims.
/// Geometry and tolerance comparisons invalidate entries, including after Undo;
/// removal and conversion to another geometry type release old entries.
#[derive(Debug, Default)]
pub struct ObjectSnapCache {
    midpoints: BTreeMap<ObjectId, Midpoints>,
    #[cfg(test)]
    builds: usize,
}

impl ObjectSnapCache {
    /// Same precision, visibility and tie rules as `nearest_object_snap_axis_aligned`.
    pub fn nearest_axis_aligned(
        &mut self,
        document: &Document,
        projection: PointCloudProjection,
        origin: Point3,
        cursor_offset: [Real; 2],
        capture_radius: Real,
    ) -> Result<Option<ObjectSnap>, DraftingError> {
        validate_capture_radius(capture_radius)?;
        validate_cursor_coordinates(cursor_offset)?;
        nearest_object_snap_with_metric(
            document,
            &AxisAlignedSnapMetric {
                projection,
                origin,
                cursor_offset,
                capture_radius,
            },
            self,
        )
    }

    /// Arbitrary projection with reusable model-space arc-length feature data.
    pub fn nearest_projected(
        &mut self,
        document: &Document,
        cursor: [Real; 2],
        capture_radius: Real,
        project: impl Fn(Point3) -> Option<[Real; 2]>,
    ) -> Result<Option<ObjectSnap>, DraftingError> {
        validate_capture_radius(capture_radius)?;
        validate_cursor_coordinates(cursor)?;
        nearest_object_snap_with_metric(
            document,
            &ProjectedSnapMetric {
                cursor,
                capture_radius,
                project,
            },
            self,
        )
    }

    pub(super) fn retain_objects(&mut self, document: &Document) {
        self.midpoints.retain(|id, _| {
            document.object(*id).is_some_and(|object| {
                matches!(
                    object.geometry(),
                    Geometry::Brep(_) | Geometry::NurbsCurve(_)
                )
            })
        });
    }

    pub(super) fn midpoints<'a>(
        &mut self,
        id: ObjectId,
        curves: impl ExactSizeIterator<Item = &'a NurbsCurve> + Clone,
        tolerance: Tolerance,
    ) -> &[Point3] {
        let fresh = self.midpoints.get(&id).is_some_and(|entry| {
            entry.tolerance == tolerance
                && entry.curves.len() == curves.len()
                && entry.curves.iter().zip(curves.clone()).all(|(a, b)| a == b)
        });
        if !fresh {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            let curves: Vec<_> = curves.cloned().collect();
            let points = curves
                .iter()
                .filter_map(|curve| {
                    // DivideByCount(2,false) returns just the half-arc-length point,
                    // with no tangent requirement at a stationary midpoint.
                    CurveRef::NurbsCurve(curve)
                        .divide_by_count(2, false, tolerance)
                        .ok()?
                        .into_iter()
                        .next()
                })
                .collect();
            self.midpoints.insert(
                id,
                Midpoints {
                    curves,
                    tolerance,
                    points,
                },
            );
        }
        &self.midpoints[&id].points
    }
}

#[cfg(test)]
mod tests;
