use std::sync::{Arc, OnceLock};

use crate::{AffineTransform3, BoundingBox3, GeometryError, Point3, Real};

mod index;
use index::ProjectedIndex;

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
/// archives. A balanced XY k-d tree is built immediately; XZ/YZ indexes are
/// initialized on demand and reused for axis-aligned viewport queries.
/// Clones share immutable point storage and all projection caches; transforms
/// construct a new data block rather than modifying shared geometry.
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
            maximum_distance,
            &mut best,
        );
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
