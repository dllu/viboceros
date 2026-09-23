//! Visible-feature snap enumeration, projection metrics, and priority ordering.
mod cache;
#[cfg(test)]
mod capture_tests;
mod centers;
mod features;
mod mesh;
mod mid_hover;
mod near;
mod polygon_centers;
mod projected_line;
mod proximity;
pub use cache::ObjectSnapCache;

use super::{DraftingError, validate_capture_radius, validate_cursor_coordinates};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    Frame3, GeometryError, Point3, PointCloud3, PointCloudProjection, Real, Vector3,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSnapKind {
    Point,
    End,
    Mid,
    Center,
    Quad,
    Near,
}

/// Enabled feature kinds, independent of the UI's persistent/one-shot lifetime.
/// Defaults to no enabled features.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectSnapModes(u8);

/// Feature selection and source policy, independent of viewport/prompt lifetime.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ObjectSnapOptions {
    pub modes: ObjectSnapModes,
    /// Enable supported wire snaps on meshes. This does not enable a feature
    /// mode or turn suspended snaps back on.
    pub mesh_edges: bool,
}

impl From<ObjectSnapModes> for ObjectSnapOptions {
    fn from(modes: ObjectSnapModes) -> Self {
        Self {
            modes,
            mesh_edges: false,
        }
    }
}

impl ObjectSnapModes {
    pub const NONE: Self = Self(0);
    /// Discrete landmarks enabled by the standard query/UI defaults. Near is opt-in.
    pub const LANDMARKS: Self = Self(0b1_1111);
    pub const ALL: Self = Self(0b11_1111);

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
            Self::Near => "Near",
        }
    }

    const fn priority(self) -> u8 {
        match self {
            Self::Point => 0,
            Self::End => 1,
            Self::Mid => 2,
            Self::Center => 3,
            Self::Quad => 4,
            Self::Near => 5,
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

    /// Euclidean distance in the query's projection: to the point feature, or
    /// to the source curve for hover-derived Center or Mid-only snaps.
    pub const fn distance(self) -> Real {
        self.distance
    }
}

/// Finds the closest visible landmark snap in the top-view XY projection.
/// These convenience queries use [`ObjectSnapModes::LANDMARKS`]; use a cache's
/// explicit-mode query to enable Near.
/// Locked objects remain snap targets, matching Rhino. Exact-distance ties use
/// the stable priority encoded by [`ObjectSnapKind`].
/// `capture_radius` is the half-width of the inclusive square snap aperture.
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
/// affine or projective viewport projection. The square aperture half-width and the
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
    fn is_affine(&self) -> bool {
        false
    }
    fn capture_radius(&self) -> Real;
    fn offset(&self, point: Point3) -> Option<[Real; 2]>;
    #[cfg(test)]
    fn distance(&self, point: Point3) -> Option<Real> {
        let [x, y] = self.offset(point)?;
        let d = x.hypot(y);
        d.is_finite().then_some(d)
    }
    /// The pick aperture is square; Euclidean distance still ranks targets.
    fn captured_distance(&self, point: Point3) -> Option<Real> {
        self.captured_offset_distance(self.offset(point)?)
    }
    fn captured_offset_distance(&self, [x, y]: [Real; 2]) -> Option<Real> {
        if x.abs().max(y.abs()) > self.capture_radius() {
            return None;
        }
        let distance = x.hypot(y);
        distance.is_finite().then_some(distance)
    }
    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError>;

    /// Oriented screen tangent; only its direction, not its speed, is needed.
    /// A projective map sends the model tangent line to the exact screen tangent
    /// line. This is a finite line projection, not a finite difference of a curve.
    fn tangent_direction(&self, point: Point3, tangent: Vector3) -> Option<[Real; 2]> {
        let origin = self.offset(point)?;
        let unit = tangent.normalized_nonzero().ok()?.as_vector().to_array();
        let p = point.to_array();
        let scale = p.into_iter().map(Real::abs).fold(1., Real::max);
        for step in [scale, -scale, scale * 0.25, -scale * 0.25] {
            let other = Point3::try_from(std::array::from_fn(|i| step.mul_add(unit[i], p[i]))).ok();
            let Some(other) = other.and_then(|p| self.offset(p)) else {
                continue;
            };
            let delta = std::array::from_fn(|i| (other[i] - origin[i]) * step.signum());
            if let Some(direction) = near::unit_screen(delta) {
                return Some(direction);
            }
        }
        None
    }
}

struct AxisAlignedSnapMetric {
    projection: PointCloudProjection,
    origin: Point3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
}

impl SnapMetric for AxisAlignedSnapMetric {
    fn is_affine(&self) -> bool {
        true
    }
    fn tangent_direction(&self, _point: Point3, tangent: Vector3) -> Option<[Real; 2]> {
        near::unit_screen(match self.projection {
            PointCloudProjection::Xy => [tangent.x(), tangent.y()],
            PointCloudProjection::Xz => [tangent.x(), tangent.z()],
            PointCloudProjection::Yz => [tangent.y(), tangent.z()],
        })
    }
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
            .nearest_visible_projected_in_box_relative(
                self.projection,
                self.origin,
                self.cursor_offset,
                self.capture_radius,
            )?
            .map(|(_, point, _)| point))
    }
}

