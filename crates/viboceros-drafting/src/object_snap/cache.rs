//! Cache expensive arc-length and polygon Center features independently of projection.
use super::*;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use viboceros_geometry::{CurveRef, CurveSegment3, NurbsCurve, NurbsSurface, Tolerance};

/// One source snapshot shared by independently lazy, camera-free features.
/// Failed integrations/recognitions are cached as well as successful targets.
#[derive(Debug)]
pub(super) struct CurveFeatures {
    pub(super) curve: NurbsCurve,
    pub(super) bounds: Option<viboceros_geometry::BoundingBox3>,
    tolerance: Tolerance,
    midpoint: OnceCell<Option<Point3>>,
    circular_center: OnceCell<Option<Point3>>,
}

impl CurveFeatures {
    fn new(curve: NurbsCurve, tolerance: Tolerance) -> Self {
        let sign = curve.control_points()[0].weight().is_sign_positive();
        let bounds = curve
            .control_points()
            .iter()
            .all(|p| p.weight().is_sign_positive() == sign)
            .then(|| curve.control_point_bounds());
        Self {
            curve,
            bounds,
            tolerance,
            midpoint: OnceCell::new(),
            circular_center: OnceCell::new(),
        }
    }

    pub(super) fn midpoint(&self) -> Option<Point3> {
        *self
            .midpoint
            .get_or_init(|| midpoint(&self.curve, self.tolerance))
    }

    pub(super) fn circular_center(&self) -> Option<Point3> {
        *self.circular_center.get_or_init(|| {
            // Bound cold UI recognition for pathological high-degree inputs.
            // The kernel itself imposes no such feature-discovery degree cap.
            (self.curve.degree() <= 32)
                .then(|| self.curve.circular_center(self.tolerance).ok().flatten())
                .flatten()
        })
    }
}

#[derive(Debug)]
struct CachedCurves {
    tolerance: Tolerance,
    features: Vec<CurveFeatures>,
}

#[derive(Debug)]
struct SurfaceCurves {
    source: NurbsSurface,
    tolerance: Tolerance,
    features: Vec<CurveFeatures>,
}

/// Reusable camera-independent snap data. Analytic features and indexed point
/// clouds keep their existing cheap queries. Cached NURBS sources lazily compute
/// arc-length Mid and circular Center, including polycurve leaves and natural
/// surface boundaries. Their B-rep entries
/// retain only edge curves; standalone surfaces retain extracted boundaries and
/// their source for invalidation. Polygon Center entries additionally retain
/// relevant planar-face sources, but not UV trims. Failed recognitions are cached.
/// Common-sign control bounds accelerate curve-hover queries.
/// Geometry and tolerance comparisons invalidate entries, including after Undo;
/// removal and conversion to another geometry type release old entries.
#[derive(Debug, Default)]
pub struct ObjectSnapCache {
    curves: BTreeMap<ObjectId, CachedCurves>,
    surfaces: BTreeMap<ObjectId, SurfaceCurves>,
    pub(super) polygons: super::polygon_centers::Cache,
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
        self.nearest_axis_aligned_with_modes(
            document,
            projection,
            origin,
            cursor_offset,
            capture_radius,
            ObjectSnapModes::ALL,
        )
    }

    /// Axis-aligned query with an explicit enabled-feature set.
    pub fn nearest_axis_aligned_with_modes(
        &mut self,
        document: &Document,
        projection: PointCloudProjection,
        origin: Point3,
        cursor_offset: [Real; 2],
        capture_radius: Real,
        modes: ObjectSnapModes,
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
            modes,
        )
    }

    /// Affine/projective viewport query with reusable model-space feature data.
    /// Uses the projection contract of `nearest_object_snap_projected`.
    pub fn nearest_projected(
        &mut self,
        document: &Document,
        cursor: [Real; 2],
        capture_radius: Real,
        project: impl Fn(Point3) -> Option<[Real; 2]>,
    ) -> Result<Option<ObjectSnap>, DraftingError> {
        self.nearest_projected_with_modes(
            document,
            cursor,
            capture_radius,
            project,
            ObjectSnapModes::ALL,
        )
    }

    /// Projected query with an explicit enabled-feature set. Center and Mid-only
    /// capture use bounded curve-proximity queries, not just target proximity.
    /// Uses the affine/projective and clipping contract of
    /// [`nearest_object_snap_projected`].
    pub fn nearest_projected_with_modes(
        &mut self,
        document: &Document,
        cursor: [Real; 2],
        capture_radius: Real,
        project: impl Fn(Point3) -> Option<[Real; 2]>,
        modes: ObjectSnapModes,
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
            modes,
        )
    }

    pub(super) fn retain_objects(&mut self, document: &Document) {
        self.polygons.retain_objects(document);
        self.curves.retain(|id, _| {
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

    pub(super) fn geometry_curves(
        &mut self,
        id: ObjectId,
        geometry: &Geometry,
        tolerance: Tolerance,
    ) -> &[CurveFeatures] {
        match geometry {
            Geometry::NurbsCurve(curve) => self.curves(id, std::iter::once(curve), tolerance),
            Geometry::PolyCurve(curve) => self.curves(
                id,
                curve.segments().iter().filter_map(|s| match s {
                    CurveSegment3::NurbsCurve(c) => Some(c),
                    _ => None,
                }),
                tolerance,
            ),
            Geometry::NurbsSurface(surface) => self.surface_curves(id, surface, tolerance),
            Geometry::Brep(brep) => {
                self.curves(id, brep.edges().iter().map(|e| e.curve()), tolerance)
            }
            _ => &[],
        }
    }

    pub(super) fn curves<'a>(
        &mut self,
        id: ObjectId,
        curves: impl Iterator<Item = &'a NurbsCurve> + Clone,
        tolerance: Tolerance,
    ) -> &[CurveFeatures] {
        if curves.clone().next().is_none() {
            self.curves.remove(&id);
            return &[];
        }
        let fresh = self.curves.get(&id).is_some_and(|entry| {
            entry.tolerance == tolerance
                && entry.features.iter().map(|f| &f.curve).eq(curves.clone())
        });
        if !fresh {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            let features = curves
                .cloned()
                .map(|curve| CurveFeatures::new(curve, tolerance))
                .collect();
            self.curves.insert(
                id,
                CachedCurves {
                    tolerance,
                    features,
                },
            );
        }
        &self.curves[&id].features
    }

    pub(super) fn surface_curves(
        &mut self,
        id: ObjectId,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> &[CurveFeatures] {
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
            let features = [
                surface.isocurve_u(*v.start()),
                surface.isocurve_v(*u.end()),
                surface.isocurve_u(*v.end()),
                surface.isocurve_v(*u.start()),
            ]
            .into_iter()
            .filter_map(Result::ok)
            .map(|curve| CurveFeatures::new(curve, tolerance))
            .collect();
            self.surfaces.insert(
                id,
                SurfaceCurves {
                    source: surface.clone(),
                    tolerance,
                    features,
                },
            );
        }
        &self.surfaces[&id].features
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
mod circular_tests;
#[cfg(test)]
mod composite_tests;
#[cfg(test)]
mod tests;
