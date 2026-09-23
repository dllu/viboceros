use std::sync::{Arc, OnceLock};

use crate::{AffineTransform3, BoundingBox3, Frame3, GeometryError, Point3, Real};

mod index;
use index::{NodeBounds, ProjectedIndex, SearchRegion};

/// Axis-aligned projection used by a point-cloud spatial query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointCloudProjection {
    Xy,
    Xz,
    Yz,
}

impl PointCloudProjection {
    const fn axes(self) -> [u8; 3] {
        match self {
            Self::Xy => [0, 1, 2],
            Self::Xz => [0, 2, 1],
            Self::Yz => [1, 2, 0],
        }
    }
}

/// An immutable, finite collection of 3D points.
///
/// The stored order is significant, matching Rhino point clouds and 3DM
/// archives. A balanced XY k-d tree is built immediately; XZ/YZ indexes and
/// three-dimensional subtree bounds for arbitrary camera-plane queries are
/// initialized on demand. Clones share point storage and all caches.
/// Transforms construct a new data block instead of modifying shared geometry.
#[derive(Clone, Debug)]
pub struct PointCloud3 {
    data: Arc<PointCloudData>,
}

#[derive(Debug)]
struct PointCloudData {
    points: Vec<Point3>,
    bounds: BoundingBox3,
    xy: ProjectedIndex,
    xz: OnceLock<ProjectedIndex>,
    yz: OnceLock<ProjectedIndex>,
    spatial_bounds: OnceLock<Vec<NodeBounds>>,
}

impl PointCloud3 {
    /// Creates a point cloud while preserving point order and duplicates.
    /// Empty point clouds are rejected because OpenNURBS considers them
    /// invalid and they have no finite bounding box.
    pub fn try_new(points: Vec<Point3>) -> Result<Self, GeometryError> {
        let bounds = BoundingBox3::from_points(points.iter().copied())?;
        let xy = ProjectedIndex::new(&points, PointCloudProjection::Xy);
        Ok(Self {
            data: Arc::new(PointCloudData {
                points,
                bounds,
                xy,
                xz: OnceLock::new(),
                yz: OnceLock::new(),
                spatial_bounds: OnceLock::new(),
            }),
        })
    }

    #[inline]
    pub fn points(&self) -> &[Point3] {
        &self.data.points
    }

    #[inline]
    pub fn bounds(&self) -> BoundingBox3 {
        self.data.bounds
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        Self::try_new(
            self.data
                .points
                .iter()
                .map(|point| transform.transform_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// Returns the nearest point in the XY projection within `maximum_distance`.
    /// Exact-distance ties use the earliest stored point.
    pub fn nearest_xy(
        &self,
        query: Point3,
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_xy_relative(query, [0.0; 2], maximum_distance)
    }

    /// Searches XY distances evaluated as `(point - origin) - offset`.
    /// Keeping the local offset separate avoids rounding it away when the
    /// origin has large absolute coordinates. Ties retain source order.
    pub fn nearest_xy_relative(
        &self,
        origin: Point3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_projected_relative(PointCloudProjection::Xy, origin, offset, maximum_distance)
    }

    /// Searches `(project(point) - project(origin)) - offset` without forming
    /// an absolute cursor. XZ/YZ indexes are initialized once, on first valid
    /// use; XY is available immediately. Exact ties use stored point order.
    pub fn nearest_projected_relative(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_region(
            projection,
            origin,
            offset,
            SearchRegion::Circle(maximum_distance),
        )
    }

    /// Nearest Euclidean target inside an inclusive square of `half_width`,
    /// in projected model units. Admission and ordering are separate: a closer
    /// point outside the square must not hide a farther point in its corner.
    /// Reuses the axis-aligned indexes and local-origin precision. An admitted
    /// but unrepresentable nearest distance is an error, not a missed point.
    pub fn nearest_projected_in_box_relative(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        half_width: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_region(projection, origin, offset, SearchRegion::Square(half_width))
    }

    /// Nearest member in an arbitrary orthonormal frame's XY projection.
    /// The frame origin and local `offset` remain separate for distant models.
    /// A lazy 3D bound cache lets the shared point tree prune projected queries.
    pub fn nearest_projected_frame_relative(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_frame_region(frame, offset, SearchRegion::Circle(maximum_distance))
    }

    /// As above, with an inclusive square capture aperture and Euclidean ranking.
    pub fn nearest_projected_in_frame_box_relative(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        half_width: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_frame_region(frame, offset, SearchRegion::Square(half_width))
    }

    fn nearest_in_frame_region(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        region: SearchRegion,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        let radius = region.half_width();
        if !radius.is_finite() || radius < 0.0 {
            return Err(GeometryError::InvalidPointCloudSearchRadius);
        }
        if offset.iter().any(|value| !value.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "point cloud search offset",
            });
        }
        let bounds = self
            .data
            .spatial_bounds
            .get_or_init(|| self.data.xy.node_bounds(&self.data.points));
        let best = self
            .data
            .xy
            .nearest_in_frame(&self.data.points, bounds, frame, offset, region);
        Ok(best.map(|(distance, index)| (index, self.data.points[index], distance)))
    }

    fn nearest_in_region(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        region: SearchRegion,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        let maximum_distance = region.half_width();
        if !maximum_distance.is_finite() || maximum_distance < 0.0 {
            return Err(GeometryError::InvalidPointCloudSearchRadius);
        }
        if offset.iter().any(|value| !value.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "point cloud search offset",
            });
        }
        let index = match projection {
            PointCloudProjection::Xy => &self.data.xy,
            PointCloudProjection::Xz => self
                .data
                .xz
                .get_or_init(|| ProjectedIndex::new(&self.data.points, projection)),
            PointCloudProjection::Yz => self
                .data
                .yz
                .get_or_init(|| ProjectedIndex::new(&self.data.points, projection)),
        };
        let mut best = None;
        index.nearest_from(
            index.root,
            &self.data.points,
            origin,
            offset,
            region,
            &mut best,
        );
        if best.is_some_and(|(distance, _)| !distance.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "point cloud projected distance",
            });
        }
        Ok(best
            .map(|(distance, point_index)| (point_index, self.data.points[point_index], distance)))
    }
}

impl PartialEq for PointCloud3 {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) || self.data.points == other.data.points
    }
}

#[cfg(test)]
mod tests;