struct FrameSnapMetric {
    frame: Frame3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
}

impl SnapMetric for FrameSnapMetric {
    fn is_affine(&self) -> bool {
        true
    }

    fn capture_radius(&self) -> Real {
        self.capture_radius
    }

    fn offset(&self, point: Point3) -> Option<[Real; 2]> {
        let projected = self.frame.projected_coordinates_of(point).ok()?;
        let delta = [
            projected[0] - self.cursor_offset[0],
            projected[1] - self.cursor_offset[1],
        ];
        delta.iter().all(|value| value.is_finite()).then_some(delta)
    }

    fn tangent_direction(&self, _point: Point3, tangent: Vector3) -> Option<[Real; 2]> {
        let unit = tangent.normalized_nonzero().ok()?.as_vector();
        near::unit_screen([
            unit.dot(self.frame.x_axis().as_vector()).ok()?,
            unit.dot(self.frame.y_axis().as_vector()).ok()?,
        ])
    }

    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError> {
        Ok(cloud
            .nearest_visible_projected_in_frame_box_relative(
                self.frame,
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
            .enumerate()
            .filter(|(index, _)| !cloud.is_hidden(*index))
            .filter_map(|(_, point)| {
                self.captured_distance(*point)
                    .map(|distance| (distance, *point))
            })
            .min_by(|(first, _), (second, _)| first.total_cmp(second))
            .map(|(_, point)| point))
    }
}

fn nearest_object_snap_with_metric(
    document: &Document,
    metric: &impl SnapMetric,
    cache: &mut ObjectSnapCache,
    options: ObjectSnapOptions,
) -> Result<Option<ObjectSnap>, DraftingError> {
    let modes = options.modes;
    // Suspension is O(1), including with large surface/B-rep documents. Cache
    // cleanup resumes on the next enabled query; public input validation still runs.
    if modes == ObjectSnapModes::NONE {
        return Ok(None);
    }
    cache.retain_objects(document);
    // Visibility lookups must not scan every layer for every object. Locked
    // geometry remains eligible for snapping, independently of selection rules.
    let visible_layers: std::collections::HashSet<_> = document
        .layers()
        .filter(|layer| layer.is_visible())
        .map(|layer| layer.id())
        .collect();
    let mut best = None;
    for object in document.objects() {
        let attributes = object.attributes();
        if !attributes.is_visible() || !visible_layers.contains(&attributes.layer_id()) {
            continue;
        }
        if matches!(object.geometry(), Geometry::Mesh(_)) {
            if options.mesh_edges {
                cache
                    .meshes
                    .visit(object, modes, metric, &mut |kind, point, distance| {
                        consider_scored_candidate(&mut best, object.id(), kind, point, distance);
                    });
            }
            continue;
        }
        let mut object_best = None;
        if modes == ObjectSnapModes::only(ObjectSnapKind::Mid) {
            mid_hover::visit(
                object,
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
            consider_candidate(&mut object_best, metric, object.id(), kind, point);
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
                features::curve(curve, modes, &mut emit);
            }
            Geometry::NurbsSurface(surface) if modes.contains(ObjectSnapKind::End) => {
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
            }
            Geometry::Brep(brep) if modes.contains(ObjectSnapKind::End) => {
                for vertex in brep.vertices() {
                    emit(ObjectSnapKind::End, vertex.point());
                }
            }
            // Mesh queries use the independently cached wire index above.
            Geometry::Mesh(_) | Geometry::NurbsSurface(_) | Geometry::Brep(_) => {}
        }
        // Mid and Center share source discovery. Each expensive feature is
        // independently lazy; surface Mid belongs to boundaries, not UV center.
        if modes.contains(ObjectSnapKind::Mid) {
            for feature in cache.geometry_curves(object, document.tolerance()) {
                if let Some(point) = feature.midpoint() {
                    emit(ObjectSnapKind::Mid, point);
                }
            }
        }
        // Direct landmarks suppress Near on the same source, even if Near is
        // visually closer. Near in turn suppresses curve-hover Center.
        if object_best.is_none() && modes.contains(ObjectSnapKind::Near) {
            near::visit(
                object,
                document.tolerance(),
                cache,
                metric,
                &mut |point, distance| {
                    consider_scored_candidate(
                        &mut object_best,
                        object.id(),
                        ObjectSnapKind::Near,
                        point,
                        distance,
                    )
                },
            );
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
            centers::visit_nurbs(object, document.tolerance(), cache, metric, &mut center);
            cache
                .polygons
                .visit(object, document.tolerance(), metric, &mut center);
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
    object_id: ObjectId,
    kind: ObjectSnapKind,
    point: Point3,
) {
    let Some(distance) = metric.captured_distance(point) else {
        return;
    };
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
