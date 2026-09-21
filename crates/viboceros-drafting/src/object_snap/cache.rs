//! Cache expensive arc-length features independently of camera projection.
use super::*;
use std::collections::BTreeMap;
use viboceros_geometry::{CurveRef, NurbsCurve, NurbsSurface, Tolerance};

#[derive(Debug)]
struct Midpoints {
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
    points: Vec<Point3>,
}

#[derive(Debug)]
struct SurfaceMidpoints {
    source: NurbsSurface,
    tolerance: Tolerance,
    points: Vec<Point3>,
}

/// Reusable camera-independent snap data. Analytic features and indexed point
/// clouds keep their existing cheap queries. Cached NURBS arc-length midpoints
/// include polycurve leaves and natural surface boundaries. B-reps retain only
/// edge curves; standalone surfaces retain their source for invalidation.
/// Geometry and tolerance comparisons invalidate entries, including after Undo;
/// removal and conversion to another geometry type release old entries.
#[derive(Debug, Default)]
pub struct ObjectSnapCache {
    midpoints: BTreeMap<ObjectId, Midpoints>,
    surfaces: BTreeMap<ObjectId, SurfaceMidpoints>,
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
                    Geometry::Brep(_) | Geometry::NurbsCurve(_) | Geometry::PolyCurve(_)
                )
            })
        });
        self.surfaces.retain(|id, _| {
            document
                .object(*id)
                .is_some_and(|object| matches!(object.geometry(), Geometry::NurbsSurface(_)))
        });
    }

    pub(super) fn midpoints<'a>(
        &mut self,
        id: ObjectId,
        curves: impl Iterator<Item = &'a NurbsCurve> + Clone,
        tolerance: Tolerance,
    ) -> &[Point3] {
        if curves.clone().next().is_none() {
            self.midpoints.remove(&id);
            return &[];
        }
        let fresh = self.midpoints.get(&id).is_some_and(|entry| {
            entry.tolerance == tolerance && entry.curves.iter().eq(curves.clone())
        });
        if !fresh {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            let curves: Vec<_> = curves.cloned().collect();
            let points = curves
                .iter()
                .filter_map(|curve| midpoint(curve, tolerance))
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

    pub(super) fn surface_midpoints(
        &mut self,
        id: ObjectId,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> &[Point3] {
        let fresh = self
            .surfaces
            .get(&id)
            .is_some_and(|entry| entry.source == *surface && entry.tolerance == tolerance);
        if !fresh {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            let u = surface.domain_u();
            let v = surface.domain_v();
            // Extract exact natural-boundary isocurves, not control-net rows:
            // periodic and unclamped surfaces need evaluation at their domains.
            // A failed/degenerate boundary cannot suppress the other features.
            let points = [
                surface.isocurve_u(*v.start()),
                surface.isocurve_v(*u.end()),
                surface.isocurve_u(*v.end()),
                surface.isocurve_v(*u.start()),
            ]
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|curve| midpoint(&curve, tolerance))
            .collect();
            self.surfaces.insert(
                id,
                SurfaceMidpoints {
                    source: surface.clone(),
                    tolerance,
                    points,
                },
            );
        }
        &self.surfaces[&id].points
    }
}

fn midpoint(curve: &NurbsCurve, tolerance: Tolerance) -> Option<Point3> {
    // No tangent requirement at a stationary midpoint.
    CurveRef::NurbsCurve(curve)
        .divide_by_count(2, false, tolerance)
        .ok()?
        .into_iter()
        .next()
}

#[cfg(test)]
mod composite_tests;
#[cfg(test)]
mod tests;
