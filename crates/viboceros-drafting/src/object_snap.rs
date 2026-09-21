//! Visible-feature snap enumeration, projection metrics, and priority ordering.
mod cache;
mod centers;
mod features;
mod mid_hover;
mod polygon_centers;
mod proximity;
pub use cache::ObjectSnapCache;

use super::{DraftingError, validate_capture_radius, validate_cursor_coordinates};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{GeometryError, Point3, PointCloud3, PointCloudProjection, Real};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSnapKind {
    Point,
    End,
    Mid,
    Center,
    Quad,
}

/// Enabled feature kinds, independent of the UI's persistent/one-shot lifetime.
/// Defaults to no enabled features.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectSnapModes(u8);

impl ObjectSnapModes {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(0b1_1111);

    pub const fn only(kind: ObjectSnapKind) -> Self {
        Self(1 << kind.priority())
    }
    pub const fn contains(self, kind: ObjectSnapKind) -> bool {
        self.0 & Self::only(kind).0 != 0
    }
    pub const fn with(self, kind: ObjectSnapKind, enabled: bool) -> Self {
        if enabled {
            Self(self.0 | Self::only(kind).0)
        } else {
            Self(self.0 & !Self::only(kind).0)
        }
    }
}

impl ObjectSnapKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::End => "End",
            Self::Mid => "Mid",
            Self::Center => "Center",
            Self::Quad => "Quad",
        }
    }

    const fn priority(self) -> u8 {
        match self {
            Self::Point => 0,
            Self::End => 1,
            Self::Mid => 2,
            Self::Center => 3,
            Self::Quad => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectSnap {
    point: Point3,
    kind: ObjectSnapKind,
    object_id: ObjectId,
    distance: Real,
}

impl ObjectSnap {
    pub const fn point(self) -> Point3 {
        self.point
    }

    pub const fn kind(self) -> ObjectSnapKind {
        self.kind
    }

    pub const fn object_id(self) -> ObjectId {
        self.object_id
    }

    /// Capture distance in the query's projection: to the point feature, or
    /// to the source curve for hover-derived Center or Mid-only snaps.
    pub const fn distance(self) -> Real {
        self.distance
    }
}

/// Finds the closest visible feature snap in the top-view XY projection.
/// Locked objects remain snap targets, matching Rhino. Exact-distance ties use
/// the stable priority encoded by [`ObjectSnapKind`].
pub fn nearest_object_snap(
    document: &Document,
    cursor: Point3,
    capture_radius: Real,
) -> Result<Option<ObjectSnap>, DraftingError> {
    nearest_object_snap_relative(document, cursor, [0.0; 2], capture_radius)
}

/// Finds XY feature snaps using `(candidate - origin) - cursor_offset`.
/// The separate local offset retains cursor precision at large world origins.
/// Radius and returned distance are in model units; point clouds retain their
/// indexed search, and visibility, locking, and tie rules match [`nearest_object_snap`].
pub fn nearest_object_snap_relative(
    document: &Document,
    origin: Point3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
) -> Result<Option<ObjectSnap>, DraftingError> {
    nearest_object_snap_axis_aligned(
        document,
        PointCloudProjection::Xy,
        origin,
        cursor_offset,
        capture_radius,
    )
}

/// Finds axis-aligned feature snaps in model units, retaining local cursor
/// precision and indexed point-cloud queries in XY, XZ, or YZ.
pub fn nearest_object_snap_axis_aligned(
    document: &Document,
    projection: PointCloudProjection,
    origin: Point3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
) -> Result<Option<ObjectSnap>, DraftingError> {
    ObjectSnapCache::default().nearest_axis_aligned(
        document,
        projection,
        origin,
        cursor_offset,
        capture_radius,
    )
}

/// Finds the closest visible feature after mapping candidates into an
/// affine or projective viewport projection. The capture radius and the
/// returned distance use the same units as `cursor` and `project`.
/// `project` must reject points behind its camera/clipping plane. Hover broad
/// phase bounds rely on the projection preserving convexity in the visible half-space.
pub fn nearest_object_snap_projected(
    document: &Document,
    cursor: [Real; 2],
    capture_radius: Real,
    project: impl Fn(Point3) -> Option<[Real; 2]>,
) -> Result<Option<ObjectSnap>, DraftingError> {
    ObjectSnapCache::default().nearest_projected(document, cursor, capture_radius, project)
}

trait SnapMetric {
    fn capture_radius(&self) -> Real;
    fn offset(&self, point: Point3) -> Option<[Real; 2]>;
    fn distance(&self, point: Point3) -> Option<Real> {
        let [x, y] = self.offset(point)?;
        let d = x.hypot(y);
        d.is_finite().then_some(d)
    }
    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError>;
}

struct AxisAlignedSnapMetric {
    projection: PointCloudProjection,
    origin: Point3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
}

impl SnapMetric for AxisAlignedSnapMetric {
    fn capture_radius(&self) -> Real {
        self.capture_radius
    }

    fn offset(&self, point: Point3) -> Option<[Real; 2]> {
        let project = |p: Point3| match self.projection {
            PointCloudProjection::Xy => [p.x(), p.y()],
            PointCloudProjection::Xz => [p.x(), p.z()],
            PointCloudProjection::Yz => [p.y(), p.z()],
        };
        let p = project(point);
        let origin = project(self.origin);
        let delta = [
            (p[0] - origin[0]) - self.cursor_offset[0],
            (p[1] - origin[1]) - self.cursor_offset[1],
        ];
        delta.iter().all(|v| v.is_finite()).then_some(delta)
    }

    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError> {
        Ok(cloud
            .nearest_projected_relative(
                self.projection,
                self.origin,
                self.cursor_offset,
                self.capture_radius,
            )?
            .map(|(_, point, _)| point))
    }
}

struct ProjectedSnapMetric<F> {
    cursor: [Real; 2],
    capture_radius: Real,
    project: F,
}

impl<F> SnapMetric for ProjectedSnapMetric<F>
where
    F: Fn(Point3) -> Option<[Real; 2]>,
{
    fn capture_radius(&self) -> Real {
        self.capture_radius
    }

    fn offset(&self, point: Point3) -> Option<[Real; 2]> {
        let projected = (self.project)(point)?;
        let delta = [projected[0] - self.cursor[0], projected[1] - self.cursor[1]];
        delta.iter().all(|v| v.is_finite()).then_some(delta)
    }

    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError> {
        Ok(cloud
            .points()
            .iter()
            .copied()
            .filter_map(|point| self.distance(point).map(|distance| (distance, point)))
            .filter(|(distance, _)| *distance <= self.capture_radius)
            .min_by(|(first, _), (second, _)| first.total_cmp(second))
            .map(|(_, point)| point))
    }
}

fn nearest_object_snap_with_metric(
    document: &Document,
    metric: &impl SnapMetric,
    cache: &mut ObjectSnapCache,
    modes: ObjectSnapModes,
) -> Result<Option<ObjectSnap>, DraftingError> {
    // Suspension is O(1), including with large surface/B-rep documents. Cache
    // cleanup resumes on the next enabled query; public input validation still runs.
    if modes == ObjectSnapModes::NONE {
        return Ok(None);
    }
    cache.retain_objects(document);
    let mut best = None;
    for object in document.objects() {
        let attributes = object.attributes();
        let Some(layer) = document.layer(attributes.layer_id()) else {
            continue;
        };
        if !attributes.is_visible() || !layer.is_visible() {
            continue;
        }
        let mut object_best = None;
        if modes == ObjectSnapModes::only(ObjectSnapKind::Mid) {
            mid_hover::visit(
                object.geometry(),
                object.id(),
                document.tolerance(),
                cache,
                metric,
                &mut |point, distance| {
                    consider_scored_candidate(
                        &mut best,
                        object.id(),
                        ObjectSnapKind::Mid,
                        point,
                        distance,
                    )
                },
            );
            continue;
        }
        let mut emit = |kind, point| {
            if !modes.contains(kind) {
                return;
            }
            consider_candidate(
                &mut object_best,
                metric,
                metric.capture_radius(),
                object.id(),
                kind,
                point,
            );
        };
        match object.geometry() {
            Geometry::Point(point) => emit(ObjectSnapKind::Point, *point),
            Geometry::PointCloud(cloud) => {
                if modes.contains(ObjectSnapKind::Point)
                    && let Some(point) = metric.nearest_point_cloud(cloud)?
                {
                    emit(ObjectSnapKind::Point, point);
                }
            }
            Geometry::Line(_)
            | Geometry::Circle(_)
            | Geometry::Arc(_)
            | Geometry::Ellipse(_)
            | Geometry::Polyline(_)
            | Geometry::NurbsCurve(_)
            | Geometry::PolyCurve(_) => {
                let curve = object
                    .geometry()
                    .curve_ref()
                    .expect("matched a curve geometry");
                features::curve(curve, &mut emit);
                // Analytic leaf features are cheap. Cache only the expensive
                // NURBS integrations, together under the owning object's ID.
                let midpoints = if !modes.contains(ObjectSnapKind::Mid) {
                    &[][..]
                } else {
                    match curve {
                        viboceros_geometry::CurveRef::NurbsCurve(curve) => cache.midpoints(
                            object.id(),
                            std::iter::once(curve),
                            document.tolerance(),
                        ),
                        viboceros_geometry::CurveRef::PolyCurve(curve) => cache.midpoints(
                            object.id(),
                            curve.segments().iter().filter_map(|segment| match segment {
                                viboceros_geometry::CurveSegment3::NurbsCurve(curve) => Some(curve),
                                _ => None,
                            }),
                            document.tolerance(),
                        ),
                        _ => &[],
                    }
                };
                for point in midpoints.iter().filter_map(|feature| feature.point) {
                    emit(ObjectSnapKind::Mid, point);
                }
            }
            Geometry::NurbsSurface(surface) => {
                let u = surface.domain_u();
                let v = surface.domain_v();
                for (u, v) in [
                    (*u.start(), *v.start()),
                    (*u.end(), *v.start()),
                    (*u.end(), *v.end()),
                    (*u.start(), *v.end()),
                ] {
                    if let Ok(point) = surface.evaluate(u, v) {
                        emit(ObjectSnapKind::End, point);
                    }
                }
                // Mid belongs to each natural boundary, never to the UV center.
                if modes.contains(ObjectSnapKind::Mid) {
                    for point in cache
                        .surface_midpoints(object.id(), surface, document.tolerance())
                        .iter()
                        .filter_map(|feature| feature.point)
                    {
                        emit(ObjectSnapKind::Mid, point);
                    }
                }
            }
            Geometry::Brep(brep) => {
                for vertex in brep.vertices() {
                    emit(ObjectSnapKind::End, vertex.point());
                }
                if modes.contains(ObjectSnapKind::Mid) {
                    for point in cache
                        .midpoints(
                            object.id(),
                            brep.edges().iter().map(|edge| edge.curve()),
                            document.tolerance(),
                        )
                        .iter()
                        .filter_map(|feature| feature.point)
                    {
                        emit(ObjectSnapKind::Mid, point);
                    }
                }
            }
            // Mesh features need a spatial index rather than an O(vertices)
            // walk per pointer frame.
            Geometry::Mesh(_) => {}
        }
        // Direct features suppress Center on the same object, not on every
        // object in the document. Across objects, compare capture distance.
        if object_best.is_none() && modes.contains(ObjectSnapKind::Center) {
            let mut center = |point, distance| {
                consider_scored_candidate(
                    &mut object_best,
                    object.id(),
                    ObjectSnapKind::Center,
                    point,
                    distance,
                );
            };
            if let Some(curve) = object.geometry().curve_ref() {
                centers::visit(curve, metric, &mut center);
            }
            cache.polygons.visit(
                object.id(),
                object.geometry(),
                document.tolerance(),
                metric,
                &mut center,
            );
        }
        if let Some(candidate) = object_best {
            consider_scored_candidate(
                &mut best,
                candidate.object_id,
                candidate.kind,
                candidate.point,
                candidate.distance,
            );
        }
    }
    Ok(best)
}

fn consider_candidate(
    best: &mut Option<ObjectSnap>,
    metric: &impl SnapMetric,
    capture_radius: Real,
    object_id: ObjectId,
    kind: ObjectSnapKind,
    point: Point3,
) {
    let Some(distance) = metric.distance(point) else {
        return;
    };
    if distance > capture_radius {
        return;
    }
    consider_scored_candidate(best, object_id, kind, point, distance);
}

fn consider_scored_candidate(
    best: &mut Option<ObjectSnap>,
    object_id: ObjectId,
    kind: ObjectSnapKind,
    point: Point3,
    distance: Real,
) {
    let candidate = ObjectSnap {
        point,
        kind,
        object_id,
        distance,
    };
    let replace = best.is_none_or(|current| {
        distance < current.distance
            || (distance == current.distance && kind.priority() < current.kind.priority())
    });
    if replace {
        *best = Some(candidate);
    }
}
