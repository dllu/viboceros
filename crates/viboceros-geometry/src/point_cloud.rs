use std::sync::{Arc, OnceLock};

use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, Point3, Real, Tolerance, Vector3,
};

mod index;
use index::{NodeBounds, ProjectedIndex, ProjectedQuery, SearchRegion};

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

/// The stored OpenNURBS height-field plane. Axes retain their original values
/// so a 3DM round trip does not orthogonalize or rescale a valid source frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointCloudPlane {
    origin: Point3,
    axes: [Vector3; 3],
}

impl PointCloudPlane {
    pub fn try_new(origin: Point3, axes: [Vector3; 3]) -> Result<Self, GeometryError> {
        // Match OpenNURBS's orthonormal right-handed frame checks without
        // changing any source components.
        let epsilon = f64::EPSILON.sqrt();
        let lengths = [axes[0].length()?, axes[1].length()?, axes[2].length()?];
        if lengths.iter().any(|length| (length - 1.0).abs() > epsilon) {
            return Err(GeometryError::Degenerate {
                context: "point cloud plane",
            });
        }
        let normalized_dot = |first: usize, second: usize| -> Result<Real, GeometryError> {
            Ok(axes[first].dot(axes[second])? / (lengths[first] * lengths[second]))
        };
        let orthogonality = [
            normalized_dot(0, 1)?,
            normalized_dot(1, 2)?,
            normalized_dot(2, 0)?,
        ];
        if orthogonality.iter().any(|dot| dot.abs() > epsilon) {
            if orthogonality.iter().any(|dot| dot.abs() >= 1.0 / 65536.0) {
                return Err(GeometryError::Degenerate {
                    context: "point cloud plane",
                });
            }
            for [first, second, third] in [[0, 1, 2], [1, 2, 0], [2, 0, 1]] {
                let cross = axes[first].cross(axes[second])?;
                let alignment =
                    cross.dot(axes[third])? / (lengths[first] * lengths[second] * lengths[third]);
                if (alignment.abs() - 1.0).abs() > epsilon {
                    return Err(GeometryError::Degenerate {
                        context: "point cloud plane",
                    });
                }
            }
        }
        if axes[0].cross(axes[1])?.dot(axes[2])? <= epsilon {
            return Err(GeometryError::Degenerate {
                context: "point cloud plane",
            });
        }
        Ok(Self { origin, axes })
    }

    pub const fn origin(self) -> Point3 {
        self.origin
    }

    pub const fn axes(self) -> [Vector3; 3] {
        self.axes
    }

    pub fn transformed(self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        if transform == AffineTransform3::identity() {
            return Ok(self);
        }
        let origin = transform.transform_point(self.origin)?;
        let x = transform.transform_vector(self.axes[0])?;
        let y = transform.transform_vector(self.axes[1])?;
        let frame = Frame3::try_from_directions(origin, x, y, Tolerance::NUMERICAL_VALIDATION)?;
        Self::try_new(
            origin,
            [
                frame.x_axis().as_vector(),
                frame.y_axis().as_vector(),
                frame.z_axis().as_vector(),
            ],
        )
    }
}

/// Optional per-point channels and the OpenNURBS ordered-stream flag.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointCloudChannels {
    /// OpenNURBS alpha is transparency: zero is opaque.
    pub colors: Option<Vec<[u8; 4]>>,
    pub normals: Option<Vec<Vector3>>,
    pub values: Option<Vec<Real>>,
    pub ordered: bool,
    pub plane: Option<PointCloudPlane>,
    /// Runtime visibility flags; OpenNURBS does not serialize these to 3DM.
    pub hidden: Option<Vec<bool>>,
}

