//! Visible-feature snap enumeration, projection metrics, and priority ordering.

use super::{DraftingError, validate_capture_radius};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{GeometryError, Point3, PointCloud3, Real};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSnapKind {
    Point,
    End,
    Mid,
    Center,
    Quad,
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

    /// Distance from the cursor in the projection used for the snap query.
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
    validate_capture_radius(capture_radius)?;
    if cursor_offset.iter().any(|value| !value.is_finite()) {
        return Err(GeometryError::NonFinite {
            context: "object snap cursor offset",
        }
        .into());
    }
    nearest_object_snap_with_metric(
        document,
        &XySnapMetric {
            origin,
            cursor_offset,
            capture_radius,
        },
    )
}

/// Finds the closest visible feature after mapping candidates into an
/// arbitrary two-dimensional viewport projection. The capture radius and the
/// returned distance use the same units as `cursor` and `project`.
pub fn nearest_object_snap_projected(
    document: &Document,
    cursor: [Real; 2],
    capture_radius: Real,
    project: impl Fn(Point3) -> Option<[Real; 2]>,
) -> Result<Option<ObjectSnap>, DraftingError> {
    validate_capture_radius(capture_radius)?;
    nearest_object_snap_with_metric(
        document,
        &ProjectedSnapMetric {
            cursor,
            capture_radius,
            project,
        },
    )
}

trait SnapMetric {
    fn capture_radius(&self) -> Real;
    fn distance(&self, point: Point3) -> Option<Real>;
    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError>;
}

struct XySnapMetric {
    origin: Point3,
    cursor_offset: [Real; 2],
    capture_radius: Real,
}

impl SnapMetric for XySnapMetric {
    fn capture_radius(&self) -> Real {
        self.capture_radius
    }

    fn distance(&self, point: Point3) -> Option<Real> {
        let distance = ((point.x() - self.origin.x()) - self.cursor_offset[0])
            .hypot((point.y() - self.origin.y()) - self.cursor_offset[1]);
        distance.is_finite().then_some(distance)
    }

    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError> {
        Ok(cloud
            .nearest_xy_relative(self.origin, self.cursor_offset, self.capture_radius)?
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

    fn distance(&self, point: Point3) -> Option<Real> {
        let projected = (self.project)(point)?;
        let distance = (projected[0] - self.cursor[0]).hypot(projected[1] - self.cursor[1]);
        distance.is_finite().then_some(distance)
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
) -> Result<Option<ObjectSnap>, DraftingError> {
    let cursor = metric;
    let capture_radius = metric.capture_radius();
    let mut best = None;

    for object in document.objects() {
        let attributes = object.attributes();
        let Some(layer) = document.layer(attributes.layer_id()) else {
            continue;
        };
        if !attributes.is_visible() || !layer.is_visible() {
            continue;
        }

        match object.geometry() {
            Geometry::Point(point) => consider_candidate(
                &mut best,
                cursor,
                capture_radius,
                object.id(),
                ObjectSnapKind::Point,
                *point,
            ),
            Geometry::PointCloud(cloud) => {
                if let Some(point) = metric.nearest_point_cloud(cloud)? {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::Point,
                        point,
                    );
                }
            }
            Geometry::Line(line) => {
                consider_candidate(
                    &mut best,
                    cursor,
                    capture_radius,
                    object.id(),
                    ObjectSnapKind::End,
                    line.start(),
                );
                consider_candidate(
                    &mut best,
                    cursor,
                    capture_radius,
                    object.id(),
                    ObjectSnapKind::End,
                    line.end(),
                );
                if let Ok(midpoint) = line.point_at(0.5) {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::Mid,
                        midpoint,
                    );
                }
            }
            Geometry::Circle(circle) => {
                consider_candidate(
                    &mut best,
                    cursor,
                    capture_radius,
                    object.id(),
                    ObjectSnapKind::Center,
                    circle.center(),
                );
                if let Ok(quadrants) = circle.quadrants() {
                    for quadrant in quadrants {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::Quad,
                            quadrant,
                        );
                    }
                }
            }
            Geometry::Arc(arc) => {
                for point in [arc.start(), arc.end()].into_iter().flatten() {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::End,
                        point,
                    );
                }
                if let Ok(midpoint) = arc.point_at(0.5) {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::Mid,
                        midpoint,
                    );
                }
                consider_candidate(
                    &mut best,
                    cursor,
                    capture_radius,
                    object.id(),
                    ObjectSnapKind::Center,
                    arc.center(),
                );
            }
            Geometry::Ellipse(ellipse) => {
                consider_candidate(
                    &mut best,
                    cursor,
                    capture_radius,
                    object.id(),
                    ObjectSnapKind::Center,
                    ellipse.center(),
                );
                if let Ok(quadrants) = ellipse.quadrants() {
                    for quadrant in quadrants {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::Quad,
                            quadrant,
                        );
                    }
                }
            }
            Geometry::Polyline(polyline) => {
                for vertex in polyline.vertices() {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::End,
                        *vertex,
                    );
                }
                for segment in polyline.segments() {
                    if let Ok(midpoint) = segment.point_at(0.5) {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::Mid,
                            midpoint,
                        );
                    }
                }
            }
            Geometry::NurbsCurve(curve) => {
                let domain = curve.domain();
                for parameter in [*domain.start(), *domain.end()] {
                    if let Ok(point) = curve.evaluate(parameter) {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::End,
                            point,
                        );
                    }
                }
            }
            Geometry::PolyCurve(curve) => {
                for segment in curve.segments() {
                    let domain = segment.domain();
                    for parameter in [*domain.start(), *domain.end()] {
                        if let Ok(point) = segment.evaluate(parameter) {
                            consider_candidate(
                                &mut best,
                                cursor,
                                capture_radius,
                                object.id(),
                                ObjectSnapKind::End,
                                point,
                            );
                        }
                    }
                }
            }
            Geometry::NurbsSurface(surface) => {
                let domain_u = surface.domain_u();
                let domain_v = surface.domain_v();
                for (u, v) in [
                    (*domain_u.start(), *domain_v.start()),
                    (*domain_u.end(), *domain_v.start()),
                    (*domain_u.end(), *domain_v.end()),
                    (*domain_u.start(), *domain_v.end()),
                ] {
                    if let Ok(point) = surface.evaluate(u, v) {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::End,
                            point,
                        );
                    }
                }
                if let Ok(point) =
                    surface.evaluate(surface.parameter_at_u(0.5)?, surface.parameter_at_v(0.5)?)
                {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::Mid,
                        point,
                    );
                }
            }
            Geometry::Brep(brep) => {
                for vertex in brep.vertices() {
                    consider_candidate(
                        &mut best,
                        cursor,
                        capture_radius,
                        object.id(),
                        ObjectSnapKind::End,
                        vertex.point(),
                    );
                }
                for edge in brep.edges() {
                    if let Ok(parameter) = edge.curve().parameter_at(0.5)
                        && let Ok(point) = edge.curve().evaluate(parameter)
                    {
                        consider_candidate(
                            &mut best,
                            cursor,
                            capture_radius,
                            object.id(),
                            ObjectSnapKind::Mid,
                            point,
                        );
                    }
                }
            }
            // Mesh vertex snapping needs a spatial index to remain responsive
            // on production STL meshes; do not introduce an O(vertices) query
            // into every pointer frame.
            Geometry::Mesh(_) => {}
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
