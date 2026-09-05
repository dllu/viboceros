//! Precision-drafting queries shared by interactive viewports.

mod point_input;
pub use point_input::{PointInput, PointInputError};

use thiserror::Error;
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
    #[error("drafting capture radius must be finite and strictly positive")]
    InvalidCaptureRadius,

    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Finds the closest visible feature snap in the top-view XY projection.
/// Locked objects remain snap targets, matching Rhino. Exact-distance ties use
/// the stable priority encoded by [`ObjectSnapKind`].
pub fn nearest_object_snap(
    document: &Document,
    cursor: Point3,
    capture_radius: Real,
) -> Result<Option<ObjectSnap>, DraftingError> {
    validate_capture_radius(capture_radius)?;
    nearest_object_snap_with_metric(
        document,
        &XySnapMetric {
            cursor,
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
    cursor: Point3,
    capture_radius: Real,
}

impl SnapMetric for XySnapMetric {
    fn capture_radius(&self) -> Real {
        self.capture_radius
    }

    fn distance(&self, point: Point3) -> Option<Real> {
        let distance = (point.x() - self.cursor.x()).hypot(point.y() - self.cursor.y());
        distance.is_finite().then_some(distance)
    }

    fn nearest_point_cloud(&self, cloud: &PointCloud3) -> Result<Option<Point3>, GeometryError> {
        Ok(cloud
            .nearest_xy(self.cursor, self.capture_radius)?
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
    if !horizontal_distance.is_finite() || !vertical_distance.is_finite() {
        return Ok(None);
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::Geometry;
    use viboceros_geometry::{
        Brep, Circle3, CircularArc3, Ellipse3, Frame3, LineSegment, NurbsCurve, NurbsSurface,
        PointCloud3, Polyline3, Tolerance, UnitVector3, Vector3,
    };

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn snaps_to_closest_line_features_and_respects_radius() {
        let mut document = Document::default();
        let line_id = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(0.0, 0.0, 3.0),
                    point(10.0, 0.0, 3.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();

        let midpoint = nearest_object_snap(&document, point(5.2, 0.1, 0.0), 0.5)
            .unwrap()
            .unwrap();
        assert_eq!(midpoint.object_id(), line_id);
        assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
        assert_eq!(midpoint.point(), point(5.0, 0.0, 3.0));
        assert!(
            nearest_object_snap(&document, point(5.2, 0.1, 0.0), 0.1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn exact_ties_use_feature_priority_not_document_order() {
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 5.0)))
            .unwrap();

        let snap = nearest_object_snap(&document, point(0.0, 0.0, 0.0), 1.0)
            .unwrap()
            .unwrap();
        assert_eq!(snap.kind(), ObjectSnapKind::Point);
        assert_eq!(snap.object_id(), point_id);
        assert_eq!(snap.point().z(), 5.0);
    }

    #[test]
    fn snaps_to_the_nearest_point_cloud_member_in_xy() {
        let mut document = Document::default();
        let cloud_id = document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![
                    point(-4.0, 2.0, 9.0),
                    point(3.0, 5.0, -7.0),
                    point(8.0, 1.0, 2.0),
                ])
                .unwrap(),
            ))
            .unwrap();
        let snap = nearest_object_snap(&document, point(3.08, 5.02, 100.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(snap.object_id(), cloud_id);
        assert_eq!(snap.kind(), ObjectSnapKind::Point);
        assert_eq!(snap.point(), point(3.0, 5.0, -7.0));
    }

    #[test]
    fn projected_snapping_uses_the_requested_view_plane() {
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let cloud_id = document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![point(0.0, 4.0, 10.0), point(6.0, 0.0, 10.0)]).unwrap(),
            ))
            .unwrap();

        let snap = nearest_object_snap_projected(&document, [0.05, 10.02], 0.1, |candidate| {
            Some([candidate.x(), candidate.z()])
        })
        .unwrap()
        .unwrap();
        assert_eq!(snap.object_id(), cloud_id);
        assert_eq!(snap.point(), point(0.0, 4.0, 10.0));
        assert!(snap.distance() < 0.1);
    }

    #[test]
    fn polycurve_segment_junctions_snap_to_the_composite_object() {
        let segments = vec![
            NurbsCurve::try_clamped_uniform(
                2,
                vec![
                    point(0.0, 0.0, 0.0),
                    point(1.0, 2.0, 0.0),
                    point(3.0, 0.0, 0.0),
                ],
            )
            .unwrap(),
            NurbsCurve::try_clamped_uniform(1, vec![point(3.0, 0.0, 0.0), point(4.0, 3.0, 0.0)])
                .unwrap(),
        ];
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::PolyCurve(
                viboceros_geometry::PolyCurve3::try_new(segments).unwrap(),
            ))
            .unwrap();
        for target in [
            point(0.0, 0.0, 0.0),
            point(3.0, 0.0, 0.0),
            point(4.0, 3.0, 0.0),
        ] {
            let snap = nearest_object_snap(&document, target, 0.1)
                .unwrap()
                .unwrap();
            assert_eq!(snap.object_id(), id);
            assert_eq!(snap.kind(), ObjectSnapKind::End);
            assert_eq!(snap.point(), target);
        }
        document.set_objects_visibility([id], false).unwrap();
        assert!(
            nearest_object_snap(&document, point(3.0, 0.0, 0.0), 0.1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn curve_endpoints_are_available_to_osnap() {
        let mut document = Document::default();
        document
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(
                    2,
                    vec![
                        point(1.0, 2.0, 0.0),
                        point(3.0, 5.0, 0.0),
                        point(7.0, 2.0, 0.0),
                    ],
                )
                .unwrap(),
            ))
            .unwrap();

        let snap = nearest_object_snap(&document, point(7.05, 2.0, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(snap.kind(), ObjectSnapKind::End);
        assert_eq!(snap.point(), point(7.0, 2.0, 0.0));
    }

    #[test]
    fn surface_corners_and_center_are_available_to_osnap() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(1.0, 2.0, 0.0),
                    point(7.0, 2.0, 0.0),
                    point(7.0, 6.0, 2.0),
                    point(1.0, 6.0, 2.0),
                ])
                .unwrap(),
            ))
            .unwrap();

        let corner = nearest_object_snap(&document, point(7.02, 6.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(corner.object_id(), id);
        assert_eq!(corner.kind(), ObjectSnapKind::End);
        assert_eq!(corner.point(), point(7.0, 6.0, 2.0));

        let center = nearest_object_snap(&document, point(4.01, 4.02, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(center.kind(), ObjectSnapKind::Mid);
        assert_eq!(center.point(), point(4.0, 4.0, 1.0));
    }

    #[test]
    fn brep_vertices_and_edge_midpoints_are_available_to_osnap() {
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let id = document
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame,
                    [[0.0, 4.0], [0.0, 6.0], [0.0, 2.0]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();

        let vertex = nearest_object_snap(&document, point(4.02, 6.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(vertex.object_id(), id);
        assert_eq!(vertex.kind(), ObjectSnapKind::End);
        assert_eq!(vertex.point(), point(4.0, 6.0, 0.0));

        let midpoint = nearest_object_snap(&document, point(2.01, 0.02, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(midpoint.object_id(), id);
        assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
        assert_eq!(midpoint.point(), point(2.0, 0.0, 0.0));
    }

    #[test]
    fn circle_and_arc_features_are_available_to_osnap() {
        let mut document = Document::default();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let circle_id = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(point(0.0, 0.0, 3.0), 2.0, normal, Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let arc_id = document
            .add_geometry(Geometry::Arc(
                CircularArc3::try_from_three_points(
                    point(9.0, 0.0, 4.0),
                    point(10.0, 1.0, 4.0),
                    point(11.0, 0.0, 4.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();

        let center = nearest_object_snap(&document, point(0.02, -0.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(center.object_id(), circle_id);
        assert_eq!(center.kind(), ObjectSnapKind::Center);
        assert_eq!(center.point(), point(0.0, 0.0, 3.0));

        let quadrant = nearest_object_snap(&document, point(2.02, 0.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(quadrant.kind(), ObjectSnapKind::Quad);
        assert_eq!(quadrant.point(), point(2.0, 0.0, 3.0));

        let midpoint = nearest_object_snap(&document, point(10.0, 1.02, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(midpoint.object_id(), arc_id);
        assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
        assert!(
            midpoint
                .point()
                .is_near(point(10.0, 1.0, 4.0), Tolerance::DEFAULT)
        );
    }

    #[test]
    fn ellipse_center_and_quadrants_are_available_to_osnap() {
        let mut document = Document::default();
        let ellipse = Ellipse3::try_from_three_points(
            point(2.0, 3.0, 5.0),
            point(8.0, 3.0, 5.0),
            point(4.0, -1.0, 5.0),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Ellipse(ellipse)).unwrap();

        let center = nearest_object_snap(&document, point(2.02, 3.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(center.object_id(), id);
        assert_eq!(center.kind(), ObjectSnapKind::Center);
        assert_eq!(center.point(), point(2.0, 3.0, 5.0));

        let quadrant = nearest_object_snap(&document, point(8.01, 3.02, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(quadrant.object_id(), id);
        assert_eq!(quadrant.kind(), ObjectSnapKind::Quad);
        assert_eq!(quadrant.point(), point(8.0, 3.0, 5.0));
    }

    #[test]
    fn polyline_vertices_and_segment_midpoints_are_available_to_osnap() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![
                        point(0.0, 0.0, 2.0),
                        point(4.0, 0.0, 2.0),
                        point(4.0, 6.0, 2.0),
                    ],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let vertex = nearest_object_snap(&document, point(4.02, 0.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(vertex.object_id(), id);
        assert_eq!(vertex.kind(), ObjectSnapKind::End);
        assert_eq!(vertex.point(), point(4.0, 0.0, 2.0));

        let midpoint = nearest_object_snap(&document, point(4.02, 3.01, 0.0), 0.1)
            .unwrap()
            .unwrap();
        assert_eq!(midpoint.kind(), ObjectSnapKind::Mid);
        assert_eq!(midpoint.point(), point(4.0, 3.0, 2.0));
    }

    #[test]
    fn locked_objects_remain_snap_targets_but_hidden_objects_do_not() {
        let mut document = Document::default();
        let default = document.current_layer_id();
        let reference = document
            .add_layer("Reference", viboceros_document::ColorRgb::new(1, 2, 3))
            .unwrap();
        document.set_current_layer(reference).unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(point(5.0, 6.0, 0.0)))
            .unwrap();
        document.set_current_layer(default).unwrap();

        document.set_layer_locked(reference, true).unwrap();
        assert_eq!(
            nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
                .unwrap()
                .unwrap()
                .object_id(),
            point_id
        );
        document.set_layer_locked(reference, false).unwrap();
        document.set_objects_locked([point_id], true).unwrap();
        assert_eq!(
            nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
                .unwrap()
                .unwrap()
                .object_id(),
            point_id
        );
        document.set_objects_visibility([point_id], false).unwrap();
        assert!(
            nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
                .unwrap()
                .is_none()
        );
        document.set_objects_visibility([point_id], true).unwrap();
        document.set_layer_visibility(reference, false).unwrap();
        assert!(
            nearest_object_snap(&document, point(5.0, 6.0, 0.0), 0.1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn orthogonal_tracking_preserves_the_anchor_plane() {
        let anchor = point(2.0, 3.0, 7.0);
        let horizontal = orthogonal_track(point(8.0, 3.1, 0.0), anchor, 0.2)
            .unwrap()
            .unwrap();
        assert_eq!(horizontal.axis(), TrackAxis::Horizontal);
        assert_eq!(horizontal.point(), point(8.0, 3.0, 7.0));

        let vertical = orthogonal_track(point(2.1, -4.0, 0.0), anchor, 0.2)
            .unwrap()
            .unwrap();
        assert_eq!(vertical.axis(), TrackAxis::Vertical);
        assert_eq!(vertical.point(), point(2.0, -4.0, 7.0));

        let both = orthogonal_track(point(2.1, 2.9, 0.0), anchor, 0.2)
            .unwrap()
            .unwrap();
        assert_eq!(both.axis(), TrackAxis::Both);
        assert_eq!(both.point(), anchor);
    }

    #[test]
    fn rejects_invalid_capture_radii() {
        let document = Document::default();
        for radius in [0.0, -1.0, Real::NAN, Real::INFINITY] {
            assert_eq!(
                nearest_object_snap(&document, point(0.0, 0.0, 0.0), radius),
                Err(DraftingError::InvalidCaptureRadius)
            );
            assert_eq!(
                orthogonal_track(point(0.0, 0.0, 0.0), point(1.0, 1.0, 0.0), radius),
                Err(DraftingError::InvalidCaptureRadius)
            );
        }
    }
}