#[derive(Debug)]
struct PointCloudData {
    points: Vec<Point3>,
    channels: PointCloudChannels,
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
        Self::try_with_colors(points, None)
    }

    /// Creates a cloud with optional per-point OpenNURBS colors. Alpha is
    /// transparency: zero is opaque and 255 is fully transparent.
    pub fn try_with_colors(
        points: Vec<Point3>,
        colors: Option<Vec<[u8; 4]>>,
    ) -> Result<Self, GeometryError> {
        Self::try_with_channels(
            points,
            PointCloudChannels {
                colors,
                ..PointCloudChannels::default()
            },
        )
    }

    /// Creates a point cloud with validated, index-aligned optional channels.
    pub fn try_with_channels(
        points: Vec<Point3>,
        mut channels: PointCloudChannels,
    ) -> Result<Self, GeometryError> {
        if channels
            .colors
            .as_ref()
            .is_some_and(|colors| colors.len() != points.len())
        {
            return Err(GeometryError::InvalidPointCloudColorCount);
        }
        if channels
            .normals
            .as_ref()
            .is_some_and(|normals| normals.len() != points.len())
        {
            return Err(GeometryError::InvalidPointCloudNormalCount);
        }
        if channels
            .values
            .as_ref()
            .is_some_and(|values| values.len() != points.len())
        {
            return Err(GeometryError::InvalidPointCloudValueCount);
        }
        if channels
            .hidden
            .as_ref()
            .is_some_and(|hidden| hidden.len() != points.len())
        {
            return Err(GeometryError::InvalidPointCloudHiddenCount);
        }
        if channels
            .hidden
            .as_ref()
            .is_some_and(|hidden| !hidden.contains(&true))
        {
            channels.hidden = None;
        }
        if channels
            .values
            .as_ref()
            .is_some_and(|values| values.iter().any(|value| !value.is_finite()))
        {
            return Err(GeometryError::NonFinite {
                context: "point cloud values",
            });
        }
        let bounds = BoundingBox3::from_points(points.iter().copied())?;
        let xy = ProjectedIndex::new(&points, PointCloudProjection::Xy);
        Ok(Self {
            data: Arc::new(PointCloudData {
                points,
                channels,
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
    pub fn colors(&self) -> Option<&[[u8; 4]]> {
        self.data.channels.colors.as_deref()
    }

    pub fn normals(&self) -> Option<&[Vector3]> {
        self.data.channels.normals.as_deref()
    }

    pub fn values(&self) -> Option<&[Real]> {
        self.data.channels.values.as_deref()
    }

    pub fn is_ordered(&self) -> bool {
        self.data.channels.ordered
    }

    pub fn plane(&self) -> Option<PointCloudPlane> {
        self.data.channels.plane
    }

    pub fn channels(&self) -> &PointCloudChannels {
        &self.data.channels
    }

    pub fn hidden(&self) -> Option<&[bool]> {
        self.data.channels.hidden.as_deref()
    }

    pub fn is_hidden(&self, index: usize) -> bool {
        self.hidden()
            .is_some_and(|hidden| hidden.get(index) == Some(&true))
    }

    pub fn hidden_count(&self) -> usize {
        self.hidden()
            .map_or(0, |hidden| hidden.iter().filter(|&&flag| flag).count())
    }

    /// Returns a new cloud with the same stored members and updated runtime visibility.
    pub fn with_hidden(&self, hidden: Vec<bool>) -> Result<Self, GeometryError> {
        let mut channels = self.data.channels.clone();
        channels.hidden = Some(hidden);
        Self::try_with_channels(self.points().to_vec(), channels)
    }

    #[inline]
    pub fn bounds(&self) -> BoundingBox3 {
        self.data.bounds
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        let mut channels = self.data.channels.clone();
        channels.plane = channels
            .plane
            .map(|plane| plane.transformed(transform))
            .transpose()?;
        Self::try_with_channels(
            self.data
                .points
                .iter()
                .map(|point| transform.transform_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
            channels,
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
            None,
        )
    }

    /// Nearest visible member in a projected circle. Stored indices are unchanged.
    pub fn nearest_visible_projected_relative(
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
            self.hidden(),
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
        self.nearest_in_region(
            projection,
            origin,
            offset,
            SearchRegion::Square(half_width),
            None,
        )
    }

    /// Nearest visible member inside an inclusive projected square.
    pub fn nearest_visible_projected_in_box_relative(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        half_width: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_region(
            projection,
            origin,
            offset,
            SearchRegion::Square(half_width),
            self.hidden(),
        )
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
        self.nearest_in_frame_region(frame, offset, SearchRegion::Circle(maximum_distance), None)
    }

    /// Nearest visible member in a frame projection.
    pub fn nearest_visible_projected_frame_relative(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_frame_region(
            frame,
            offset,
            SearchRegion::Circle(maximum_distance),
            self.hidden(),
        )
    }

    /// As above, with an inclusive square capture aperture and Euclidean ranking.
    pub fn nearest_projected_in_frame_box_relative(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        half_width: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_frame_region(frame, offset, SearchRegion::Square(half_width), None)
    }

    /// Nearest visible member inside an inclusive frame-projected square.
    pub fn nearest_visible_projected_in_frame_box_relative(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        half_width: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_in_frame_region(
            frame,
            offset,
            SearchRegion::Square(half_width),
            self.hidden(),
        )
    }

    fn nearest_in_frame_region(
        &self,
        frame: Frame3,
        offset: [Real; 2],
        region: SearchRegion,
        hidden: Option<&[bool]>,
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
        let best =
            self.data
                .xy
                .nearest_in_frame(&self.data.points, hidden, bounds, frame, offset, region);
        Ok(best.map(|(distance, index)| (index, self.data.points[index], distance)))
    }

    fn nearest_in_region(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        region: SearchRegion,
        hidden: Option<&[bool]>,
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
            ProjectedQuery {
                points: &self.data.points,
                hidden,
                origin,
                offset,
                region,
            },
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
        Arc::ptr_eq(&self.data, &other.data)
            || (self.data.points == other.data.points && self.data.channels == other.data.channels)
    }
}

#[cfg(test)]
mod tests;
