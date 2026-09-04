use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::RangeInclusive;

use spade::{
    ConstrainedDelaunayTriangulation, HasPosition, Point2 as TriangulationPoint2, Triangulation,
};

use crate::nurbs::{CURVE_COINCIDENCE_ABSOLUTE, find_span_in_knots};
use crate::{
    AffineTransform3, BoundingBox3, CurveCurveIntersectionEvent, CurveCurveOverlap, Frame3,
    GeometryError, LineSegment, MAX_REGULAR_POLYGON_SIDES, MeshFace, NurbsCurve, NurbsCurve2,
    NurbsSurface, Point2, Point3, Real, Tolerance, TriangleMesh, UnitVector3, Vector3,
    WeightedPoint2, WeightedPoint3, integration::integrate_adaptive,
    nurbs_surface::integrate_area_patch, require_finite, vector::product_three,
};

const LOOP_SAMPLES_PER_SPAN: usize = 4;
const MAX_EAR_CLIP_VERTICES: usize = 16_384;
const MAX_CONSTRAINED_TRIM_VERTICES: usize = 131_072;
const MAX_TRIM_ROOT_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug)]
struct BoundarySnapPoint {
    point: Point3,
    tolerance: Real,
}

#[derive(Clone, Copy, Debug)]
struct TrimTriangulationVertex {
    position: TriangulationPoint2<Real>,
    source_index: usize,
}

impl HasPosition for TrimTriangulationVertex {
    type Scalar = Real;

    fn position(&self) -> TriangulationPoint2<Self::Scalar> {
        self.position
    }
}

/// A B-rep vertex with its model-space coincidence tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrepVertex {
    point: Point3,
    tolerance: Real,
}

impl BrepVertex {
    pub fn try_new(point: Point3, tolerance: Real) -> Result<Self, GeometryError> {
        require_nonnegative_finite(tolerance, "vertex tolerance")?;
        Ok(Self { point, tolerance })
    }

    #[inline]
    pub const fn point(self) -> Point3 {
        self.point
    }

    #[inline]
    pub const fn tolerance(self) -> Real {
        self.tolerance
    }
}

/// A shared model-space edge curve and its two topological endpoint vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepEdge {
    vertices: [usize; 2],
    curve: NurbsCurve,
    tolerance: Real,
}

impl BrepEdge {
    pub fn try_new(
        vertices: [usize; 2],
        curve: NurbsCurve,
        tolerance: Real,
    ) -> Result<Self, GeometryError> {
        require_nonnegative_finite(tolerance, "edge tolerance")?;
        Ok(Self {
            vertices,
            curve,
            tolerance,
        })
    }

    #[inline]
    pub const fn vertices(&self) -> [usize; 2] {
        self.vertices
    }

    #[inline]
    pub const fn curve(&self) -> &NurbsCurve {
        &self.curve
    }

    #[inline]
    pub const fn tolerance(&self) -> Real {
        self.tolerance
    }
}

fn oriented_edge_vertices(edge: &BrepEdge, reversed: bool) -> [usize; 2] {
    if reversed {
        [edge.vertices[1], edge.vertices[0]]
    } else {
        edge.vertices
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrepTrimType {
    Boundary,
    Mated,
    Seam,
    Singular,
}

/// Isoparametric classification of a face-local trim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceIso {
    NotIso,
    /// A constant-U isocurve in the interior of the underlying surface.
    InteriorUConstant,
    /// A constant-V isocurve in the interior of the underlying surface.
    InteriorVConstant,
    South,
    East,
    North,
    West,
}

/// One corner of a rectangular surface trim in increasing U/V coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RectangularSurfaceCorner {
    SouthWest,
    SouthEast,
    NorthEast,
    NorthWest,
}

/// Endpoint description for a straight rectangular-face cut that starts at a
/// trim corner and ends on another boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangularSurfaceCornerCut {
    corner: RectangularSurfaceCorner,
    destination: Point2,
}

impl RectangularSurfaceCornerCut {
    #[inline]
    pub const fn new(corner: RectangularSurfaceCorner, destination: Point2) -> Self {
        Self {
            corner,
            destination,
        }
    }

    #[inline]
    pub const fn corner(self) -> RectangularSurfaceCorner {
        self.corner
    }

    #[inline]
    pub const fn destination(self) -> Point2 {
        self.destination
    }
}

/// One face-local use of a shared 3D edge, paired with its exact 2D p-curve.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepTrim {
    vertices: [usize; 2],
    edge: Option<usize>,
    reversed_3d: bool,
    curve: NurbsCurve2,
    trim_type: BrepTrimType,
    iso: SurfaceIso,
    tolerance: [Real; 2],
}

impl BrepTrim {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        vertices: [usize; 2],
        edge: Option<usize>,
        reversed_3d: bool,
        curve: NurbsCurve2,
        trim_type: BrepTrimType,
        iso: SurfaceIso,
        tolerance: [Real; 2],
    ) -> Result<Self, GeometryError> {
        for value in tolerance {
            require_nonnegative_finite(value, "trim tolerance")?;
        }
        if (edge.is_none()) != (trim_type == BrepTrimType::Singular) {
            return Err(GeometryError::InvalidBrepTopology {
                context: "only singular trims may omit a 3D edge",
            });
        }
        if trim_type == BrepTrimType::Singular && reversed_3d {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a singular trim cannot reverse a missing 3D edge",
            });
        }
        if trim_type == BrepTrimType::Singular && vertices[0] != vertices[1] {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a singular trim must begin and end at the same vertex",
            });
        }
        Ok(Self {
            vertices,
            edge,
            reversed_3d,
            curve,
            trim_type,
            iso,
            tolerance,
        })
    }

    #[inline]
    pub const fn vertices(&self) -> [usize; 2] {
        self.vertices
    }

    #[inline]
    pub const fn edge(&self) -> Option<usize> {
        self.edge
    }

    #[inline]
    pub const fn is_reversed_3d(&self) -> bool {
        self.reversed_3d
    }

    #[inline]
    pub const fn curve(&self) -> &NurbsCurve2 {
        &self.curve
    }

    #[inline]
    pub const fn trim_type(&self) -> BrepTrimType {
        self.trim_type
    }

    #[inline]
    pub const fn iso(&self) -> SurfaceIso {
        self.iso
    }

    #[inline]
    pub const fn tolerance(&self) -> [Real; 2] {
        self.tolerance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrepLoopType {
    Outer,
    Inner,
}

/// A closed, oriented sequence of face-local trims.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepLoop {
    loop_type: BrepLoopType,
    trims: Vec<BrepTrim>,
}

impl BrepLoop {
    pub fn try_new(loop_type: BrepLoopType, trims: Vec<BrepTrim>) -> Result<Self, GeometryError> {
        if trims.is_empty() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a face loop must contain at least one trim",
            });
        }
        Ok(Self { loop_type, trims })
    }

    #[inline]
    pub const fn loop_type(&self) -> BrepLoopType {
        self.loop_type
    }

    #[inline]
    pub fn trims(&self) -> &[BrepTrim] {
        &self.trims
    }
}

/// A trimmed NURBS face. `reversed` flips the natural surface normal without
/// changing the counterclockwise parameter-space orientation of its outer loop.
#[derive(Clone, Debug, PartialEq)]
pub struct BrepFace {
    surface: NurbsSurface,
    reversed: bool,
    loops: Vec<BrepLoop>,
}

impl BrepFace {
    pub fn try_new(
        surface: NurbsSurface,
        reversed: bool,
        loops: Vec<BrepLoop>,
    ) -> Result<Self, GeometryError> {
        if loops.is_empty() || loops[0].loop_type != BrepLoopType::Outer {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a face must begin with one outer loop",
            });
        }
        if loops[1..]
            .iter()
            .any(|face_loop| face_loop.loop_type != BrepLoopType::Inner)
        {
            return Err(GeometryError::InvalidBrepTopology {
                context: "only the first face loop may be outer",
            });
        }
        Ok(Self {
            surface,
            reversed,
            loops,
        })
    }

    #[inline]
    pub const fn surface(&self) -> &NurbsSurface {
        &self.surface
    }

    #[inline]
    pub const fn is_reversed(&self) -> bool {
        self.reversed
    }

    #[inline]
    pub fn loops(&self) -> &[BrepLoop] {
        &self.loops
    }

    /// Returns the exact underlying U-isocurve portions inside this face's
    /// parameter-space trim region.
    pub fn isocurve_u_segments(
        &self,
        v: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let curve = self.surface.isocurve_u(v)?;
        let intervals = trimmed_isocurve_intervals(self, 0, v, tolerance)?;
        trim_isocurve_to_intervals(curve, intervals)
    }

    /// Returns the exact underlying V-isocurve portions inside this face's
    /// parameter-space trim region.
    pub fn isocurve_v_segments(
        &self,
        u: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        let curve = self.surface.isocurve_v(u)?;
        let intervals = trimmed_isocurve_intervals(self, 1, u, tolerance)?;
        trim_isocurve_to_intervals(curve, intervals)
    }

    /// Tests whether a natural surface parameter lies on or inside the face's
    /// exact trim region.
    pub fn contains_parameters(
        &self,
        u: Real,
        v: Real,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        let u_domain = self.surface.domain_u();
        let v_domain = self.surface.domain_v();
        require_finite([u, v], "B-rep face parameters")?;
        if u < *u_domain.start()
            || u > *u_domain.end()
            || v < *v_domain.start()
            || v > *v_domain.end()
        {
            return Ok(false);
        }
        let intervals = trimmed_isocurve_intervals(self, 0, v, tolerance)?;
        let epsilon = trim_parameter_epsilon([*u_domain.start(), *u_domain.end()], tolerance);
        Ok(intervals
            .iter()
            .any(|interval| parameter_interval_contains(*interval, u, epsilon)))
    }

    /// Returns the exact U/V bounds when the outer trim is one
    /// counterclockwise axis-aligned rectangle with no holes.
    pub fn rectangular_trim_bounds(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<[[Real; 2]; 2]>, GeometryError> {
        rectangular_face_trim_bounds(self, tolerance)
    }
}

/// A validated boundary representation with shared model-space topology and
/// exact face-local parameter-space trims.
#[derive(Clone, Debug, PartialEq)]
pub struct Brep {
    vertices: Vec<BrepVertex>,
    edges: Vec<BrepEdge>,
    faces: Vec<BrepFace>,
}

impl Brep {
    pub fn try_new(
        vertices: Vec<BrepVertex>,
        edges: Vec<BrepEdge>,
        faces: Vec<BrepFace>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if vertices.is_empty() || edges.is_empty() || faces.is_empty() {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a B-rep requires vertices, edges, and faces",
            });
        }
        let brep = Self {
            vertices,
            edges,
            faces,
        };
        brep.validate(tolerance)?;
        Ok(brep)
    }

    /// Wraps a NURBS surface as one exact natural-domain face.
    pub fn try_surface_face(
        surface: NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let u = surface.domain_u();
        let v = surface.domain_v();
        Self::try_rectangular_surface_face(
            surface.clone(),
            *u.start()..=*u.end(),
            *v.start()..=*v.end(),
            tolerance,
        )
    }

    /// Builds one exact face whose rectangular trim lies in the supplied
    /// subdomain while retaining the complete underlying NURBS surface.
    ///
    /// Closed directions share one seam edge between their two trims, and
    /// collapsed sides become singular trims without a 3D edge. Interior
    /// constant-U and constant-V trims retain their OpenNURBS isoparametric
    /// classes.
    pub fn try_rectangular_surface_face(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_rectangular_surface_face_with_orientation(surface, u, v, false, tolerance)
    }

    /// Builds the same rectangular face while explicitly preserving its
    /// orientation relative to the underlying surface.
    pub fn try_rectangular_surface_face_with_orientation(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            [*u.start(), *u.end(), *v.start(), *v.end()],
            "rectangular surface-face trim bounds",
        )?;
        // Reuse the exact tensor trimmer's domain validation without changing
        // the underlying surface retained by this face.
        surface.try_trimmed(u.clone(), v.clone())?;
        let bounds = [[*u.start(), *u.end()], [*v.start(), *v.end()]];
        let corner_points = [
            surface.evaluate(bounds[0][0], bounds[1][0])?,
            surface.evaluate(bounds[0][1], bounds[1][0])?,
            surface.evaluate(bounds[0][1], bounds[1][1])?,
            surface.evaluate(bounds[0][0], bounds[1][1])?,
        ];
        let side_curves = [
            surface
                .isocurve_u(bounds[1][0])?
                .try_trimmed(bounds[0][0]..=bounds[0][1])?,
            surface
                .isocurve_v(bounds[0][1])?
                .try_trimmed(bounds[1][0]..=bounds[1][1])?,
            surface
                .isocurve_u(bounds[1][1])?
                .try_trimmed(bounds[0][0]..=bounds[0][1])?
                .reversed()?,
            surface
                .isocurve_v(bounds[0][0])?
                .try_trimmed(bounds[1][0]..=bounds[1][1])?
                .reversed()?,
        ];
        let singular = side_curves.each_ref().map(|curve| {
            let first = curve.control_points()[0].point();
            curve
                .control_points()
                .iter()
                .all(|control| control.point() == first)
        });

        // Join corner records only where the intervening topological side
        // closes or collapses. Coincident points on unrelated sides remain
        // distinct vertices, as required at self-intersections.
        let mut corner_groups = [0, 1, 2, 3];
        for side in 0..4 {
            if corner_points[side].distance_to(corner_points[(side + 1) % 4])?
                <= tolerance.absolute()
            {
                let first = corner_groups[side];
                let second = corner_groups[(side + 1) % 4];
                for group in &mut corner_groups {
                    if *group == second {
                        *group = first;
                    }
                }
            }
        }
        let surface_u = surface.domain_u();
        let surface_v = surface.domain_v();
        let closed_u = bounds[0][0] == *surface_u.start()
            && bounds[0][1] == *surface_u.end()
            && surface.is_closed_u()?;
        let closed_v = bounds[1][0] == *surface_v.start()
            && bounds[1][1] == *surface_v.end()
            && surface.is_closed_v()?;
        let seam_sides = [closed_v, closed_u, closed_v, closed_u];

        let mut group_vertices = [usize::MAX; 4];
        let mut corner_vertices = [usize::MAX; 4];
        let mut vertices = Vec::new();
        for corner in 0..4 {
            let group = corner_groups[corner];
            if group_vertices[group] == usize::MAX {
                group_vertices[group] = vertices.len();
                vertices.push(BrepVertex::try_new(corner_points[corner], 0.0)?);
            }
            corner_vertices[corner] = group_vertices[group];
        }

        let mut edge_indices = [None; 4];
        let mut reversed_3d = [false; 4];
        let mut edges = Vec::new();
        for side in 0..4 {
            if singular[side] {
                continue;
            }
            let paired_side = match side {
                2 if closed_v && !singular[0] => Some(0),
                3 if closed_u && !singular[1] => Some(1),
                _ => None,
            };
            if let Some(paired_side) = paired_side {
                edge_indices[side] = edge_indices[paired_side];
                reversed_3d[side] = true;
                continue;
            }
            edge_indices[side] = Some(edges.len());
            edges.push(BrepEdge::try_new(
                [corner_vertices[side], corner_vertices[(side + 1) % 4]],
                side_curves[side].clone(),
                0.0,
            )?);
        }
        let iso = [
            if bounds[1][0] == *surface_v.start() {
                SurfaceIso::South
            } else {
                SurfaceIso::InteriorVConstant
            },
            if bounds[0][1] == *surface_u.end() {
                SurfaceIso::East
            } else {
                SurfaceIso::InteriorUConstant
            },
            if bounds[1][1] == *surface_v.end() {
                SurfaceIso::North
            } else {
                SurfaceIso::InteriorVConstant
            },
            if bounds[0][0] == *surface_u.start() {
                SurfaceIso::West
            } else {
                SurfaceIso::InteriorUConstant
            },
        ];
        let parameter_corners = [
            Point2::try_new(bounds[0][0], bounds[1][0])?,
            Point2::try_new(bounds[0][1], bounds[1][0])?,
            Point2::try_new(bounds[0][1], bounds[1][1])?,
            Point2::try_new(bounds[0][0], bounds[1][1])?,
        ];
        let trims = (0..4)
            .map(|side| {
                let trim_type = if singular[side] {
                    BrepTrimType::Singular
                } else if seam_sides[side] {
                    BrepTrimType::Seam
                } else {
                    BrepTrimType::Boundary
                };
                BrepTrim::try_new(
                    [corner_vertices[side], corner_vertices[(side + 1) % 4]],
                    edge_indices[side],
                    reversed_3d[side],
                    NurbsCurve2::try_line(
                        parameter_corners[side],
                        parameter_corners[(side + 1) % 4],
                    )?,
                    trim_type,
                    iso[side],
                    [0.0, 0.0],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let face = BrepFace::try_new(
            surface,
            reversed,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?;
        Self::try_new(vertices, edges, vec![face], tolerance)
    }

    /// Splits one rectangular surface region at an interior constant-U
    /// isocurve while retaining the complete underlying surface in both
    /// pieces.
    ///
    /// Ordinary four-sided faces use the vertex, edge, and trim ordering
    /// produced by Rhino's cutting-object `Split`. Closed seams and singular
    /// sides retain the canonical rectangular-face topology.
    pub fn try_split_rectangular_surface_face_u(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        parameter: Real,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite([parameter], "rectangular surface-face U split parameter")?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let west = Self::try_rectangular_surface_face_with_orientation(
            surface.clone(),
            u_start..=parameter,
            v_start..=v_end,
            reversed,
            tolerance,
        )?;
        let east = Self::try_rectangular_surface_face_with_orientation(
            surface,
            parameter..=u_end,
            v_start..=v_end,
            reversed,
            tolerance,
        )?;
        Ok([
            reorder_cutting_split_rectangle(
                west,
                [0, 3, 1, 2],
                [0, 3, 1, 2],
                [1, 2, 3, 0],
                None,
                tolerance,
            )?,
            reorder_cutting_split_rectangle(
                east,
                [1, 2, 0, 3],
                [1, 2, 3, 0],
                [0, 1, 2, 3],
                Some(3),
                tolerance,
            )?,
        ])
    }

    /// Splits one rectangular surface region at an interior constant-V
    /// isocurve while retaining the complete underlying surface in both
    /// pieces. See [`Self::try_split_rectangular_surface_face_u`] for topology
    /// ordering details.
    pub fn try_split_rectangular_surface_face_v(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        parameter: Real,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite([parameter], "rectangular surface-face V split parameter")?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let south = Self::try_rectangular_surface_face_with_orientation(
            surface.clone(),
            u_start..=u_end,
            v_start..=parameter,
            reversed,
            tolerance,
        )?;
        let north = Self::try_rectangular_surface_face_with_orientation(
            surface,
            u_start..=u_end,
            parameter..=v_end,
            reversed,
            tolerance,
        )?;
        Ok([
            reorder_cutting_split_rectangle(
                south,
                [0, 1, 3, 2],
                [0, 1, 2, 3],
                [3, 0, 1, 2],
                Some(2),
                tolerance,
            )?,
            reorder_cutting_split_rectangle(
                north,
                [2, 3, 0, 1],
                [2, 3, 0, 1],
                [0, 1, 2, 3],
                None,
                tolerance,
            )?,
        ])
    }

    /// Splits a rectangular surface region along one exact curve running from
    /// its west side to its east side. The supplied curve is oriented (or
    /// reversed) from west to east and retained as the shared boundary of the
    /// two independent result B-reps.
    ///
    /// The straight parameter-space trim is validated against the 3D curve,
    /// so this constructor rejects surface parameterizations for which the
    /// proposed p-curve would only be an approximation.
    pub fn try_split_rectangular_surface_face_west_east(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular west-east surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [west_v, east_v] = side_parameters;
        if west_v <= v_start || west_v >= v_end || east_v <= v_start || east_v >= v_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a west-east surface split must end inside both opposite sides",
            });
        }
        let west_cut = Point2::try_new(u_start, west_v)?;
        let east_cut = Point2::try_new(u_end, east_v)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(u_start, west_v)?,
            surface.evaluate(u_end, east_v)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, west_cut, east_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let south = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_start, v_start)?,
                Point2::try_new(u_end, v_start)?,
                west_cut,
                east_cut,
            ],
            [
                (
                    [0, 1],
                    surface.isocurve_u(v_start)?.try_trimmed(u_start..=u_end)?,
                ),
                (
                    [1, 3],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=east_v)?,
                ),
                ([2, 3], cut_curve.clone()),
                (
                    [2, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=west_v)?
                        .reversed()?,
                ),
            ],
            [
                (3, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
                (1, false, boundary_iso[1]),
                (2, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let north = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_end, v_end)?,
                Point2::try_new(u_start, v_end)?,
                west_cut,
                east_cut,
            ],
            [
                (
                    [0, 1],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=u_end)?
                        .reversed()?,
                ),
                (
                    [1, 2],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(west_v..=v_end)?
                        .reversed()?,
                ),
                ([2, 3], cut_curve),
                (
                    [3, 0],
                    surface.isocurve_v(u_end)?.try_trimmed(east_v..=v_end)?,
                ),
            ],
            [
                (2, false, SurfaceIso::NotIso),
                (3, false, boundary_iso[1]),
                (0, false, boundary_iso[2]),
                (1, false, boundary_iso[3]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([south, north])
    }

    /// Splits a rectangular surface region along one exact curve running from
    /// its south side to its north side. This is the transposed counterpart of
    /// [`Self::try_split_rectangular_surface_face_west_east`].
    pub fn try_split_rectangular_surface_face_south_north(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular south-north surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [south_u, north_u] = side_parameters;
        if south_u <= u_start || south_u >= u_end || north_u <= u_start || north_u >= u_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a south-north surface split must end inside both opposite sides",
            });
        }
        let south_cut = Point2::try_new(south_u, v_start)?;
        let north_cut = Point2::try_new(north_u, v_end)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(south_u, v_start)?,
            surface.evaluate(north_u, v_end)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, south_cut, north_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let west = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_start, v_start)?,
                Point2::try_new(u_start, v_end)?,
                south_cut,
                north_cut,
            ],
            [
                (
                    [0, 2],
                    surface
                        .isocurve_u(v_start)?
                        .try_trimmed(u_start..=south_u)?,
                ),
                (
                    [1, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=v_end)?
                        .reversed()?,
                ),
                ([2, 3], cut_curve.clone()),
                (
                    [3, 1],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=north_u)?
                        .reversed()?,
                ),
            ],
            [
                (2, false, SurfaceIso::NotIso),
                (3, false, boundary_iso[2]),
                (1, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let east = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_end, v_start)?,
                Point2::try_new(u_end, v_end)?,
                south_cut,
                north_cut,
            ],
            [
                (
                    [0, 1],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=v_end)?,
                ),
                (
                    [1, 3],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(north_u..=u_end)?
                        .reversed()?,
                ),
                ([2, 3], cut_curve),
                (
                    [2, 0],
                    surface.isocurve_u(v_start)?.try_trimmed(south_u..=u_end)?,
                ),
            ],
            [
                (3, false, boundary_iso[0]),
                (0, false, boundary_iso[1]),
                (1, false, boundary_iso[2]),
                (2, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([west, east])
    }

    /// Splits a rectangular surface region along one exact curve joining the
    /// interiors of its south and east sides. The result ordering and the
    /// triangle/pentagon topology match Rhino's cutting-object `Split`.
    pub fn try_split_rectangular_surface_face_south_east(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular south-east surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [south_u, east_v] = side_parameters;
        if south_u <= u_start || south_u >= u_end || east_v <= v_start || east_v >= v_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a south-east surface split must end inside both adjacent sides",
            });
        }
        let south_cut = Point2::try_new(south_u, v_start)?;
        let east_cut = Point2::try_new(u_end, east_v)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(south_u, v_start)?,
            surface.evaluate(u_end, east_v)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, south_cut, east_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let remainder = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_start, v_start)?,
                Point2::try_new(u_end, v_end)?,
                Point2::try_new(u_start, v_end)?,
                south_cut,
                east_cut,
            ],
            [
                (
                    [0, 3],
                    surface
                        .isocurve_u(v_start)?
                        .try_trimmed(u_start..=south_u)?,
                ),
                (
                    [1, 2],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=u_end)?
                        .reversed()?,
                ),
                (
                    [2, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=v_end)?
                        .reversed()?,
                ),
                ([3, 4], cut_curve.clone()),
                (
                    [4, 1],
                    surface.isocurve_v(u_end)?.try_trimmed(east_v..=v_end)?,
                ),
            ],
            [
                (3, false, SurfaceIso::NotIso),
                (4, false, boundary_iso[1]),
                (1, false, boundary_iso[2]),
                (2, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let corner = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [Point2::try_new(u_end, v_start)?, south_cut, east_cut],
            [
                (
                    [0, 2],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=east_v)?,
                ),
                ([1, 2], cut_curve),
                (
                    [1, 0],
                    surface.isocurve_u(v_start)?.try_trimmed(south_u..=u_end)?,
                ),
            ],
            [
                (2, false, boundary_iso[0]),
                (0, false, boundary_iso[1]),
                (1, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([remainder, corner])
    }

    /// Splits a rectangular surface region along one exact curve joining the
    /// interiors of its east and north sides. The result ordering and the
    /// triangle/pentagon topology match Rhino's cutting-object `Split`.
    pub fn try_split_rectangular_surface_face_east_north(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular east-north surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [east_v, north_u] = side_parameters;
        if east_v <= v_start || east_v >= v_end || north_u <= u_start || north_u >= u_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "an east-north surface split must end inside both adjacent sides",
            });
        }
        let east_cut = Point2::try_new(u_end, east_v)?;
        let north_cut = Point2::try_new(north_u, v_end)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(u_end, east_v)?,
            surface.evaluate(north_u, v_end)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, east_cut, north_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let remainder = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_start, v_start)?,
                Point2::try_new(u_end, v_start)?,
                Point2::try_new(u_start, v_end)?,
                east_cut,
                north_cut,
            ],
            [
                (
                    [0, 1],
                    surface.isocurve_u(v_start)?.try_trimmed(u_start..=u_end)?,
                ),
                (
                    [1, 3],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=east_v)?,
                ),
                (
                    [2, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=v_end)?
                        .reversed()?,
                ),
                ([3, 4], cut_curve.clone()),
                (
                    [4, 2],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=north_u)?
                        .reversed()?,
                ),
            ],
            [
                (3, false, SurfaceIso::NotIso),
                (4, false, boundary_iso[2]),
                (2, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
                (1, false, boundary_iso[1]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let corner = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [Point2::try_new(u_end, v_end)?, east_cut, north_cut],
            [
                (
                    [0, 2],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(north_u..=u_end)?
                        .reversed()?,
                ),
                ([1, 2], cut_curve),
                (
                    [1, 0],
                    surface.isocurve_v(u_end)?.try_trimmed(east_v..=v_end)?,
                ),
            ],
            [
                (2, false, boundary_iso[1]),
                (0, false, boundary_iso[2]),
                (1, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([remainder, corner])
    }

    /// Splits a rectangular surface region along one exact curve joining the
    /// interiors of its north and west sides. The result ordering and the
    /// triangle/pentagon topology match Rhino's cutting-object `Split`.
    pub fn try_split_rectangular_surface_face_north_west(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular north-west surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [north_u, west_v] = side_parameters;
        if north_u <= u_start || north_u >= u_end || west_v <= v_start || west_v >= v_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a north-west surface split must end inside both adjacent sides",
            });
        }
        let north_cut = Point2::try_new(north_u, v_end)?;
        let west_cut = Point2::try_new(u_start, west_v)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(north_u, v_end)?,
            surface.evaluate(u_start, west_v)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, north_cut, west_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let remainder = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_start, v_start)?,
                Point2::try_new(u_end, v_start)?,
                Point2::try_new(u_end, v_end)?,
                north_cut,
                west_cut,
            ],
            [
                (
                    [0, 1],
                    surface.isocurve_u(v_start)?.try_trimmed(u_start..=u_end)?,
                ),
                (
                    [1, 2],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=v_end)?,
                ),
                (
                    [2, 3],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(north_u..=u_end)?
                        .reversed()?,
                ),
                ([3, 4], cut_curve.clone()),
                (
                    [4, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=west_v)?
                        .reversed()?,
                ),
            ],
            [
                (3, false, SurfaceIso::NotIso),
                (4, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
                (1, false, boundary_iso[1]),
                (2, false, boundary_iso[2]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let corner = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [Point2::try_new(u_start, v_end)?, north_cut, west_cut],
            [
                (
                    [0, 2],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(west_v..=v_end)?
                        .reversed()?,
                ),
                ([1, 2], cut_curve),
                (
                    [1, 0],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=north_u)?
                        .reversed()?,
                ),
            ],
            [
                (2, false, boundary_iso[2]),
                (0, false, boundary_iso[3]),
                (1, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([remainder, corner])
    }

    /// Splits a rectangular surface region along one exact curve joining the
    /// interiors of its west and south sides. Rhino returns this corner's
    /// triangle before the complementary pentagon, unlike the other three
    /// adjacent-side orientations; this constructor preserves that ordering.
    pub fn try_split_rectangular_surface_face_west_south(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        side_parameters: [Real; 2],
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            side_parameters,
            "rectangular west-south surface split parameters",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let [u_start, u_end] = [*u.start(), *u.end()];
        let [v_start, v_end] = [*v.start(), *v.end()];
        let [west_v, south_u] = side_parameters;
        if west_v <= v_start || west_v >= v_end || south_u <= u_start || south_u >= u_end {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a west-south surface split must end inside both adjacent sides",
            });
        }
        let west_cut = Point2::try_new(u_start, west_v)?;
        let south_cut = Point2::try_new(south_u, v_start)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(u_start, west_v)?,
            surface.evaluate(south_u, v_start)?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, west_cut, south_cut, tolerance)?;
        let boundary_iso =
            rectangular_surface_boundary_iso(&surface, [[u_start, u_end], [v_start, v_end]]);

        let corner = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [Point2::try_new(u_start, v_start)?, west_cut, south_cut],
            [
                (
                    [0, 2],
                    surface
                        .isocurve_u(v_start)?
                        .try_trimmed(u_start..=south_u)?,
                ),
                ([1, 2], cut_curve.clone()),
                (
                    [1, 0],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(v_start..=west_v)?
                        .reversed()?,
                ),
            ],
            [
                (2, false, boundary_iso[3]),
                (0, false, boundary_iso[0]),
                (1, true, SurfaceIso::NotIso),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        let remainder = try_surface_cutting_face(
            surface.clone(),
            reversed,
            [
                Point2::try_new(u_end, v_start)?,
                Point2::try_new(u_end, v_end)?,
                Point2::try_new(u_start, v_end)?,
                west_cut,
                south_cut,
            ],
            [
                (
                    [0, 1],
                    surface.isocurve_v(u_end)?.try_trimmed(v_start..=v_end)?,
                ),
                (
                    [1, 2],
                    surface
                        .isocurve_u(v_end)?
                        .try_trimmed(u_start..=u_end)?
                        .reversed()?,
                ),
                (
                    [2, 3],
                    surface
                        .isocurve_v(u_start)?
                        .try_trimmed(west_v..=v_end)?
                        .reversed()?,
                ),
                ([3, 4], cut_curve),
                (
                    [4, 0],
                    surface.isocurve_u(v_start)?.try_trimmed(south_u..=u_end)?,
                ),
            ],
            [
                (3, false, SurfaceIso::NotIso),
                (4, false, boundary_iso[0]),
                (0, false, boundary_iso[1]),
                (1, false, boundary_iso[2]),
                (2, false, boundary_iso[3]),
            ],
            &cut_parameter_curve,
            tolerance,
        )?;
        Ok([corner, remainder])
    }

    /// Splits a rectangular surface region along one exact curve from a trim
    /// corner to either the opposite corner or the interior of a nonincident
    /// side. Existing corner topology is reused rather than duplicated, and
    /// the two independent triangle/quad results retain Rhino's vertex, edge,
    /// and trim ordering.
    pub fn try_split_rectangular_surface_face_from_corner(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        cut: RectangularSurfaceCornerCut,
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        surface.try_trimmed(u.clone(), v.clone())?;
        let bounds = [[*u.start(), *u.end()], [*v.start(), *v.end()]];
        let (kind, destination) =
            classify_rectangular_corner_cut(cut.corner(), cut.destination(), bounds, tolerance)?;
        let [cut_start, cut_end] = rectangular_corner_cut_parameters(kind, destination, bounds)?;
        let cut_curve = orient_surface_split_curve(
            &cut_curve,
            surface.evaluate(cut_start.x(), cut_start.y())?,
            surface.evaluate(cut_end.x(), cut_end.y())?,
            tolerance,
        )?;
        let cut_parameter_curve =
            surface_split_parameter_curve(&surface, &cut_curve, cut_start, cut_end, tolerance)?;
        let [first, second] = rectangular_corner_cut_face_specs(kind);
        Ok([
            try_rectangular_corner_cut_face(
                surface.clone(),
                reversed,
                bounds,
                destination,
                SurfaceSplitCurveRef {
                    spatial: &cut_curve,
                    parameter: &cut_parameter_curve,
                },
                first,
                tolerance,
            )?,
            try_rectangular_corner_cut_face(
                surface,
                reversed,
                bounds,
                destination,
                SurfaceSplitCurveRef {
                    spatial: &cut_curve,
                    parameter: &cut_parameter_curve,
                },
                second,
                tolerance,
            )?,
        ])
    }

    /// Splits a rectangular surface region along one exact simple closed curve
    /// wholly inside its trim bounds.
    ///
    /// The first result retains the rectangular outer loop and uses the cut as
    /// a clockwise inner loop. The second result uses the same cut
    /// counterclockwise as its outer loop. Smooth closed cutters remain one
    /// edge, while interior degree-multiple kinks become separate edges,
    /// matching Rhino's cutting-object `Split` topology.
    pub fn try_split_rectangular_surface_face_with_closed_curve(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        cut_curve: NurbsCurve,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<[Self; 2], GeometryError> {
        require_finite(
            [*u.start(), *u.end(), *v.start(), *v.end()],
            "rectangular closed surface split bounds",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        if !cut_curve.is_closed()? {
            return invalid("a closed surface split requires a closed cutting curve");
        }
        let bounds = [[*u.start(), *u.end()], [*v.start(), *v.end()]];
        let seam = cut_curve.evaluate(*cut_curve.domain().start())?;
        let (seam_u, seam_v) = surface.closest_parameters(seam, tolerance)?;
        let seam_parameter = Point2::try_new(seam_u, seam_v)?;
        let parameter_curve = surface_split_parameter_curve(
            &surface,
            &cut_curve,
            seam_parameter,
            seam_parameter,
            tolerance,
        )?;
        validate_closed_surface_cut_parameter_curve(&parameter_curve, bounds, tolerance)?;
        let (_, source_reversed_for_outer) = oriented_cap_curve(parameter_curve.clone())?;
        let segments =
            closed_surface_cut_segments(&surface, &cut_curve, &parameter_curve, bounds, tolerance)?;

        let cut_points = segments
            .iter()
            .map(|segment| segment.spatial.evaluate(*segment.spatial.domain().start()))
            .collect::<Result<Vec<_>, _>>()?;
        let cut_vertices = cut_points
            .iter()
            .copied()
            .map(|point| BrepVertex::try_new(point, 0.0))
            .collect::<Result<Vec<_>, _>>()?;

        let mut outside = Self::try_rectangular_surface_face_with_orientation(
            surface.clone(),
            u,
            v,
            reversed,
            tolerance,
        )?;
        let outside_vertex_offset = outside.vertices.len();
        let outside_edge_offset = outside.edges.len();
        outside.vertices.extend(cut_vertices.iter().copied());
        outside
            .edges
            .extend(closed_surface_cut_edges(&segments, outside_vertex_offset)?);
        outside.faces[0].loops.push(closed_surface_cut_loop(
            &segments,
            outside_vertex_offset,
            outside_edge_offset,
            BrepLoopType::Inner,
            !source_reversed_for_outer,
        )?);
        let outside = Self::try_new(outside.vertices, outside.edges, outside.faces, tolerance)?;

        let inside_edges = closed_surface_cut_edges(&segments, 0)?;
        let inside_loop = closed_surface_cut_loop(
            &segments,
            0,
            0,
            BrepLoopType::Outer,
            source_reversed_for_outer,
        )?;
        let inside_face = BrepFace::try_new(surface, reversed, vec![inside_loop])?;
        let inside = Self::try_new(cut_vertices, inside_edges, vec![inside_face], tolerance)?;
        Ok([outside, inside])
    }

    /// Splits one rectangular surface region by an arrangement of exact,
    /// simple boundary-to-boundary and closed interior curves.
    ///
    /// Every transverse curve/curve intersection becomes a shared
    /// parameter-space node. The curves and rectangular boundary are split at
    /// those nodes, and every bounded counterclockwise cell is returned as an
    /// independent one-face B-rep retaining the complete underlying surface.
    /// Nested and disjoint closed curves become inner loops on their containing
    /// faces. Coincident complete cutters are ignored; partially coincident
    /// cutters and open cutters which do not end on the rectangular boundary
    /// are rejected.
    ///
    /// Curved cutters require an exact affine pullback to the surface, matching
    /// the single-curve split constructors. Isoparametric cutters work on any
    /// surface because their p-curves are exact parameter-space lines.
    pub fn try_split_rectangular_surface_face_with_curves(
        surface: NurbsSurface,
        u: RangeInclusive<Real>,
        v: RangeInclusive<Real>,
        cut_curves: impl IntoIterator<Item = NurbsCurve>,
        reversed: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        require_finite(
            [*u.start(), *u.end(), *v.start(), *v.end()],
            "rectangular surface cutting arrangement bounds",
        )?;
        surface.try_trimmed(u.clone(), v.clone())?;
        let cut_curves = cut_curves.into_iter().collect::<Vec<_>>();
        if cut_curves.is_empty() {
            return invalid("a surface cutting arrangement requires at least one curve");
        }
        try_rectangular_surface_cut_arrangement(
            surface,
            [[*u.start(), *u.end()], [*v.start(), *v.end()]],
            cut_curves,
            reversed,
            tolerance,
        )
    }

    /// Converts every polygon of a mesh into one degree-one NURBS face.
    ///
    /// Exact-location mesh vertices and edges become shared B-rep topology.
    /// Quads retain their potentially warped bilinear shape. Triangles become
    /// either trimmed planar parallelograms or untrimmed bilinear surfaces
    /// whose west side is collapsed, matching Rhino's `Brep.CreateFromMesh`
    /// construction and parameter domains.
    pub fn try_from_mesh(
        mesh: &TriangleMesh,
        trim_triangular_faces: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut vertex_indices = BTreeMap::<[u64; 3], usize>::new();
        let mut vertices = Vec::new();
        let mut raw_to_brep = Vec::with_capacity(mesh.vertices().len());
        for &point in mesh.vertices() {
            let key = point.to_array().map(canonical_brep_coordinate_bits);
            let index = if let Some(&index) = vertex_indices.get(&key) {
                index
            } else {
                let index = vertices.len();
                vertices.push(BrepVertex::try_new(point, 0.0)?);
                vertex_indices.insert(key, index);
                index
            };
            raw_to_brep.push(index);
        }

        // OpenNURBS builds the complete mesh-topology vertex table, marks
        // vertices touched by faces, and then compacts it. Preserve that
        // order even when an unused raw vertex is the first occurrence of a
        // location referenced again later.
        let mut used_vertices = vec![false; vertices.len()];
        for face in mesh.faces() {
            for &raw in face.indices() {
                used_vertices[raw_to_brep[raw as usize]] = true;
            }
        }
        let mut vertex_remap = vec![usize::MAX; vertices.len()];
        let mut retained_vertices =
            Vec::with_capacity(used_vertices.iter().filter(|&&used| used).count());
        for (source, (vertex, used)) in vertices.into_iter().zip(used_vertices).enumerate() {
            if used {
                vertex_remap[source] = retained_vertices.len();
                retained_vertices.push(vertex);
            }
        }
        for vertex in &mut raw_to_brep {
            *vertex = vertex_remap[*vertex];
        }
        let vertices = retained_vertices;

        let mut edge_use_counts = BTreeMap::<(usize, usize), usize>::new();
        for face in mesh.faces() {
            let indices = face.indices();
            for side in 0..indices.len() {
                let start = raw_to_brep[indices[side] as usize];
                let end = raw_to_brep[indices[(side + 1) % indices.len()] as usize];
                let key = ordered_pair(start, end);
                let uses = edge_use_counts.entry(key).or_default();
                *uses = uses.checked_add(1).ok_or(GeometryError::TooManyMeshFaces)?;
            }
        }
        let edge_keys = edge_use_counts.keys().copied().collect::<Vec<_>>();
        let edge_indices = edge_keys
            .iter()
            .copied()
            .enumerate()
            .map(|(index, key)| (key, index))
            .collect::<BTreeMap<_, _>>();
        let edges = edge_keys
            .iter()
            .map(|&(start, end)| {
                BrepEdge::try_new(
                    [start, end],
                    NurbsCurve::try_new(
                        1,
                        vec![vertices[start].point, vertices[end].point],
                        vec![0.0, 0.0, 1.0, 1.0],
                    )?,
                    0.0,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        let mut faces = Vec::with_capacity(mesh.face_count());
        for face in mesh.faces() {
            let face_vertices = face
                .indices()
                .iter()
                .map(|&raw| raw_to_brep[raw as usize])
                .collect::<Vec<_>>();
            let face_points = face_vertices
                .iter()
                .map(|&vertex| vertices[vertex].point)
                .collect::<Vec<_>>();
            let surface_corners = match *face {
                MeshFace::Triangle(_) if trim_triangular_faces => {
                    let fourth =
                        face_points[0].translated(face_points[1].vector_to(face_points[2])?)?;
                    [face_points[0], face_points[1], face_points[2], fourth]
                }
                MeshFace::Triangle(_) => [
                    face_points[0],
                    face_points[1],
                    face_points[2],
                    face_points[0],
                ],
                MeshFace::Quad(_) => [
                    face_points[0],
                    face_points[1],
                    face_points[2],
                    face_points[3],
                ],
            };
            let surface = mesh_face_bilinear_surface(surface_corners)?;
            let domain_u = surface.domain_u();
            let domain_v = surface.domain_v();
            let parameters = [
                Point2::try_new(*domain_u.start(), *domain_v.start())?,
                Point2::try_new(*domain_u.end(), *domain_v.start())?,
                Point2::try_new(*domain_u.end(), *domain_v.end())?,
                Point2::try_new(*domain_u.start(), *domain_v.end())?,
            ];
            let (parameter_sides, iso): (&[(usize, usize)], &[SurfaceIso]) = match *face {
                MeshFace::Triangle(_) if trim_triangular_faces => (
                    &[(0, 1), (1, 2), (2, 0)],
                    &[SurfaceIso::South, SurfaceIso::East, SurfaceIso::NotIso],
                ),
                MeshFace::Triangle(_) => (
                    &[(0, 1), (1, 2), (2, 3)],
                    &[SurfaceIso::South, SurfaceIso::East, SurfaceIso::North],
                ),
                MeshFace::Quad(_) => (
                    &[(0, 1), (1, 2), (2, 3), (3, 0)],
                    &[
                        SurfaceIso::South,
                        SurfaceIso::East,
                        SurfaceIso::North,
                        SurfaceIso::West,
                    ],
                ),
            };
            let mut trims = Vec::with_capacity(
                parameter_sides.len() + usize::from(face.is_triangle() && !trim_triangular_faces),
            );
            for (side, (&(parameter_start, parameter_end), &iso)) in
                parameter_sides.iter().zip(iso).enumerate()
            {
                let start = face_vertices[side];
                let end = face_vertices[(side + 1) % face_vertices.len()];
                let edge_key = ordered_pair(start, end);
                let edge = edge_indices[&edge_key];
                let trim_type = if edge_use_counts[&edge_key] == 1 {
                    BrepTrimType::Boundary
                } else {
                    BrepTrimType::Mated
                };
                trims.push(BrepTrim::try_new(
                    [start, end],
                    Some(edge),
                    [start, end] != edges[edge].vertices,
                    NurbsCurve2::try_line(parameters[parameter_start], parameters[parameter_end])?,
                    trim_type,
                    iso,
                    [0.0, 0.0],
                )?);
            }
            if face.is_triangle() && !trim_triangular_faces {
                trims.push(BrepTrim::try_new(
                    [face_vertices[0], face_vertices[0]],
                    None,
                    false,
                    NurbsCurve2::try_line(parameters[3], parameters[0])?,
                    BrepTrimType::Singular,
                    SurfaceIso::West,
                    [0.0, 0.0],
                )?);
            }
            faces.push(BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact six-face solid box over increasing frame-axis intervals.
    pub fn try_box(
        frame: Frame3,
        intervals: [[Real; 2]; 3],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(intervals.into_iter().flatten(), "box intervals")?;
        if intervals.iter().any(|interval| interval[0] >= interval[1]) {
            return Err(GeometryError::Degenerate { context: "box" });
        }

        let origin = frame.origin().to_array();
        let axes = frame.axes().map(|axis| axis.as_vector().to_array());
        let mut vertices = Vec::with_capacity(8);
        for z in intervals[2] {
            for y in intervals[1] {
                for x in intervals[0] {
                    let parameters = [x, y, z];
                    let point = Point3::try_from(std::array::from_fn(|coordinate| {
                        parameters[0].mul_add(
                            axes[0][coordinate],
                            parameters[1].mul_add(
                                axes[1][coordinate],
                                parameters[2].mul_add(axes[2][coordinate], origin[coordinate]),
                            ),
                        )
                    }))?;
                    vertices.push(BrepVertex::try_new(point, 0.0)?);
                }
            }
        }

        let edge_vertices = [
            [0, 1],
            [1, 3],
            [2, 3],
            [0, 2],
            [4, 5],
            [5, 7],
            [6, 7],
            [4, 6],
            [0, 4],
            [1, 5],
            [2, 6],
            [3, 7],
        ];
        let edges = edge_vertices
            .into_iter()
            .map(|indices| {
                let line = LineSegment::try_new(
                    vertices[indices[0]].point,
                    vertices[indices[1]].point,
                    tolerance,
                )?;
                BrepEdge::try_new(indices, line.to_nurbs()?, 0.0)
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;

        // Every face uses a naturally outward-oriented bilinear surface and a
        // counterclockwise outer p-loop. Edge booleans are ON_BrepTrim::m_bRev3d.
        let face_specs = [
            ([0, 2, 3, 1], [3, 2, 1, 0], [false, false, true, true]),
            ([4, 5, 7, 6], [4, 5, 6, 7], [false, false, true, true]),
            ([0, 1, 5, 4], [0, 9, 4, 8], [false, false, true, true]),
            ([2, 6, 7, 3], [10, 6, 11, 2], [false, false, true, true]),
            ([0, 4, 6, 2], [8, 7, 10, 3], [false, false, true, true]),
            ([1, 3, 7, 5], [1, 11, 5, 9], [false, false, true, true]),
        ];
        let parameter_corners = [
            Point2::try_new(0.0, 0.0)?,
            Point2::try_new(1.0, 0.0)?,
            Point2::try_new(1.0, 1.0)?,
            Point2::try_new(0.0, 1.0)?,
        ];
        let iso = [
            SurfaceIso::South,
            SurfaceIso::East,
            SurfaceIso::North,
            SurfaceIso::West,
        ];
        let mut faces = Vec::with_capacity(6);
        for (corners, edge_indices, reversals) in face_specs {
            let surface = NurbsSurface::try_bilinear(corners.map(|index| vertices[index].point))?;
            let mut trims = Vec::with_capacity(4);
            for index in 0..4 {
                trims.push(BrepTrim::try_new(
                    [corners[index], corners[(index + 1) % 4]],
                    Some(edge_indices[index]),
                    reversals[index],
                    NurbsCurve2::try_line(
                        parameter_corners[index],
                        parameter_corners[(index + 1) % 4],
                    )?,
                    BrepTrimType::Mated,
                    iso[index],
                    [0.0, 0.0],
                )?);
            }
            faces.push(BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs a regular pyramid as one joined B-rep.
    ///
    /// The base starts on the frame's positive X axis and proceeds
    /// counterclockwise around its Z axis. A negative height places the apex
    /// opposite frame Z while retaining that base ordering. The triangular
    /// walls use Rhino's centroid-based planar parameterization rather than a
    /// collapsed tensor-product edge.
    pub fn try_pyramid(
        frame: Frame3,
        side_count: usize,
        radius: Real,
        height: Real,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        validate_pyramid_dimensions(side_count, [radius], height, "pyramid")?;
        let base = regular_polygon_ring(frame, side_count, radius, 0.0)?;
        let apex = frame_point(frame, 0.0, 0.0, height)?;

        // Rhino creates the first wall before the remaining ring topology, so
        // its apex is vertex 2 rather than the final vertex.
        let mut vertices = Vec::with_capacity(side_count + 1);
        vertices.push(BrepVertex::try_new(base[0], 0.0)?);
        vertices.push(BrepVertex::try_new(base[1], 0.0)?);
        vertices.push(BrepVertex::try_new(apex, 0.0)?);
        for &point in &base[2..] {
            vertices.push(BrepVertex::try_new(point, 0.0)?);
        }
        let apex_vertex = 2;
        let base_vertex = |index: usize| if index < 2 { index } else { index + 1 };

        let mut edges = Vec::with_capacity(2 * side_count);
        let mut base_edges = vec![usize::MAX; side_count];
        let mut side_edges = vec![usize::MAX; side_count];
        base_edges[0] = push_line_edge(
            &mut edges,
            &vertices,
            [base_vertex(0), base_vertex(1)],
            [0.0, base[0].distance_to(base[1])?],
        )?;
        side_edges[1] = push_line_edge(
            &mut edges,
            &vertices,
            [base_vertex(1), apex_vertex],
            [0.0, base[1].distance_to(apex)?],
        )?;
        side_edges[0] = push_line_edge(
            &mut edges,
            &vertices,
            [apex_vertex, base_vertex(0)],
            [0.0, apex.distance_to(base[0])?],
        )?;
        for index in 1..side_count {
            let next = (index + 1) % side_count;
            base_edges[index] = push_line_edge(
                &mut edges,
                &vertices,
                [base_vertex(index), base_vertex(next)],
                [0.0, base[index].distance_to(base[next])?],
            )?;
            if next != 0 {
                side_edges[next] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [base_vertex(next), apex_vertex],
                    [0.0, base[next].distance_to(apex)?],
                )?;
            }
        }

        let base_trim_type = if solid {
            BrepTrimType::Mated
        } else {
            BrepTrimType::Boundary
        };
        let mut faces = Vec::with_capacity(side_count + usize::from(solid));
        for index in 0..side_count {
            let next = (index + 1) % side_count;
            let edge_vector = base[index].vector_to(base[next])?;
            let half_edge = edge_vector.scaled(0.5)?;
            let side_length = edge_vector.length()?;
            let midpoint = base[index].translated(half_edge)?;
            let face_height = midpoint.distance_to(apex)?;
            let u = [-0.5 * side_length, 0.5 * side_length];
            let v = [-face_height / 3.0, 2.0 * face_height / 3.0];
            let surface = NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    base[index],
                    base[next],
                    apex.translated(half_edge.scaled(-1.0)?)?,
                    apex.translated(half_edge)?,
                ],
                vec![u[0], u[0], u[1], u[1]],
                vec![v[0], v[0], v[1], v[1]],
            )?;
            let parameters = [
                Point2::try_new(u[0], v[0])?,
                Point2::try_new(u[1], v[0])?,
                Point2::try_new(0.0, v[1])?,
            ];
            let loop_record = BrepLoop::try_new(
                BrepLoopType::Outer,
                vec![
                    BrepTrim::try_new(
                        [base_vertex(index), base_vertex(next)],
                        Some(base_edges[index]),
                        false,
                        NurbsCurve2::try_line(parameters[0], parameters[1])?,
                        base_trim_type,
                        SurfaceIso::South,
                        [0.0, 0.0],
                    )?,
                    BrepTrim::try_new(
                        [base_vertex(next), apex_vertex],
                        Some(side_edges[next]),
                        next == 0,
                        NurbsCurve2::try_line(parameters[1], parameters[2])?,
                        BrepTrimType::Mated,
                        SurfaceIso::NotIso,
                        [0.0, 0.0],
                    )?,
                    BrepTrim::try_new(
                        [apex_vertex, base_vertex(index)],
                        Some(side_edges[index]),
                        index != 0,
                        NurbsCurve2::try_line(parameters[2], parameters[0])?,
                        BrepTrimType::Mated,
                        SurfaceIso::NotIso,
                        [0.0, 0.0],
                    )?,
                ],
            )?;
            faces.push(BrepFace::try_new(surface, height < 0.0, vec![loop_record])?);
        }
        if solid {
            let indices = (0..side_count).map(base_vertex).collect::<Vec<_>>();
            faces.push(polygon_cap_face(
                frame,
                &base,
                &indices,
                &base_edges,
                false,
                height > 0.0,
                tolerance,
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs a regular truncated pyramid as one joined B-rep.
    ///
    /// Corresponding base and top corners share the same angular phase. Open
    /// results retain joined wall faces and expose both polygon rims as naked
    /// boundaries; solid results add exact trimmed planar caps.
    pub fn try_truncated_pyramid(
        frame: Frame3,
        side_count: usize,
        radii: [Real; 2],
        height: Real,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        validate_pyramid_dimensions(side_count, radii, height, "truncated pyramid")?;
        let base = regular_polygon_ring(frame, side_count, radii[0], 0.0)?;
        let top = regular_polygon_ring(frame, side_count, radii[1], height)?;

        // Rhino's lofted topology begins at the first top corner, crosses to
        // the first base corner, then alternates base/top corners around the
        // remaining ring.
        let mut vertices = Vec::with_capacity(2 * side_count);
        vertices.push(BrepVertex::try_new(top[0], 0.0)?);
        vertices.push(BrepVertex::try_new(base[0], 0.0)?);
        for index in 1..side_count {
            vertices.push(BrepVertex::try_new(base[index], 0.0)?);
            vertices.push(BrepVertex::try_new(top[index], 0.0)?);
        }
        let top_vertex = |index: usize| if index == 0 { 0 } else { 2 * index + 1 };
        let base_vertex = |index: usize| if index == 0 { 1 } else { 2 * index };
        let slant_length = top[0].distance_to(base[0])?;
        let base_side_length = base[0].distance_to(base[1])?;
        let top_side_length = top[0].distance_to(top[1])?;
        let equal_radii = radii[0] == radii[1];
        let wall_v = if equal_radii {
            [0.0, base_side_length]
        } else if radii[0] > radii[1] {
            [-base_side_length, 0.0]
        } else {
            [0.0, top_side_length]
        };
        // The loft keeps one common V parameterization on both polygon
        // boundaries. Consequently the shorter rim edge intentionally has a
        // non-arc-length curve domain when the two radii differ.
        let base_edge_domain = wall_v;
        let top_edge_domain = [-wall_v[1], -wall_v[0]];

        let mut edges = Vec::with_capacity(3 * side_count);
        let mut slant_edges = vec![usize::MAX; side_count];
        let mut base_edges = vec![usize::MAX; side_count];
        let mut top_edges = vec![usize::MAX; side_count];
        slant_edges[0] = push_line_edge(
            &mut edges,
            &vertices,
            [top_vertex(0), base_vertex(0)],
            [0.0, slant_length],
        )?;
        for index in 0..side_count {
            let next = (index + 1) % side_count;
            base_edges[index] = push_line_edge(
                &mut edges,
                &vertices,
                [base_vertex(index), base_vertex(next)],
                base_edge_domain,
            )?;
            if next != 0 {
                slant_edges[next] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [base_vertex(next), top_vertex(next)],
                    [-slant_length, 0.0],
                )?;
                top_edges[index] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [top_vertex(next), top_vertex(index)],
                    top_edge_domain,
                )?;
            } else {
                top_edges[index] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [top_vertex(0), top_vertex(index)],
                    top_edge_domain,
                )?;
            }
        }

        let rim_trim_type = if solid {
            BrepTrimType::Mated
        } else {
            BrepTrimType::Boundary
        };
        let mut faces = Vec::with_capacity(side_count + 2 * usize::from(solid));
        for index in 0..side_count {
            let next = (index + 1) % side_count;
            let surface = NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![top[index], base[index], top[next], base[next]],
                vec![0.0, 0.0, slant_length, slant_length],
                vec![wall_v[0], wall_v[0], wall_v[1], wall_v[1]],
            )?;
            let loop_record = rectangular_surface_loop(
                &surface,
                [
                    RectangularTrimSpec::edge(
                        [top_vertex(index), base_vertex(index)],
                        slant_edges[index],
                        index != 0,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge(
                        [base_vertex(index), base_vertex(next)],
                        base_edges[index],
                        false,
                        rim_trim_type,
                    ),
                    RectangularTrimSpec::edge(
                        [base_vertex(next), top_vertex(next)],
                        slant_edges[next],
                        next == 0,
                        BrepTrimType::Mated,
                    ),
                    RectangularTrimSpec::edge(
                        [top_vertex(next), top_vertex(index)],
                        top_edges[index],
                        false,
                        rim_trim_type,
                    ),
                ],
            )?;
            faces.push(BrepFace::try_new(surface, height < 0.0, vec![loop_record])?);
        }
        if solid {
            let base_indices = (0..side_count).map(base_vertex).collect::<Vec<_>>();
            let top_indices = (0..side_count).map(top_vertex).collect::<Vec<_>>();
            faces.push(polygon_cap_face(
                fitted_polygon_cap_frame(&base, frame.z_axis(), tolerance)?,
                &base,
                &base_indices,
                &base_edges,
                false,
                height > 0.0,
                tolerance,
            )?);
            faces.push(polygon_cap_face(
                fitted_polygon_cap_frame(&top, frame.z_axis(), tolerance)?,
                &top,
                &top_indices,
                &top_edges,
                true,
                height < 0.0,
                tolerance,
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs one exact trimmed planar face from a closed NURBS boundary.
    ///
    /// The source curve remains the sole 3D boundary edge. Its exact rational
    /// projection becomes a counterclockwise p-curve on an affine plane, so
    /// concave and curved boundaries are retained without filling the plane's
    /// unused rectangular parameter domain.
    pub fn try_planar_face(
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_planar_face_with_holes(curve, &[], tolerance)
    }

    /// Constructs one exact trimmed planar face with zero or more inner loops.
    ///
    /// Every supplied NURBS curve is retained as a distinct shared-topology
    /// boundary edge. Inner curves are projected into the outer curve's exact
    /// affine plane and oriented clockwise in parameter space. Their standard
    /// sampled trim representation is validated as disjoint holes inside the
    /// counterclockwise outer loop before topology is committed.
    pub fn try_planar_face_with_holes(
        outer: &NurbsCurve,
        inner: &[NurbsCurve],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !outer.is_closed()? || !outer.is_planar(tolerance)? {
            return Err(GeometryError::InvalidPlanarFaceBoundary);
        }

        let projection = project_planar_curve(outer, tolerance)
            .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
        let mut surface_bounds = projection.coordinate_bounds;
        let mut projected = Vec::with_capacity(inner.len() + 1);
        projected.push((
            outer,
            projection.curve,
            projection.maximum_residual,
            BrepLoopType::Outer,
        ));
        for curve in inner {
            if !curve.is_closed()? {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            let (parameter_curve, maximum_residual) =
                project_curve_to_frame(curve, projection.frame, tolerance)
                    .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            for control in parameter_curve.control_points() {
                let point = control.point();
                surface_bounds[0][0] = surface_bounds[0][0].min(point.x());
                surface_bounds[0][1] = surface_bounds[0][1].max(point.x());
                surface_bounds[1][0] = surface_bounds[1][0].min(point.y());
                surface_bounds[1][1] = surface_bounds[1][1].max(point.y());
            }
            projected.push((
                curve,
                parameter_curve,
                maximum_residual,
                BrepLoopType::Inner,
            ));
        }
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let surface = planar_cap_surface(projection.frame, zero, surface_bounds)?;
        let mut vertices = Vec::with_capacity(projected.len());
        let mut edges = Vec::with_capacity(projected.len());
        let mut loops = Vec::with_capacity(projected.len());
        for (index, (curve, parameter_curve, maximum_residual, loop_type)) in
            projected.into_iter().enumerate()
        {
            let (mut parameter_curve, mut curve_reversed) = oriented_cap_curve(parameter_curve)
                .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            if loop_type == BrepLoopType::Inner {
                parameter_curve = parameter_curve.reversed()?;
                curve_reversed = !curve_reversed;
            }
            let seam = curve.evaluate(*curve.domain().start())?;
            let closure_tolerance = cap_closure_tolerance(
                seam,
                curve,
                &surface,
                &parameter_curve,
                curve_reversed,
                maximum_residual,
            )
            .map_err(|_| GeometryError::InvalidPlanarFaceBoundary)?;
            vertices.push(BrepVertex::try_new(seam, closure_tolerance)?);
            edges.push(BrepEdge::try_new(
                [index, index],
                curve.clone(),
                closure_tolerance,
            )?);
            loops.push(single_edge_loop(
                index,
                index,
                loop_type,
                parameter_curve,
                curve_reversed,
                BrepTrimType::Boundary,
                [closure_tolerance, closure_tolerance],
            )?);
        }
        if loops.len() > 1 {
            let sampled = loops
                .iter()
                .map(|face_loop| sample_trim_loop(face_loop, LOOP_SAMPLES_PER_SPAN))
                .collect::<Result<Vec<_>, _>>()?;
            let loop_lengths = sampled.iter().map(Vec::len).collect::<Vec<_>>();
            let parameters = sampled.into_iter().flatten().collect::<Vec<_>>();
            if triangulate_trim_region(&parameters, &loop_lengths)?.is_none() {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
        }
        let faces = vec![BrepFace::try_new(surface, false, loops)?];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact capped right circular cylinder with one wall face
    /// and two polar disk faces. Periodic parameter seams are represented by
    /// shared radial/axial seam edges rather than duplicated boundary edges.
    pub fn try_cylinder(
        frame: Frame3,
        radius: Real,
        start_height: Real,
        end_height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let wall = NurbsSurface::try_cylinder(frame, radius, start_height, end_height)?;
        let height_domain = wall.domain_v();
        let low_height = *height_domain.start();
        let high_height = *height_domain.end();
        let low_frame = frame_at_height(frame, low_height, tolerance)?;
        let high_frame = frame_at_height(frame, high_height, tolerance)?;
        let u_domain = wall.domain_u();
        let low_seam = wall.evaluate(*u_domain.start(), low_height)?;
        let high_seam = wall.evaluate(*u_domain.start(), high_height)?;
        let vertices = vec![
            BrepVertex::try_new(low_seam, 0.0)?,
            BrepVertex::try_new(high_seam, 0.0)?,
            BrepVertex::try_new(low_frame.origin(), 0.0)?,
            BrepVertex::try_new(high_frame.origin(), 0.0)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], surface_u_control_curve(&wall, 0)?, 0.0)?,
            BrepEdge::try_new([1, 1], surface_u_control_curve(&wall, 1)?, 0.0)?,
            BrepEdge::try_new(
                [0, 1],
                LineSegment::try_new(low_seam, high_seam, tolerance)?.to_nurbs()?,
                0.0,
            )?,
            BrepEdge::try_new(
                [2, 0],
                LineSegment::try_new(low_frame.origin(), low_seam, tolerance)?.to_nurbs()?,
                0.0,
            )?,
            BrepEdge::try_new(
                [3, 1],
                LineSegment::try_new(high_frame.origin(), high_seam, tolerance)?.to_nurbs()?,
                0.0,
            )?,
        ];

        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 2, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 1, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 0], 2, true, BrepTrimType::Seam),
            ],
        )?;
        let low_disk = NurbsSurface::try_disk(low_frame, radius)?;
        let low_loop = rectangular_surface_loop(
            &low_disk,
            [
                RectangularTrimSpec::singular(2),
                RectangularTrimSpec::edge([2, 0], 3, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([0, 0], 0, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 2], 3, true, BrepTrimType::Seam),
            ],
        )?;
        let high_disk = NurbsSurface::try_disk(high_frame, radius)?;
        let high_loop = rectangular_surface_loop(
            &high_disk,
            [
                RectangularTrimSpec::singular(3),
                RectangularTrimSpec::edge([3, 1], 4, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 1, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 3], 4, true, BrepTrimType::Seam),
            ],
        )?;
        let faces = vec![
            BrepFace::try_new(wall, false, vec![wall_loop])?,
            BrepFace::try_new(low_disk, false, vec![low_loop])?,
            BrepFace::try_new(high_disk, true, vec![high_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs Rhino's exact closed tube extrusion as a four-face B-rep.
    ///
    /// The frame origin is the first cap center and frame Z points toward the
    /// second cap. Radii may be supplied in either order. The outer and inner
    /// circular wall domains occupy consecutive arc-length intervals, matching
    /// the joined-profile parameterization produced by Rhino's `Tube` command.
    pub fn try_tube(
        frame: Frame3,
        radii: [Real; 2],
        height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            radii.into_iter().chain(std::iter::once(height)),
            "tube dimensions",
        )?;
        if radii.into_iter().any(|radius| radius <= 0.0) || radii[0] == radii[1] || height <= 0.0 {
            return Err(GeometryError::Degenerate { context: "tube" });
        }
        let inner_radius = radii[0].min(radii[1]);
        let outer_radius = radii[0].max(radii[1]);
        let outer_domain_end = std::f64::consts::TAU * outer_radius;
        let inner_domain_end = std::f64::consts::TAU.mul_add(inner_radius, outer_domain_end);
        let cap_extent = 1.25 * outer_radius;
        require_finite(
            [outer_domain_end, inner_domain_end, cap_extent],
            "tube parameter domains",
        )?;

        let outer_wall =
            tube_wall_surface(frame, outer_radius, height, [0.0, outer_domain_end], false)?;
        let inner_wall = tube_wall_surface(
            frame,
            inner_radius,
            height,
            [outer_domain_end, inner_domain_end],
            true,
        )?;
        let end_frame = frame_at_height(frame, height, tolerance)?;
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let cap_bounds = [[-cap_extent, cap_extent], [-cap_extent, cap_extent]];
        let start_cap = planar_cap_surface(frame, zero, cap_bounds)?;
        let end_cap = planar_cap_surface(end_frame, zero, cap_bounds)?;

        let outer_u = outer_wall.domain_u();
        let inner_u = inner_wall.domain_u();
        let v = outer_wall.domain_v();
        let vertices = vec![
            BrepVertex::try_new(outer_wall.evaluate(*outer_u.start(), *v.start())?, 0.0)?,
            BrepVertex::try_new(outer_wall.evaluate(*outer_u.start(), *v.end())?, 0.0)?,
            BrepVertex::try_new(inner_wall.evaluate(*inner_u.start(), *v.start())?, 0.0)?,
            BrepVertex::try_new(inner_wall.evaluate(*inner_u.start(), *v.end())?, 0.0)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], surface_u_control_curve(&outer_wall, 0)?, 0.0)?,
            BrepEdge::try_new([0, 1], surface_v_control_curve(&outer_wall, 0)?, 0.0)?,
            BrepEdge::try_new([1, 1], surface_u_control_curve(&outer_wall, 1)?, 0.0)?,
            BrepEdge::try_new([2, 2], surface_u_control_curve(&inner_wall, 0)?, 0.0)?,
            BrepEdge::try_new([2, 3], surface_v_control_curve(&inner_wall, 0)?, 0.0)?,
            BrepEdge::try_new([3, 3], surface_u_control_curve(&inner_wall, 1)?, 0.0)?,
        ];
        let outer_wall_loop = rectangular_surface_loop(
            &outer_wall,
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 1, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 2, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 0], 1, true, BrepTrimType::Seam),
            ],
        )?;
        let inner_wall_loop = rectangular_surface_loop(
            &inner_wall,
            [
                RectangularTrimSpec::edge([2, 2], 3, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([2, 3], 4, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([3, 3], 5, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([3, 2], 4, true, BrepTrimType::Seam),
            ],
        )?;
        let outer_cap_curve = circular_parameter_curve(outer_radius)?;
        let inner_cap_curve = circular_parameter_curve(inner_radius)?.reversed()?;
        let start_cap_loops = vec![
            single_edge_loop(
                0,
                0,
                BrepLoopType::Outer,
                outer_cap_curve.clone(),
                false,
                BrepTrimType::Mated,
                [0.0, 0.0],
            )?,
            single_edge_loop(
                2,
                3,
                BrepLoopType::Inner,
                inner_cap_curve.clone(),
                false,
                BrepTrimType::Mated,
                [0.0, 0.0],
            )?,
        ];
        let end_cap_loops = vec![
            single_edge_loop(
                1,
                2,
                BrepLoopType::Outer,
                outer_cap_curve,
                false,
                BrepTrimType::Mated,
                [0.0, 0.0],
            )?,
            single_edge_loop(
                3,
                5,
                BrepLoopType::Inner,
                inner_cap_curve,
                false,
                BrepTrimType::Mated,
                [0.0, 0.0],
            )?,
        ];
        let faces = vec![
            BrepFace::try_new(outer_wall, false, vec![outer_wall_loop])?,
            BrepFace::try_new(inner_wall, false, vec![inner_wall_loop])?,
            BrepFace::try_new(start_cap, true, start_cap_loops)?,
            BrepFace::try_new(end_cap, false, end_cap_loops)?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs Rhino's exact capped right circular truncated cone.
    ///
    /// The supplied frame origin is the base center and frame Z points toward
    /// the end circle. The wall uses slant-length V parameters; outward-facing
    /// affine cap surfaces share the two circular rims, while one generatrix
    /// edge represents both uses of the periodic wall seam.
    pub fn try_truncated_cone(
        frame: Frame3,
        radii: [Real; 2],
        height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let wall = NurbsSurface::try_truncated_cone(frame, radii, height)?;
        let end_frame = frame_at_height(frame, height, tolerance)?;
        let base_cap_frame = Frame3::try_from_directions(
            frame.origin(),
            frame.x_axis().as_vector(),
            frame.y_axis().as_vector().scaled(-1.0)?,
            tolerance,
        )?;
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let base_cap = planar_cap_surface(
            base_cap_frame,
            zero,
            [[-radii[0], radii[0]], [-radii[0], radii[0]]],
        )?;
        let end_cap = planar_cap_surface(
            end_frame,
            zero,
            [[-radii[1], radii[1]], [-radii[1], radii[1]]],
        )?;

        let domain_u = wall.domain_u();
        let domain_v = wall.domain_v();
        let base_seam = wall.evaluate(*domain_u.start(), *domain_v.start())?;
        let end_seam = wall.evaluate(*domain_u.start(), *domain_v.end())?;
        let base_rim = surface_u_control_curve(&wall, 0)?;
        let end_rim = surface_u_control_curve(&wall, 1)?.reversed()?;
        let seam = surface_v_control_curve(&wall, 0)?;
        let vertices = vec![
            BrepVertex::try_new(base_seam, 0.0)?,
            BrepVertex::try_new(end_seam, 0.0)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], base_rim, 0.0)?,
            BrepEdge::try_new([0, 1], seam, 0.0)?,
            BrepEdge::try_new([1, 1], end_rim, 0.0)?,
        ];

        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 1, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 2, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 0], 1, true, BrepTrimType::Seam),
            ],
        )?;
        let base_loop = single_edge_loop(
            0,
            0,
            BrepLoopType::Outer,
            circular_parameter_curve(radii[0])?,
            true,
            BrepTrimType::Mated,
            [0.0, 0.0],
        )?;
        let end_loop = single_edge_loop(
            1,
            2,
            BrepLoopType::Outer,
            circular_parameter_curve(radii[1])?,
            true,
            BrepTrimType::Mated,
            [0.0, 0.0],
        )?;
        let faces = vec![
            BrepFace::try_new(wall, false, vec![wall_loop])?,
            BrepFace::try_new(base_cap, false, vec![base_loop])?,
            BrepFace::try_new(end_cap, false, vec![end_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact capped right circular cone from its base frame and
    /// signed apex height. The wall apex and both periodic parameter seams use
    /// singular/shared topology rather than duplicate boundary geometry.
    pub fn try_cone(
        base_frame: Frame3,
        radius: Real,
        height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let apex_frame = frame_at_height(base_frame, height, tolerance)?;
        let wall = NurbsSurface::try_cone(apex_frame, radius, -height)?;
        let u_domain = wall.domain_u();
        let base_parameter = -height;
        let base_seam = wall.evaluate(*u_domain.start(), base_parameter)?;
        let apex = apex_frame.origin();
        let base = base_frame.origin();
        let base_v_index = usize::from(height < 0.0);
        let vertices = vec![
            BrepVertex::try_new(base_seam, 0.0)?,
            BrepVertex::try_new(apex, 0.0)?,
            BrepVertex::try_new(base, 0.0)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], surface_u_control_curve(&wall, base_v_index)?, 0.0)?,
            BrepEdge::try_new(
                [0, 1],
                LineSegment::try_new(base_seam, apex, tolerance)?.to_nurbs()?,
                0.0,
            )?,
            BrepEdge::try_new(
                [2, 0],
                LineSegment::try_new(base, base_seam, tolerance)?.to_nurbs()?,
                0.0,
            )?,
        ];

        let wall_specs = if height > 0.0 {
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 1, false, BrepTrimType::Seam),
                RectangularTrimSpec::singular(1),
                RectangularTrimSpec::edge([1, 0], 1, true, BrepTrimType::Seam),
            ]
        } else {
            [
                RectangularTrimSpec::singular(1),
                RectangularTrimSpec::edge([1, 0], 1, true, BrepTrimType::Seam),
                RectangularTrimSpec::edge([0, 0], 0, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 1, false, BrepTrimType::Seam),
            ]
        };
        let wall_loop = rectangular_surface_loop(&wall, wall_specs)?;
        let cap = NurbsSurface::try_disk(base_frame, radius)?;
        let cap_loop = rectangular_surface_loop(
            &cap,
            [
                RectangularTrimSpec::singular(2),
                RectangularTrimSpec::edge([2, 0], 2, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([0, 0], 0, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 2], 2, true, BrepTrimType::Seam),
            ],
        )?;
        let faces = vec![
            BrepFace::try_new(wall, false, vec![wall_loop])?,
            BrepFace::try_new(cap, height < 0.0, vec![cap_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs Rhino's exact finite paraboloid B-rep.
    ///
    /// The frame origin is the vertex, frame Z is the opening direction, and
    /// frame X reaches the circular-rim seam. The wall has one singular apex
    /// trim, two uses of one meridian seam edge, and a reversed-domain rim
    /// edge. `solid` adds Rhino's affine one-edge planar cap without changing
    /// the wall topology.
    pub fn try_paraboloid(
        vertex_frame: Frame3,
        radius: Real,
        height: Real,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let wall = NurbsSurface::try_paraboloid(vertex_frame, radius, height)?;
        let domain_u = wall.domain_u();
        let domain_v = wall.domain_v();
        let vertex = vertex_frame.origin();
        let rim_seam = wall.evaluate(*domain_u.start(), *domain_v.end())?;
        let vertices = vec![
            BrepVertex::try_new(vertex, 0.0)?,
            BrepVertex::try_new(rim_seam, 0.0)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 1], surface_v_control_curve(&wall, 0)?, 0.0)?,
            BrepEdge::try_new(
                [1, 1],
                surface_u_control_curve(&wall, wall.control_point_count_v() - 1)?.reversed()?,
                0.0,
            )?,
        ];
        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::singular(0),
                RectangularTrimSpec::edge([0, 1], 0, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge(
                    [1, 1],
                    1,
                    false,
                    if solid {
                        BrepTrimType::Mated
                    } else {
                        BrepTrimType::Boundary
                    },
                ),
                RectangularTrimSpec::edge([1, 0], 0, true, BrepTrimType::Seam),
            ],
        )?;
        let mut faces = vec![BrepFace::try_new(wall, false, vec![wall_loop])?];
        if solid {
            let cap_frame = frame_at_height(vertex_frame, height, tolerance)?;
            let cap = planar_cap_surface(
                cap_frame,
                Vector3::try_new(0.0, 0.0, 0.0)?,
                [[-radius, radius], [-radius, radius]],
            )?;
            let cap_loop = single_edge_loop(
                1,
                1,
                BrepLoopType::Outer,
                circular_parameter_curve(radius)?,
                true,
                BrepTrimType::Mated,
                [0.0, 0.0],
            )?;
            faces.push(BrepFace::try_new(cap, false, vec![cap_loop])?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact capped straight extrusion of a closed planar curve.
    ///
    /// The source curve and its rational data are retained by both rim edges
    /// and the ruled wall. Each cap is an affine plane trimmed by an exact 2D
    /// projection of that same rational curve. The wall seam is shared twice,
    /// so the result has no duplicated topological boundary geometry.
    pub fn try_extruded_curve(
        curve: &NurbsCurve,
        start_offset: Vector3,
        end_offset: Vector3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !curve.is_closed()? || !curve.is_planar(tolerance)? {
            return Err(GeometryError::InvalidCappedExtrusionProfile);
        }

        let wall = NurbsSurface::try_extruded_curve(curve, start_offset, end_offset)?;
        let path = Vector3::try_new(
            end_offset.x() - start_offset.x(),
            end_offset.y() - start_offset.y(),
            end_offset.z() - start_offset.z(),
        )?;
        let projection = project_planar_curve(curve, tolerance)?;
        let normal_distance = path.dot(projection.frame.z_axis().as_vector())?;
        if normal_distance.abs() <= tolerance.absolute() {
            return Err(GeometryError::CoplanarCappedExtrusion);
        }

        let start_cap =
            planar_cap_surface(projection.frame, start_offset, projection.coordinate_bounds)?;
        let end_cap =
            planar_cap_surface(projection.frame, end_offset, projection.coordinate_bounds)?;
        let (cap_curve, cap_curve_reversed) = oriented_cap_curve(projection.curve)?;

        let u_domain = wall.domain_u();
        let v_domain = wall.domain_v();
        let start_seam = wall.evaluate(*u_domain.start(), *v_domain.start())?;
        let end_seam = wall.evaluate(*u_domain.start(), *v_domain.end())?;
        let start_profile = surface_u_control_curve(&wall, 0)?;
        let end_profile = surface_u_control_curve(&wall, 1)?;
        let closure_tolerance = extrusion_closure_tolerance(
            [start_seam, end_seam],
            [&start_profile, &end_profile],
            [&start_cap, &end_cap],
            &cap_curve,
            cap_curve_reversed,
            projection.maximum_residual,
        )?;
        let vertices = vec![
            BrepVertex::try_new(start_seam, closure_tolerance)?,
            BrepVertex::try_new(end_seam, closure_tolerance)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], start_profile, closure_tolerance)?,
            BrepEdge::try_new([1, 1], end_profile, closure_tolerance)?,
            BrepEdge::try_new(
                [0, 1],
                LineSegment::try_new(start_seam, end_seam, tolerance)?.to_nurbs()?,
                0.0,
            )?,
        ];

        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 2, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 1, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 0], 2, true, BrepTrimType::Seam),
            ],
        )?;
        let cap_trim_tolerance = [closure_tolerance, closure_tolerance];
        let start_loop = single_edge_loop(
            0,
            0,
            BrepLoopType::Outer,
            cap_curve.clone(),
            cap_curve_reversed,
            BrepTrimType::Mated,
            cap_trim_tolerance,
        )?;
        let end_loop = single_edge_loop(
            1,
            1,
            BrepLoopType::Outer,
            cap_curve,
            cap_curve_reversed,
            BrepTrimType::Mated,
            cap_trim_tolerance,
        )?;
        let path_opposes_surface = normal_distance < 0.0;
        let faces = vec![
            BrepFace::try_new(
                wall,
                path_opposes_surface ^ cap_curve_reversed,
                vec![wall_loop],
            )?,
            BrepFace::try_new(start_cap, !path_opposes_surface, vec![start_loop])?,
            BrepFace::try_new(end_cap, path_opposes_surface, vec![end_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact capped fixed-orientation sweep along an open NURBS path.
    ///
    /// The tensor-product wall retains the complete rational data of both
    /// curves. Exact translated profile curves form the two shared rims, and
    /// an exact translated copy of the path forms the twice-used wall seam.
    pub fn try_extruded_curve_along_curve(
        profile: &NurbsCurve,
        path: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !profile.is_closed()? || !profile.is_planar(tolerance)? {
            return Err(GeometryError::InvalidCappedExtrusionProfile);
        }
        if path.is_closed()? {
            return Err(GeometryError::InvalidCappedExtrusionPath);
        }

        let wall = NurbsSurface::try_extruded_curve_along_curve(profile, path)?;
        let path_domain = path.domain();
        let path_start = path.evaluate(*path_domain.start())?;
        let path_end = path.evaluate(*path_domain.end())?;
        let start_offset = Vector3::try_new(0.0, 0.0, 0.0)?;
        let end_offset = path_start.vector_to(path_end)?;
        let projection = project_planar_curve(profile, tolerance)?;
        let normal_distance = end_offset.dot(projection.frame.z_axis().as_vector())?;
        if normal_distance.abs() <= tolerance.absolute() {
            return Err(GeometryError::CoplanarCappedExtrusion);
        }

        let start_cap =
            planar_cap_surface(projection.frame, start_offset, projection.coordinate_bounds)?;
        let end_cap =
            planar_cap_surface(projection.frame, end_offset, projection.coordinate_bounds)?;
        let (cap_curve, cap_curve_reversed) = oriented_cap_curve(projection.curve)?;

        let profile_domain = profile.domain();
        let profile_start = profile.evaluate(*profile_domain.start())?;
        let start_seam = profile_start;
        let end_seam = profile_start.translated(end_offset)?;
        let start_profile = profile.clone();
        let end_profile = profile.transformed(AffineTransform3::from_translation(end_offset))?;
        let seam = path.transformed(AffineTransform3::from_translation(
            path_start.vector_to(profile_start)?,
        ))?;
        let closure_tolerance = extrusion_closure_tolerance(
            [start_seam, end_seam],
            [&start_profile, &end_profile],
            [&start_cap, &end_cap],
            &cap_curve,
            cap_curve_reversed,
            projection.maximum_residual,
        )?;
        let vertices = vec![
            BrepVertex::try_new(start_seam, closure_tolerance)?,
            BrepVertex::try_new(end_seam, closure_tolerance)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], start_profile, closure_tolerance)?,
            BrepEdge::try_new([1, 1], end_profile, closure_tolerance)?,
            BrepEdge::try_new([0, 1], seam, closure_tolerance)?,
        ];

        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::edge([0, 0], 0, false, BrepTrimType::Mated),
                RectangularTrimSpec::edge([0, 1], 2, false, BrepTrimType::Seam),
                RectangularTrimSpec::edge([1, 1], 1, true, BrepTrimType::Mated),
                RectangularTrimSpec::edge([1, 0], 2, true, BrepTrimType::Seam),
            ],
        )?;
        let cap_trim_tolerance = [closure_tolerance, closure_tolerance];
        let start_loop = single_edge_loop(
            0,
            0,
            BrepLoopType::Outer,
            cap_curve.clone(),
            cap_curve_reversed,
            BrepTrimType::Mated,
            cap_trim_tolerance,
        )?;
        let end_loop = single_edge_loop(
            1,
            1,
            BrepLoopType::Outer,
            cap_curve,
            cap_curve_reversed,
            BrepTrimType::Mated,
            cap_trim_tolerance,
        )?;
        let path_opposes_surface = normal_distance < 0.0;
        let faces = vec![
            BrepFace::try_new(
                wall,
                path_opposes_surface ^ cap_curve_reversed,
                vec![wall_loop],
            )?,
            BrepFace::try_new(start_cap, !path_opposes_surface, vec![start_loop])?,
            BrepFace::try_new(end_cap, path_opposes_surface, vec![end_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Constructs an exact capped extrusion from a closed planar curve to an apex.
    ///
    /// The ruled wall retains the source curve's rational data and represents
    /// its collapsed apex edge with a singular trim. One profile edge is shared
    /// with an affine planar cap, while one radial seam edge is used twice by
    /// the wall instead of duplicating boundary geometry.
    pub fn try_extruded_curve_to_point(
        curve: &NurbsCurve,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !curve.is_closed()? || !curve.is_planar(tolerance)? {
            return Err(GeometryError::InvalidCappedExtrusionProfile);
        }

        let wall = NurbsSurface::try_extruded_curve_to_point(curve, apex)?;
        let projection = project_planar_curve(curve, tolerance)?;
        let normal_distance = projection
            .frame
            .origin()
            .vector_to(apex)?
            .dot(projection.frame.z_axis().as_vector())?;
        if normal_distance.abs() <= tolerance.absolute() {
            return Err(GeometryError::CoplanarCappedExtrusion);
        }

        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        let cap = planar_cap_surface(projection.frame, zero, projection.coordinate_bounds)?;
        let (cap_curve, cap_curve_reversed) = oriented_cap_curve(projection.curve)?;

        let domain_u = wall.domain_u();
        let domain_v = wall.domain_v();
        let profile_seam = wall.evaluate(*domain_u.start(), *domain_v.start())?;
        let wall_apex = wall.evaluate(*domain_u.end(), *domain_v.start())?;
        let profile = surface_v_control_curve(&wall, 0)?;
        let seam = surface_u_control_curve(&wall, 0)?;
        let closure_tolerance = cap_closure_tolerance(
            profile_seam,
            &profile,
            &cap,
            &cap_curve,
            cap_curve_reversed,
            projection.maximum_residual,
        )?;
        let apex_tolerance = wall_apex.distance_to(apex)?;
        let vertices = vec![
            BrepVertex::try_new(profile_seam, closure_tolerance)?,
            BrepVertex::try_new(apex, apex_tolerance)?,
        ];
        let edges = vec![
            BrepEdge::try_new([0, 0], profile, closure_tolerance)?,
            BrepEdge::try_new([0, 1], seam, apex_tolerance)?,
        ];

        let wall_loop = rectangular_surface_loop(
            &wall,
            [
                RectangularTrimSpec::edge([0, 1], 1, false, BrepTrimType::Seam),
                RectangularTrimSpec::singular(1),
                RectangularTrimSpec::edge([1, 0], 1, true, BrepTrimType::Seam),
                RectangularTrimSpec::edge([0, 0], 0, true, BrepTrimType::Mated),
            ],
        )?;
        let cap_loop = single_edge_loop(
            0,
            0,
            BrepLoopType::Outer,
            cap_curve,
            cap_curve_reversed,
            BrepTrimType::Mated,
            [closure_tolerance, closure_tolerance],
        )?;
        let apex_is_above_profile = normal_distance > 0.0;
        let faces = vec![
            BrepFace::try_new(
                wall,
                cap_curve_reversed ^ apex_is_above_profile,
                vec![wall_loop],
            )?,
            BrepFace::try_new(cap, apex_is_above_profile, vec![cap_loop])?,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    #[inline]
    pub fn vertices(&self) -> &[BrepVertex] {
        &self.vertices
    }

    #[inline]
    pub fn edges(&self) -> &[BrepEdge] {
        &self.edges
    }

    #[inline]
    pub fn faces(&self) -> &[BrepFace] {
        &self.faces
    }

    /// Returns the selected face's exact non-seam boundary curves in connected
    /// components.
    ///
    /// Mated edges are included because they become naked when a single face
    /// is considered in isolation. Seam and singular trims are excluded. Each
    /// component starts from its lowest source edge index and is extended at
    /// either end, matching Rhino's `DupFaceBorder`/`JoinCurves` ordering;
    /// disconnected components are returned in reverse source-edge order.
    pub fn face_boundary_curve_components(
        &self,
        face_index: usize,
    ) -> Result<Vec<Vec<NurbsCurve>>, GeometryError> {
        let face = self
            .faces
            .get(face_index)
            .ok_or(GeometryError::BrepFaceIndexOutOfRange {
                face: face_index,
                face_count: self.faces.len(),
            })?;
        let mut boundary_edges = vec![false; self.edges.len()];
        for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
            if matches!(trim.trim_type, BrepTrimType::Boundary | BrepTrimType::Mated) {
                let edge = trim
                    .edge
                    .expect("validated non-singular B-rep trim must reference an edge");
                boundary_edges[edge] = true;
            }
        }
        let edge_indices = boundary_edges
            .iter()
            .enumerate()
            .filter_map(|(edge, selected)| selected.then_some(edge))
            .collect::<Vec<_>>();
        if edge_indices.is_empty() {
            return Ok(Vec::new());
        }

        let mut local_edges_at_vertex = BTreeMap::<usize, Vec<usize>>::new();
        for (local_edge, &edge_index) in edge_indices.iter().enumerate() {
            let vertices = self.edges[edge_index].vertices;
            local_edges_at_vertex
                .entry(vertices[0])
                .or_default()
                .push(local_edge);
            if vertices[1] != vertices[0] {
                local_edges_at_vertex
                    .entry(vertices[1])
                    .or_default()
                    .push(local_edge);
            }
        }

        let mut visited = vec![false; edge_indices.len()];
        let mut components = Vec::new();
        for root in 0..edge_indices.len() {
            if visited[root] {
                continue;
            }
            visited[root] = true;
            let mut pending = vec![root];
            let mut component = Vec::new();
            while let Some(local_edge) = pending.pop() {
                component.push(edge_indices[local_edge]);
                for vertex in self.edges[edge_indices[local_edge]].vertices {
                    for &neighbor in &local_edges_at_vertex[&vertex] {
                        if !visited[neighbor] {
                            visited[neighbor] = true;
                            pending.push(neighbor);
                        }
                    }
                }
            }
            component.sort_unstable();
            components.push(self.chain_boundary_component(&component)?);
        }
        components.reverse();
        Ok(components)
    }

    fn chain_boundary_component(
        &self,
        edge_indices: &[usize],
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        debug_assert!(!edge_indices.is_empty());
        let mut chain = VecDeque::with_capacity(edge_indices.len());
        chain.push_back((edge_indices[0], false));
        let mut remaining = edge_indices[1..].to_vec();
        while !remaining.is_empty() {
            let (first_edge, first_reversed) = chain[0];
            let (last_edge, last_reversed) = chain[chain.len() - 1];
            let first_vertices = oriented_edge_vertices(&self.edges[first_edge], first_reversed);
            let last_vertices = oriented_edge_vertices(&self.edges[last_edge], last_reversed);
            let Some((position, placement)) =
                remaining
                    .iter()
                    .enumerate()
                    .find_map(|(position, &candidate)| {
                        let vertices = self.edges[candidate].vertices;
                        if vertices[0] == last_vertices[1] {
                            Some((position, (false, false)))
                        } else if vertices[1] == last_vertices[1] {
                            Some((position, (false, true)))
                        } else if vertices[1] == first_vertices[0] {
                            Some((position, (true, false)))
                        } else if vertices[0] == first_vertices[0] {
                            Some((position, (true, true)))
                        } else {
                            None
                        }
                    })
            else {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "a face boundary edge component could not be chained",
                });
            };
            let edge = remaining.remove(position);
            let (prepend, reversed) = placement;
            if prepend {
                chain.push_front((edge, reversed));
            } else {
                chain.push_back((edge, reversed));
            }
        }
        if chain.len() > 1 {
            let (first_edge, first_reversed) = chain[0];
            let (last_edge, last_reversed) = chain[chain.len() - 1];
            let first = oriented_edge_vertices(&self.edges[first_edge], first_reversed)[0];
            let last = oriented_edge_vertices(&self.edges[last_edge], last_reversed)[1];
            if first == last {
                let root_position = chain
                    .iter()
                    .position(|(edge, _)| *edge == edge_indices[0])
                    .expect("the boundary chain must retain its root edge");
                chain.rotate_left((root_position + chain.len() - 1) % chain.len());
            }
        }
        chain
            .into_iter()
            .map(|(edge, reversed)| {
                if reversed {
                    self.edges[edge].curve.reversed()
                } else {
                    Ok(self.edges[edge].curve.clone())
                }
            })
            .collect()
    }

    /// Duplicates a non-empty, unique face subset as one validated B-rep.
    ///
    /// Faces retain the requested order while vertices and edges retain source
    /// order. Edges mated only to omitted faces become boundaries, seams remain
    /// seams, and vertex tolerances are recomputed like OpenNURBS
    /// `ON_Brep::DuplicateFaces`.
    pub fn duplicate_faces(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.copy_face_subset(face_indices, tolerance, true)
    }

    /// Copies a non-empty, unique face subset while preserving source vertex
    /// tolerances, matching OpenNURBS `ON_Brep::SubBrep` and the remainder left
    /// by Rhino's `ExtractSrf` command.
    pub fn sub_brep(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.copy_face_subset(face_indices, tolerance, false)
    }

    fn copy_face_subset(
        &self,
        face_indices: &[usize],
        tolerance: Tolerance,
        recompute_vertex_tolerances: bool,
    ) -> Result<Self, GeometryError> {
        if face_indices.is_empty() {
            return Err(GeometryError::EmptyBrepFaceSubset);
        }
        let mut selected_faces = vec![false; self.faces.len()];
        for &face in face_indices {
            let Some(selected) = selected_faces.get_mut(face) else {
                return Err(GeometryError::BrepFaceIndexOutOfRange {
                    face,
                    face_count: self.faces.len(),
                });
            };
            if std::mem::replace(selected, true) {
                return Err(GeometryError::DuplicateBrepFaceIndex { face });
            }
        }

        let mut used_vertices = vec![false; self.vertices.len()];
        let mut used_edges = vec![false; self.edges.len()];
        let mut total_edge_uses = vec![0_usize; self.edges.len()];
        let mut loop_edge_uses = Vec::with_capacity(face_indices.len());
        for &face_index in face_indices {
            let face = &self.faces[face_index];
            let mut face_loop_edge_uses = Vec::with_capacity(face.loops.len());
            for face_loop in &face.loops {
                let mut uses = BTreeMap::new();
                for trim in &face_loop.trims {
                    for vertex in trim.vertices {
                        used_vertices[vertex] = true;
                    }
                    if let Some(edge) = trim.edge {
                        used_edges[edge] = true;
                        total_edge_uses[edge] += 1;
                        *uses.entry(edge).or_insert(0_usize) += 1;
                        for vertex in self.edges[edge].vertices {
                            used_vertices[vertex] = true;
                        }
                    }
                }
                face_loop_edge_uses.push(uses);
            }
            loop_edge_uses.push(face_loop_edge_uses);
        }

        let mut vertex_map = vec![usize::MAX; self.vertices.len()];
        let mut vertices = Vec::with_capacity(used_vertices.iter().filter(|used| **used).count());
        for (source, vertex) in self.vertices.iter().copied().enumerate() {
            if used_vertices[source] {
                vertex_map[source] = vertices.len();
                vertices.push(vertex);
            }
        }
        let mut edge_map = vec![usize::MAX; self.edges.len()];
        let mut edges = Vec::with_capacity(used_edges.iter().filter(|used| **used).count());
        for (source, edge) in self.edges.iter().enumerate() {
            if used_edges[source] {
                edge_map[source] = edges.len();
                edges.push(BrepEdge::try_new(
                    edge.vertices.map(|vertex| vertex_map[vertex]),
                    edge.curve.clone(),
                    edge.tolerance,
                )?);
            }
        }

        let mut faces = Vec::with_capacity(face_indices.len());
        for (selection_index, &face_index) in face_indices.iter().enumerate() {
            let source_face = &self.faces[face_index];
            let loops = source_face
                .loops
                .iter()
                .enumerate()
                .map(|(loop_index, face_loop)| {
                    let trims = face_loop
                        .trims
                        .iter()
                        .map(|trim| {
                            let trim_type = match trim.edge {
                                None => BrepTrimType::Singular,
                                Some(edge) if total_edge_uses[edge] == 1 => BrepTrimType::Boundary,
                                Some(edge)
                                    if loop_edge_uses[selection_index][loop_index]
                                        .get(&edge)
                                        .is_some_and(|uses| *uses >= 2) =>
                                {
                                    BrepTrimType::Seam
                                }
                                Some(_) => BrepTrimType::Mated,
                            };
                            BrepTrim::try_new(
                                trim.vertices.map(|vertex| vertex_map[vertex]),
                                trim.edge.map(|edge| edge_map[edge]),
                                trim.reversed_3d,
                                trim.curve.clone(),
                                trim_type,
                                trim.iso,
                                trim.tolerance,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    BrepLoop::try_new(face_loop.loop_type, trims)
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            faces.push(BrepFace::try_new(
                source_face.surface.clone(),
                source_face.reversed,
                loops,
            )?);
        }
        if recompute_vertex_tolerances {
            recompute_duplicated_face_vertex_tolerances(&mut vertices, &edges, &faces)?;
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Duplicates every face as an independent, validated one-face B-rep.
    ///
    /// Source-order vertices and edges used by each face are compacted and
    /// remapped. Edges formerly mated to another face become boundaries,
    /// while multiple uses within the same loop remain seams. This matches
    /// the topology produced by Rhino's `BrepFace.DuplicateFace`/`Explode`
    /// path without approximating trims or underlying surfaces.
    pub fn explode_faces(&self, tolerance: Tolerance) -> Result<Vec<Self>, GeometryError> {
        (0..self.faces.len())
            .map(|face| self.duplicate_faces(&[face], tolerance))
            .collect()
    }

    /// Returns every topological edge once, followed by the exact trimmed
    /// interior isocurves selected by the OpenNURBS wire-density rules.
    pub fn wireframe_curves(
        &self,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> Result<Vec<NurbsCurve>, GeometryError> {
        // Validate the density before staging any output, including for a
        // hypothetical face whose interior contributes no curves.
        self.faces[0].surface().wire_parameters_u(wire_density)?;
        if self.edges.len() > crate::MAX_SURFACE_WIRES {
            return Err(GeometryError::TooManySurfaceWires);
        }
        let mut curves = Vec::new();
        curves
            .try_reserve_exact(self.edges.len())
            .map_err(|_| GeometryError::TooManySurfaceWires)?;
        curves.extend(self.edges.iter().map(|edge| edge.curve().clone()));

        for face in &self.faces {
            let parameters_v = face.surface().wire_parameters_v(wire_density)?;
            let interior_v_count = parameters_v.len().saturating_sub(2);
            for v in parameters_v.into_iter().skip(1).take(interior_v_count) {
                for curve in face.isocurve_u_segments(v, tolerance)? {
                    push_brep_wire(&mut curves, curve)?;
                }
            }
            let parameters_u = face.surface().wire_parameters_u(wire_density)?;
            let interior_u_count = parameters_u.len().saturating_sub(2);
            for u in parameters_u.into_iter().skip(1).take(interior_u_count) {
                for curve in face.isocurve_v_segments(u, tolerance)? {
                    push_brep_wire(&mut curves, curve)?;
                }
            }
        }
        Ok(curves)
    }

    /// Finds the selected model-space point's nearest underlying face
    /// parameters, rejecting closest points that fall outside that face's trim
    /// region. Ties retain face order.
    pub fn closest_face_parameters(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<(usize, Real, Real)>, GeometryError> {
        let mut best: Option<(Real, usize, Real, Real)> = None;
        for (face_index, face) in self.faces.iter().enumerate() {
            let (u, v) = face.surface.closest_parameters(target, tolerance)?;
            if !face.contains_parameters(u, v, tolerance)? {
                continue;
            }
            let distance = face.surface.evaluate(u, v)?.distance_to(target)?;
            if best.is_none_or(|candidate| distance < candidate.0) {
                best = Some((distance, face_index, u, v));
            }
        }
        Ok(best.map(|(_, face, u, v)| (face, u, v)))
    }

    /// Finds the model-space point's nearest parameters on any underlying face
    /// surface without testing the face's parameter-space trim region. Ties
    /// retain face order.
    pub fn closest_underlying_face_parameters(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(usize, Real, Real), GeometryError> {
        let mut best: Option<(Real, usize, Real, Real)> = None;
        for (face_index, face) in self.faces.iter().enumerate() {
            let (u, v) = face.surface.closest_parameters(target, tolerance)?;
            let distance = face.surface.evaluate(u, v)?.distance_to(target)?;
            if best.is_none_or(|candidate| distance < candidate.0) {
                best = Some((distance, face_index, u, v));
            }
        }
        best.map(|(_, face, u, v)| (face, u, v))
            .ok_or(GeometryError::Degenerate {
                context: "B-rep underlying face closest-point search",
            })
    }

    pub fn edge_use_count(&self, edge_index: usize) -> Option<usize> {
        (edge_index < self.edges.len()).then(|| {
            self.trim_uses()
                .into_iter()
                .filter(|trim_use| trim_use.trim.edge == Some(edge_index))
                .count()
        })
    }

    pub fn is_manifold(&self) -> bool {
        (0..self.edges.len()).all(|edge| self.edge_use_count(edge).is_some_and(|count| count <= 2))
    }

    pub fn is_closed(&self) -> bool {
        (0..self.edges.len()).all(|edge| self.edge_use_count(edge) == Some(2))
    }

    pub fn is_solid(&self) -> bool {
        if !self.is_manifold() || !self.is_closed() {
            return false;
        }
        let uses = self.trim_uses();
        (0..self.edges.len()).all(|edge_index| {
            let edge_uses = uses
                .iter()
                .filter(|trim_use| trim_use.trim.edge == Some(edge_index))
                .collect::<Vec<_>>();
            edge_uses.len() == 2
                && (edge_uses[0].trim.reversed_3d ^ self.faces[edge_uses[0].face].reversed)
                    != (edge_uses[1].trim.reversed_3d ^ self.faces[edge_uses[1].face].reversed)
        })
    }

    /// Computes total face area directly from the exact NURBS faces.
    ///
    /// Full natural surface domains are integrated independently over every
    /// nonempty knot-span rectangle. Rectangular isoparametric trims are
    /// clamped exactly before the same integration, while other planar trimmed
    /// faces use their exact oriented p-curve boundaries, including
    /// subtractive inner loops. The control geometry is recentered first so
    /// large translations do not degrade any calculation.
    pub fn area(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let full_domain_faces = self
            .faces
            .iter()
            .map(|face| face_covers_full_surface_domain(face, tolerance))
            .collect::<Result<Vec<_>, _>>()?;
        let bounds = self.bounds();
        let reference = bounds.center()?;
        let scale = bounds.min().distance_to(bounds.max())?;
        let centered_surfaces = self
            .faces
            .iter()
            .map(|face| centered_surface(&face.surface, reference))
            .collect::<Result<Vec<_>, _>>()?;
        let rectangular_surfaces = self
            .faces
            .iter()
            .zip(&full_domain_faces)
            .zip(&centered_surfaces)
            .map(|((face, full_domain), surface)| {
                if *full_domain {
                    Ok(None)
                } else {
                    rectangular_face_trim_bounds(face, tolerance)?
                        .map(|bounds| {
                            surface.try_trimmed(
                                bounds[0][0]..=bounds[0][1],
                                bounds[1][0]..=bounds[1][1],
                            )
                        })
                        .transpose()
                }
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let planar_faces = full_domain_faces
            .iter()
            .zip(&rectangular_surfaces)
            .zip(&centered_surfaces)
            .enumerate()
            .map(
                |(face_index, ((full_domain, rectangular_surface), surface))| {
                    if *full_domain || rectangular_surface.is_some() {
                        Ok(None)
                    } else {
                        planar_surface_plane(surface, tolerance)?.map_or_else(
                            || Err(GeometryError::UnsupportedBrepTrimArea { face: face_index }),
                            |plane| Ok(Some(plane)),
                        )
                    }
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let absolute_tolerance = match product_three(
            tolerance.absolute(),
            scale.max(tolerance.absolute()),
            1.0,
            "B-rep area tolerance",
        ) {
            Ok(value) => value,
            Err(GeometryError::NonFinite { .. }) => Real::MAX,
            Err(error) => return Err(error),
        };
        let integration_piece_count = self
            .faces
            .iter()
            .zip(&full_domain_faces)
            .zip(&rectangular_surfaces)
            .map(|((face, full_domain), rectangular_surface)| {
                let integration_surface = if *full_domain {
                    Some(&face.surface)
                } else {
                    rectangular_surface.as_ref()
                };
                if let Some(surface) = integration_surface {
                    surface
                        .spans_u()
                        .count()
                        .checked_mul(surface.spans_v().count())
                        .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
                } else {
                    Ok(1)
                }
            })
            .try_fold(0_usize, |total, count| {
                total
                    .checked_add(count?)
                    .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
            })?;
        let piece_tolerance =
            (absolute_tolerance / integration_piece_count as Real).max(Real::MIN_POSITIVE);
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (face_index, (face, surface)) in self.faces.iter().zip(&centered_surfaces).enumerate() {
            let integration_surface = if full_domain_faces[face_index] {
                Some(surface)
            } else {
                rectangular_surfaces[face_index].as_ref()
            };
            if let Some(integration_surface) = integration_surface {
                for (u_start, u_end) in integration_surface.spans_u() {
                    for (v_start, v_end) in integration_surface.spans_v() {
                        let contribution = integrate_area_patch(
                            integration_surface,
                            [u_start, u_end],
                            [v_start, v_end],
                            piece_tolerance,
                            tolerance.relative(),
                        )?;
                        neumaier_add(&mut sum, &mut correction, contribution);
                    }
                }
            } else {
                let doubled_area = integrate_planar_trimmed_face_doubled_area(
                    face,
                    surface,
                    planar_faces[face_index]
                        .expect("a non-full-domain area face was verified planar"),
                    piece_tolerance,
                    tolerance.relative(),
                )?;
                let contribution =
                    product_three(doubled_area.abs(), 0.5, 1.0, "planar B-rep face area")?;
                neumaier_add(&mut sum, &mut correction, contribution);
            }
        }
        let area = sum + correction;
        require_finite([area], "B-rep area")?;
        Ok(area)
    }

    /// Computes oriented volume directly from the exact NURBS faces.
    ///
    /// The divergence-theorem integral is evaluated independently over every
    /// nonempty knot-span rectangle. Subtracting the control-geometry bounds
    /// center before the scalar triple product makes the result insensitive to
    /// large translations. Planar trimmed faces use an oriented boundary-area
    /// integral; nonplanar general trims still require a constrained
    /// parameter-space integrator and are rejected explicitly.
    pub fn signed_volume(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        if !self.is_solid() {
            return Err(GeometryError::OpenBrepVolume);
        }
        let full_domain_faces = self
            .faces
            .iter()
            .map(|face| face_covers_full_surface_domain(face, tolerance))
            .collect::<Result<Vec<_>, _>>()?;

        let bounds = self.bounds();
        let reference = bounds.center()?;
        let scale = bounds.min().distance_to(bounds.max())?;
        let centered_surfaces = self
            .faces
            .iter()
            .map(|face| centered_surface(&face.surface, reference))
            .collect::<Result<Vec<_>, _>>()?;
        let planar_faces = full_domain_faces
            .iter()
            .zip(&centered_surfaces)
            .enumerate()
            .map(|(face_index, (full_domain, surface))| {
                if *full_domain {
                    Ok(None)
                } else {
                    planar_surface_plane(surface, tolerance)?.map_or_else(
                        || {
                            Err(GeometryError::UnsupportedBrepTrimMassProperties {
                                face: face_index,
                            })
                        },
                        |plane| Ok(Some(plane)),
                    )
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let absolute_tolerance = match product_three(
            tolerance.absolute(),
            scale.max(tolerance.absolute()),
            scale.max(tolerance.absolute()),
            "B-rep volume tolerance",
        ) {
            Ok(value) => value,
            // A tolerance larger than the representable volume range imposes
            // no useful absolute restriction, but the relative target remains
            // meaningful to the adaptive integrator.
            Err(GeometryError::NonFinite { .. }) => Real::MAX,
            Err(error) => return Err(error),
        };
        let area_tolerance = match product_three(
            tolerance.absolute(),
            scale.max(tolerance.absolute()),
            1.0,
            "B-rep area tolerance",
        ) {
            Ok(value) => value,
            Err(GeometryError::NonFinite { .. }) => Real::MAX,
            Err(error) => return Err(error),
        };
        let integration_piece_count = self
            .faces
            .iter()
            .zip(&full_domain_faces)
            .map(|(face, full_domain)| {
                if *full_domain {
                    face.surface
                        .spans_u()
                        .count()
                        .checked_mul(face.surface.spans_v().count())
                        .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
                } else {
                    Ok(1)
                }
            })
            .try_fold(0_usize, |total, count| {
                total
                    .checked_add(count?)
                    .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
            })?;
        let piece_tolerance =
            (absolute_tolerance / integration_piece_count as Real).max(Real::MIN_POSITIVE);
        let piece_area_tolerance =
            (area_tolerance / integration_piece_count as Real).max(Real::MIN_POSITIVE);
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (face_index, (face, surface)) in self.faces.iter().zip(&centered_surfaces).enumerate() {
            if full_domain_faces[face_index] {
                for (u_start, u_end) in surface.spans_u() {
                    for (v_start, v_end) in surface.spans_v() {
                        let contribution = integrate_volume_patch(
                            surface,
                            face.reversed,
                            [u_start, u_end],
                            [v_start, v_end],
                            piece_tolerance,
                            tolerance.relative(),
                        )?;
                        neumaier_add(&mut sum, &mut correction, contribution);
                    }
                }
            } else {
                let contribution = integrate_planar_trimmed_face_volume(
                    face,
                    surface,
                    planar_faces[face_index]
                        .expect("a non-full-domain volume face was verified planar"),
                    piece_area_tolerance,
                    tolerance.relative(),
                )?;
                neumaier_add(&mut sum, &mut correction, contribution);
            }
        }
        let volume = sum + correction;
        require_finite([volume], "B-rep signed volume")?;
        Ok(volume)
    }

    /// Conservative control-geometry bounds. Exact curved-edge bounds can be
    /// tighter, but this box always contains every positive-weight NURBS locus.
    pub fn bounds(&self) -> BoundingBox3 {
        let mut points = self
            .vertices
            .iter()
            .map(|vertex| vertex.point)
            .chain(self.edges.iter().flat_map(|edge| {
                edge.curve
                    .control_points()
                    .iter()
                    .map(|control| control.point())
            }))
            .collect::<Vec<_>>();
        for face in &self.faces {
            let trimmed = rectangular_face_trim_bounds(face, Tolerance::DEFAULT)
                .ok()
                .flatten()
                .and_then(|bounds| {
                    face.surface
                        .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])
                        .ok()
                });
            points.extend(
                trimmed
                    .as_ref()
                    .unwrap_or(&face.surface)
                    .control_points()
                    .iter()
                    .map(|control| control.point()),
            );
        }
        BoundingBox3::from_points(points).expect("a validated B-rep has finite control geometry")
    }

    /// Applies a nonsingular affine map while retaining the shared topology
    /// and exact face-local p-curves. An orientation-reversing map toggles all
    /// face reversals so the represented material side remains unchanged.
    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let orientation_reversing = transform.orientation_reversing()?;
        let scale = transform.maximum_linear_scale()?;
        let vertices = self
            .vertices
            .iter()
            .map(|vertex| {
                BrepVertex::try_new(
                    transform.transform_point(vertex.point)?,
                    scaled_tolerance(vertex.tolerance, scale)?,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let edges = self
            .edges
            .iter()
            .map(|edge| {
                BrepEdge::try_new(
                    edge.vertices,
                    edge.curve.transformed(transform)?,
                    scaled_tolerance(edge.tolerance, scale)?,
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let faces = self
            .faces
            .iter()
            .map(|face| {
                BrepFace::try_new(
                    face.surface.transformed(transform)?,
                    face.reversed ^ orientation_reversing,
                    face.loops.clone(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// Tessellates full rectangular and generally trimmed faces.
    ///
    /// Trim boundaries are sampled per exact p-curve knot span and
    /// constrained-triangulated in parameter space while preserving every
    /// outer and inner boundary sample for watertight stitching. Nonplanar
    /// faces also receive the underlying surface's knot-span grid samples so
    /// their interior approximation tracks the requested density.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        self.tessellate_impl(samples_per_span, false, false, tolerance)
    }

    /// Creates one editable triangle/quad mesh for this B-rep.
    ///
    /// Full rectangular surface cells remain quadrilaterals and trimmed
    /// regions remain constrained triangles. With smooth seams, boundary
    /// samples are snapped to shared exact edges and closed solids are
    /// required to remain watertight. Jagged seams intentionally disable
    /// shared-edge snapping and permit naked edges between faces.
    pub fn polygon_mesh(
        &self,
        density: Real,
        simple_planes: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let samples_per_span = self
            .faces
            .iter()
            .map(|face| {
                face.surface
                    .polygon_mesh_samples_per_span(density, simple_planes, tolerance)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .expect("a validated B-rep has at least one face");
        self.tessellate_impl(samples_per_span, true, jagged_seams, tolerance)
    }

    fn tessellate_impl(
        &self,
        samples_per_span: usize,
        preserve_quads: bool,
        jagged_seams: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        if samples_per_span == 0 {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            let mesh = if face_covers_full_surface_domain(face, tolerance)? {
                let surface_mesh = if preserve_quads {
                    face.surface
                        .tessellate_grid(samples_per_span, true, tolerance)?
                } else {
                    face.surface.tessellate(samples_per_span, tolerance)?
                };
                let mut face_vertices = surface_mesh.vertices().to_vec();
                if !jagged_seams {
                    self.snap_face_boundary_vertices(
                        face,
                        &mut face_vertices,
                        samples_per_span,
                        tolerance,
                    )?;
                }
                TriangleMesh::try_new_faces(
                    face_vertices,
                    surface_mesh.faces().to_vec(),
                    tolerance,
                )?
            } else if let Some(bounds) = rectangular_face_trim_bounds(face, tolerance)? {
                let surface = face
                    .surface
                    .try_trimmed(bounds[0][0]..=bounds[0][1], bounds[1][0]..=bounds[1][1])?;
                if preserve_quads {
                    surface.tessellate_grid(samples_per_span, true, tolerance)?
                } else {
                    surface.tessellate(samples_per_span, tolerance)?
                }
            } else if planar_surface_plane(&face.surface, tolerance)?.is_some() {
                self.tessellate_planar_trimmed_face(face_index, face, samples_per_span, tolerance)?
            } else {
                self.tessellate_nonplanar_trimmed_face(
                    face_index,
                    face,
                    samples_per_span,
                    tolerance,
                )?
            };
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            let combined_vertex_count = vertices
                .len()
                .checked_add(mesh.vertices().len())
                .ok_or(GeometryError::TooManyMeshVertices)?;
            if combined_vertex_count > u32::MAX as usize {
                return Err(GeometryError::TooManyMeshVertices);
            }
            vertices.extend(mesh.vertices());
            for source_face in mesh.faces() {
                let mut mapped = source_face.remapped(|vertex| {
                    vertex
                        .checked_add(offset)
                        .expect("the combined mesh vertex range was checked")
                });
                if face.reversed {
                    mapped = mapped.reversed();
                }
                faces.push(mapped);
            }
        }
        let mesh = TriangleMesh::try_new_faces(vertices, faces, tolerance)?;
        let topology = mesh.topology();
        if !jagged_seams
            && ((self.is_closed() && !topology.is_closed())
                || (self.is_solid() && !topology.is_solid()))
        {
            return Err(GeometryError::UnstitchedBrepTessellation {
                boundary_edges: topology.boundary_edge_count(),
                orientation_conflicts: topology.orientation_conflict_edge_count(),
            });
        }
        Ok(mesh)
    }

    fn tessellate_planar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let mut sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| sample_trim_loop(face_loop, samples_per_span))
            .collect::<Result<Vec<_>, _>>()?;
        let (parameters, boundary_vertex_count, triangles) = if sampled_loops.len() == 1 {
            let mut parameters = sampled_loops.pop().expect("one sampled trim loop exists");
            let boundary_vertex_count = parameters.len();
            let triangles = if let Some(triangles) =
                triangulate_simple_trim_polygon(&mut parameters)?
            {
                triangles
            } else {
                triangulate_trim_region(&parameters, &[boundary_vertex_count])?
                    .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?
            };
            (parameters, boundary_vertex_count, triangles)
        } else {
            let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
            let parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
            let boundary_vertex_count = parameters.len();
            let triangles = triangulate_trim_region(&parameters, &loop_lengths)?
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?;
            (parameters, boundary_vertex_count, triangles)
        };
        let mut face_vertices = parameters
            .iter()
            .map(|parameter| face.surface.evaluate(parameter.x(), parameter.y()))
            .collect::<Result<Vec<_>, _>>()?;
        let candidates = self.trim_boundary_snap_points(face, samples_per_span, tolerance)?;
        snap_points_to_candidates(
            &mut face_vertices[..boundary_vertex_count],
            candidates,
            tolerance,
        );
        TriangleMesh::try_new(face_vertices, triangles, tolerance)
    }

    fn tessellate_nonplanar_trimmed_face(
        &self,
        face_index: usize,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let sampled_loops = face
            .loops
            .iter()
            .map(|face_loop| sample_trim_loop(face_loop, samples_per_span))
            .collect::<Result<Vec<_>, _>>()?;
        let loop_lengths = sampled_loops.iter().map(Vec::len).collect::<Vec<_>>();
        let mut parameters = sampled_loops.into_iter().flatten().collect::<Vec<_>>();
        let boundary_vertex_count = parameters.len();
        append_trimmed_surface_grid_parameters(
            &mut parameters,
            &loop_lengths,
            &face.surface,
            samples_per_span,
        )?;
        let triangles = triangulate_trim_region(&parameters, &loop_lengths)?
            .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?;
        let mut face_vertices = parameters
            .iter()
            .map(|parameter| face.surface.evaluate(parameter.x(), parameter.y()))
            .collect::<Result<Vec<_>, _>>()?;
        let candidates = self.trim_boundary_snap_points(face, samples_per_span, tolerance)?;
        snap_points_to_candidates(
            &mut face_vertices[..boundary_vertex_count],
            candidates,
            tolerance,
        );
        TriangleMesh::try_new(face_vertices, triangles, tolerance)
    }

    fn snap_face_boundary_vertices(
        &self,
        face: &BrepFace,
        vertices: &mut [Point3],
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        let candidates = self.face_boundary_snap_points(face, samples_per_span, tolerance)?;
        let span_count_u = face.surface.spans_u().count();
        let span_count_v = face.surface.spans_v().count();
        let side = samples_per_span
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let vertices_per_patch = side
            .checked_mul(side)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let expected_vertex_count = span_count_u
            .checked_mul(span_count_v)
            .and_then(|count| count.checked_mul(vertices_per_patch))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertices.len() != expected_vertex_count {
            return invalid("surface tessellation layout changed while stitching B-rep edges");
        }

        for v_span in 0..span_count_v {
            for u_span in 0..span_count_u {
                let patch = v_span * span_count_u + u_span;
                let patch_offset = patch * vertices_per_patch;
                for v_sample in 0..=samples_per_span {
                    for u_sample in 0..=samples_per_span {
                        let vertex = &mut vertices[patch_offset + v_sample * side + u_sample];
                        if v_span == 0 && v_sample == 0 {
                            snap_point_to_candidates(vertex, &candidates[0], tolerance);
                        }
                        if u_span + 1 == span_count_u && u_sample == samples_per_span {
                            snap_point_to_candidates(vertex, &candidates[1], tolerance);
                        }
                        if v_span + 1 == span_count_v && v_sample == samples_per_span {
                            snap_point_to_candidates(vertex, &candidates[2], tolerance);
                        }
                        if u_span == 0 && u_sample == 0 {
                            snap_point_to_candidates(vertex, &candidates[3], tolerance);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn face_boundary_snap_points(
        &self,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<[Vec<BoundarySnapPoint>; 4], GeometryError> {
        let mut candidates: [Vec<BoundarySnapPoint>; 4] = std::array::from_fn(|_| Vec::new());
        for trim in &face.loops[0].trims {
            let side = match trim.iso {
                SurfaceIso::South => 0,
                SurfaceIso::East => 1,
                SurfaceIso::North => 2,
                SurfaceIso::West => 3,
                SurfaceIso::NotIso
                | SurfaceIso::InteriorUConstant
                | SurfaceIso::InteriorVConstant => {
                    return invalid("a full-domain B-rep face has a non-boundary side");
                }
            };
            candidates[side].extend(self.trim_snap_points(trim, samples_per_span, tolerance)?);
        }
        Ok(candidates)
    }

    fn trim_boundary_snap_points(
        &self,
        face: &BrepFace,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<Vec<BoundarySnapPoint>, GeometryError> {
        let mut candidates = Vec::new();
        for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
            candidates.extend(self.trim_snap_points(trim, samples_per_span, tolerance)?);
        }
        Ok(candidates)
    }

    fn trim_snap_points(
        &self,
        trim: &BrepTrim,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<Vec<BoundarySnapPoint>, GeometryError> {
        let Some(edge_index) = trim.edge else {
            let vertex = self.vertices[trim.vertices[0]];
            return Ok(vec![BoundarySnapPoint {
                point: vertex.point,
                tolerance: tolerance.absolute().max(vertex.tolerance),
            }]);
        };
        let edge = &self.edges[edge_index];
        let spans = edge.curve.spans().collect::<Vec<_>>();
        let domain = edge.curve.domain();
        let last_span = spans
            .len()
            .checked_sub(1)
            .ok_or(GeometryError::InvalidBrepTopology {
                context: "a B-rep edge curve has no nonempty span",
            })?;
        let capacity = spans
            .len()
            .checked_mul(
                samples_per_span
                    .checked_add(1)
                    .ok_or(GeometryError::TooManyMeshVertices)?,
            )
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if capacity > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let mut candidates = Vec::with_capacity(capacity);
        for (span_index, (start, end)) in spans.into_iter().enumerate() {
            for sample in 0..=samples_per_span {
                let is_start = span_index == 0 && sample == 0;
                let is_end = span_index == last_span && sample == samples_per_span;
                let (point, component_tolerance) = if is_start {
                    let vertex = self.vertices[edge.vertices[0]];
                    (vertex.point, edge.tolerance.max(vertex.tolerance))
                } else if is_end {
                    let vertex = self.vertices[edge.vertices[1]];
                    (vertex.point, edge.tolerance.max(vertex.tolerance))
                } else {
                    let parameter =
                        brep_span_parameter(start, end, sample, samples_per_span, *domain.end());
                    (edge.curve.evaluate(parameter)?, edge.tolerance)
                };
                candidates.push(BoundarySnapPoint {
                    point,
                    tolerance: tolerance.absolute().max(component_tolerance),
                });
            }
        }
        Ok(candidates)
    }

    fn validate(&self, tolerance: Tolerance) -> Result<(), GeometryError> {
        for edge in &self.edges {
            if edge
                .vertices
                .iter()
                .any(|vertex| *vertex >= self.vertices.len())
            {
                return invalid("an edge references a missing vertex");
            }
            let domain = edge.curve.domain();
            let endpoints = [
                edge.curve.evaluate(*domain.start())?,
                edge.curve.evaluate(*domain.end())?,
            ];
            for (end, endpoint) in endpoints.into_iter().enumerate() {
                let vertex = self.vertices[edge.vertices[end]];
                let allowed = tolerance
                    .absolute()
                    .max(vertex.tolerance)
                    .max(edge.tolerance);
                if endpoint.distance_to(vertex.point)? > allowed {
                    return invalid("an edge-curve endpoint misses its vertex");
                }
            }
        }

        for face in &self.faces {
            for face_loop in &face.loops {
                self.validate_loop(face, face_loop, tolerance)?;
            }
        }

        let uses = self.trim_uses();
        for edge_index in 0..self.edges.len() {
            let edge_uses = uses
                .iter()
                .filter(|trim_use| trim_use.trim.edge == Some(edge_index))
                .collect::<Vec<_>>();
            if edge_uses.is_empty() {
                return invalid("every B-rep edge must be used by a trim");
            }
            for trim_use in &edge_uses {
                let same_loop_uses = edge_uses
                    .iter()
                    .filter(|other| {
                        trim_use.face == other.face && trim_use.face_loop == other.face_loop
                    })
                    .count();
                let expected = if edge_uses.len() == 1 {
                    BrepTrimType::Boundary
                } else if same_loop_uses >= 2 {
                    BrepTrimType::Seam
                } else {
                    BrepTrimType::Mated
                };
                if trim_use.trim.trim_type != expected {
                    return invalid("a trim type disagrees with its edge-use topology");
                }
            }
        }
        Ok(())
    }

    fn validate_loop(
        &self,
        face: &BrepFace,
        face_loop: &BrepLoop,
        tolerance: Tolerance,
    ) -> Result<(), GeometryError> {
        for (index, trim) in face_loop.trims.iter().enumerate() {
            if trim
                .vertices
                .iter()
                .any(|vertex| *vertex >= self.vertices.len())
            {
                return invalid("a trim references a missing vertex");
            }
            if let Some(edge_index) = trim.edge {
                let Some(edge) = self.edges.get(edge_index) else {
                    return invalid("a trim references a missing edge");
                };
                let expected = if trim.reversed_3d {
                    [edge.vertices[1], edge.vertices[0]]
                } else {
                    edge.vertices
                };
                if trim.vertices != expected {
                    return invalid("a trim's vertex direction disagrees with its 3D edge");
                }
            }

            let next = &face_loop.trims[(index + 1) % face_loop.trims.len()];
            if trim.vertices[1] != next.vertices[0] {
                return invalid("a trim loop is not topologically closed");
            }
            let end = trim.curve.end_point()?;
            let next_start = next.curve.start_point()?;
            let parameter_tolerance = [
                tolerance
                    .absolute()
                    .max(trim.tolerance[0])
                    .max(next.tolerance[0]),
                tolerance
                    .absolute()
                    .max(trim.tolerance[1])
                    .max(next.tolerance[1]),
            ];
            if !parameter_points_near(end, next_start, parameter_tolerance) {
                return invalid("adjacent p-curves do not meet in parameter space");
            }

            let parameter_ends = [trim.curve.start_point()?, end];
            for (end_index, parameter) in parameter_ends.into_iter().enumerate() {
                let surface_point = face.surface.evaluate(parameter.x(), parameter.y())?;
                let vertex = self.vertices[trim.vertices[end_index]];
                let allowed = tolerance.absolute().max(vertex.tolerance);
                if surface_point.distance_to(vertex.point)? > allowed {
                    return invalid("a p-curve endpoint misses its model-space vertex");
                }
            }
            validate_iso(face, trim, parameter_tolerance)?;
        }

        let signed_area = sampled_loop_signed_area(face_loop)?;
        let orientation_is_valid = match face_loop.loop_type {
            BrepLoopType::Outer => signed_area > 0.0,
            BrepLoopType::Inner => signed_area < 0.0,
        };
        if !orientation_is_valid {
            return invalid("outer p-loops must be counterclockwise and inner p-loops clockwise");
        }
        Ok(())
    }

    fn trim_uses(&self) -> Vec<TrimUse<'_>> {
        let mut uses = Vec::new();
        for (face, face_record) in self.faces.iter().enumerate() {
            for (face_loop, loop_record) in face_record.loops.iter().enumerate() {
                for trim in &loop_record.trims {
                    uses.push(TrimUse {
                        face,
                        face_loop,
                        trim,
                    });
                }
            }
        }
        uses
    }
}

fn reorder_cutting_split_rectangle(
    brep: Brep,
    vertex_order: [usize; 4],
    edge_order: [usize; 4],
    trim_order: [usize; 4],
    reversed_edge: Option<usize>,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    if brep.vertices.len() != 4
        || brep.edges.len() != 4
        || brep.faces.len() != 1
        || brep.faces[0].loops.len() != 1
        || brep.faces[0].loops[0].trims.len() != 4
    {
        return Ok(brep);
    }

    let mut vertex_map = [usize::MAX; 4];
    let vertices = vertex_order
        .into_iter()
        .enumerate()
        .map(|(new_index, old_index)| {
            vertex_map[old_index] = new_index;
            brep.vertices[old_index]
        })
        .collect::<Vec<_>>();

    let mut edge_map = [usize::MAX; 4];
    let edges = edge_order
        .into_iter()
        .enumerate()
        .map(|(new_index, old_index)| {
            edge_map[old_index] = new_index;
            let edge = &brep.edges[old_index];
            let mut edge_vertices = edge.vertices.map(|vertex| vertex_map[vertex]);
            let mut curve = edge.curve.clone();
            if reversed_edge == Some(old_index) {
                edge_vertices.reverse();
                curve = curve.reversed()?;
            }
            BrepEdge::try_new(edge_vertices, curve, edge.tolerance)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let old_face = &brep.faces[0];
    let old_loop = &old_face.loops[0];
    let trims = trim_order
        .into_iter()
        .map(|old_index| {
            let trim = &old_loop.trims[old_index];
            BrepTrim::try_new(
                trim.vertices.map(|vertex| vertex_map[vertex]),
                trim.edge.map(|edge| edge_map[edge]),
                trim.reversed_3d ^ trim.edge.is_some_and(|edge| reversed_edge == Some(edge)),
                trim.curve.clone(),
                trim.trim_type,
                trim.iso,
                trim.tolerance,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let face = BrepFace::try_new(
        old_face.surface.clone(),
        old_face.reversed,
        vec![BrepLoop::try_new(old_loop.loop_type, trims)?],
    )?;
    Brep::try_new(vertices, edges, vec![face], tolerance)
}

fn rectangular_surface_boundary_iso(
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
) -> [SurfaceIso; 4] {
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    [
        if bounds[1][0] == *domain_v.start() {
            SurfaceIso::South
        } else {
            SurfaceIso::InteriorVConstant
        },
        if bounds[0][1] == *domain_u.end() {
            SurfaceIso::East
        } else {
            SurfaceIso::InteriorUConstant
        },
        if bounds[1][1] == *domain_v.end() {
            SurfaceIso::North
        } else {
            SurfaceIso::InteriorVConstant
        },
        if bounds[0][0] == *domain_u.start() {
            SurfaceIso::West
        } else {
            SurfaceIso::InteriorUConstant
        },
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RectangularBoundarySide {
    South,
    East,
    North,
    West,
}

impl RectangularBoundarySide {
    const fn index(self) -> usize {
        match self {
            Self::South => 0,
            Self::East => 1,
            Self::North => 2,
            Self::West => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RectangularCornerCutKind {
    SouthWestToEast,
    SouthWestToNorth,
    SouthEastToNorth,
    SouthEastToWest,
    NorthEastToWest,
    NorthEastToSouth,
    NorthWestToSouth,
    NorthWestToEast,
    SouthWestToNorthEast,
    SouthEastToNorthWest,
}

#[derive(Clone, Copy)]
enum RectangularCornerCutDestination {
    Side(RectangularBoundarySide),
    Corner(RectangularSurfaceCorner),
}

#[derive(Clone, Copy)]
enum CornerCutVertex {
    SouthWest,
    SouthEast,
    NorthEast,
    NorthWest,
    Destination,
}

#[derive(Clone, Copy)]
enum CornerCutEdgeKind {
    Boundary(RectangularBoundarySide),
    Cut,
}

#[derive(Clone, Copy)]
struct CornerCutEdgeSpec {
    vertices: [usize; 2],
    kind: CornerCutEdgeKind,
}

struct CornerCutFaceSpec {
    vertices: Vec<CornerCutVertex>,
    edges: Vec<CornerCutEdgeSpec>,
    loop_edges: Vec<(usize, bool)>,
}

#[derive(Clone, Copy)]
struct SurfaceSplitCurveRef<'a> {
    spatial: &'a NurbsCurve,
    parameter: &'a NurbsCurve2,
}

fn classify_rectangular_corner_cut(
    corner: RectangularSurfaceCorner,
    destination: Point2,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> Result<(RectangularCornerCutKind, Point2), GeometryError> {
    let epsilon = [
        trim_parameter_epsilon(bounds[0], tolerance),
        trim_parameter_epsilon(bounds[1], tolerance),
    ];
    let near = |left: Real, right: Real, epsilon: Real| (left - right).abs() <= epsilon;
    let west = near(destination.x(), bounds[0][0], epsilon[0]);
    let east = near(destination.x(), bounds[0][1], epsilon[0]);
    let south = near(destination.y(), bounds[1][0], epsilon[1]);
    let north = near(destination.y(), bounds[1][1], epsilon[1]);
    let interior_u =
        destination.x() > bounds[0][0] + epsilon[0] && destination.x() < bounds[0][1] - epsilon[0];
    let interior_v =
        destination.y() > bounds[1][0] + epsilon[1] && destination.y() < bounds[1][1] - epsilon[1];
    let (destination_kind, snapped) = if west && south {
        (
            RectangularCornerCutDestination::Corner(RectangularSurfaceCorner::SouthWest),
            Point2::try_new(bounds[0][0], bounds[1][0])?,
        )
    } else if east && south {
        (
            RectangularCornerCutDestination::Corner(RectangularSurfaceCorner::SouthEast),
            Point2::try_new(bounds[0][1], bounds[1][0])?,
        )
    } else if east && north {
        (
            RectangularCornerCutDestination::Corner(RectangularSurfaceCorner::NorthEast),
            Point2::try_new(bounds[0][1], bounds[1][1])?,
        )
    } else if west && north {
        (
            RectangularCornerCutDestination::Corner(RectangularSurfaceCorner::NorthWest),
            Point2::try_new(bounds[0][0], bounds[1][1])?,
        )
    } else if south && interior_u {
        (
            RectangularCornerCutDestination::Side(RectangularBoundarySide::South),
            Point2::try_new(
                destination.x().clamp(bounds[0][0], bounds[0][1]),
                bounds[1][0],
            )?,
        )
    } else if east && interior_v {
        (
            RectangularCornerCutDestination::Side(RectangularBoundarySide::East),
            Point2::try_new(
                bounds[0][1],
                destination.y().clamp(bounds[1][0], bounds[1][1]),
            )?,
        )
    } else if north && interior_u {
        (
            RectangularCornerCutDestination::Side(RectangularBoundarySide::North),
            Point2::try_new(
                destination.x().clamp(bounds[0][0], bounds[0][1]),
                bounds[1][1],
            )?,
        )
    } else if west && interior_v {
        (
            RectangularCornerCutDestination::Side(RectangularBoundarySide::West),
            Point2::try_new(
                bounds[0][0],
                destination.y().clamp(bounds[1][0], bounds[1][1]),
            )?,
        )
    } else {
        return Err(GeometryError::InvalidBrepTopology {
            context: "a corner surface split must end on another trim boundary",
        });
    };

    use RectangularBoundarySide::{East, North, South, West};
    use RectangularCornerCutDestination::{Corner, Side};
    use RectangularSurfaceCorner::{NorthEast, NorthWest, SouthEast, SouthWest};
    let kind = match (corner, destination_kind) {
        (SouthWest, Side(East)) => RectangularCornerCutKind::SouthWestToEast,
        (SouthWest, Side(North)) => RectangularCornerCutKind::SouthWestToNorth,
        (SouthEast, Side(North)) => RectangularCornerCutKind::SouthEastToNorth,
        (SouthEast, Side(West)) => RectangularCornerCutKind::SouthEastToWest,
        (NorthEast, Side(West)) => RectangularCornerCutKind::NorthEastToWest,
        (NorthEast, Side(South)) => RectangularCornerCutKind::NorthEastToSouth,
        (NorthWest, Side(South)) => RectangularCornerCutKind::NorthWestToSouth,
        (NorthWest, Side(East)) => RectangularCornerCutKind::NorthWestToEast,
        (SouthWest, Corner(NorthEast)) | (NorthEast, Corner(SouthWest)) => {
            RectangularCornerCutKind::SouthWestToNorthEast
        }
        (SouthEast, Corner(NorthWest)) | (NorthWest, Corner(SouthEast)) => {
            RectangularCornerCutKind::SouthEastToNorthWest
        }
        _ => {
            return Err(GeometryError::InvalidBrepTopology {
                context: "a corner surface split must reach a nonincident side or opposite corner",
            });
        }
    };
    Ok((kind, snapped))
}

fn rectangular_surface_corner_parameter(
    corner: RectangularSurfaceCorner,
    bounds: [[Real; 2]; 2],
) -> Result<Point2, GeometryError> {
    match corner {
        RectangularSurfaceCorner::SouthWest => Point2::try_new(bounds[0][0], bounds[1][0]),
        RectangularSurfaceCorner::SouthEast => Point2::try_new(bounds[0][1], bounds[1][0]),
        RectangularSurfaceCorner::NorthEast => Point2::try_new(bounds[0][1], bounds[1][1]),
        RectangularSurfaceCorner::NorthWest => Point2::try_new(bounds[0][0], bounds[1][1]),
    }
}

fn rectangular_corner_cut_parameters(
    kind: RectangularCornerCutKind,
    destination: Point2,
    bounds: [[Real; 2]; 2],
) -> Result<[Point2; 2], GeometryError> {
    use RectangularCornerCutKind::{
        NorthEastToSouth, NorthEastToWest, NorthWestToEast, NorthWestToSouth, SouthEastToNorth,
        SouthEastToNorthWest, SouthEastToWest, SouthWestToEast, SouthWestToNorth,
        SouthWestToNorthEast,
    };
    let start = match kind {
        SouthWestToEast | SouthWestToNorth | SouthWestToNorthEast => {
            RectangularSurfaceCorner::SouthWest
        }
        SouthEastToNorth | SouthEastToWest | SouthEastToNorthWest => {
            RectangularSurfaceCorner::SouthEast
        }
        NorthEastToWest | NorthEastToSouth => RectangularSurfaceCorner::NorthEast,
        NorthWestToSouth | NorthWestToEast => RectangularSurfaceCorner::NorthWest,
    };
    let end = match kind {
        SouthWestToNorthEast => {
            rectangular_surface_corner_parameter(RectangularSurfaceCorner::NorthEast, bounds)?
        }
        SouthEastToNorthWest => {
            rectangular_surface_corner_parameter(RectangularSurfaceCorner::NorthWest, bounds)?
        }
        _ => destination,
    };
    Ok([rectangular_surface_corner_parameter(start, bounds)?, end])
}

fn rectangular_corner_cut_face_specs(kind: RectangularCornerCutKind) -> [CornerCutFaceSpec; 2] {
    use CornerCutEdgeKind::Cut;
    use CornerCutVertex::{
        Destination as D, NorthEast as Ne, NorthWest as Nw, SouthEast as Se, SouthWest as Sw,
    };
    use RectangularBoundarySide::{East as E, North as N, South as S, West as W};
    use RectangularCornerCutKind::{
        NorthEastToSouth, NorthEastToWest, NorthWestToEast, NorthWestToSouth, SouthEastToNorth,
        SouthEastToNorthWest, SouthEastToWest, SouthWestToEast, SouthWestToNorth,
        SouthWestToNorthEast,
    };
    let boundary = |vertices, side| CornerCutEdgeSpec {
        vertices,
        kind: CornerCutEdgeKind::Boundary(side),
    };
    let cut = |vertices| CornerCutEdgeSpec {
        vertices,
        kind: Cut,
    };
    let face = |vertices: &[CornerCutVertex],
                edges: &[CornerCutEdgeSpec],
                loop_edges: &[(usize, bool)]| CornerCutFaceSpec {
        vertices: vertices.to_vec(),
        edges: edges.to_vec(),
        loop_edges: loop_edges.to_vec(),
    };

    match kind {
        SouthWestToEast => [
            face(
                &[Sw, Se, D],
                &[boundary([0, 1], S), boundary([1, 2], E), cut([0, 2])],
                &[(0, false), (1, false), (2, true)],
            ),
            face(
                &[Sw, Ne, Nw, D],
                &[
                    boundary([1, 2], N),
                    boundary([2, 0], W),
                    cut([0, 3]),
                    boundary([3, 1], E),
                ],
                &[(2, false), (3, false), (0, false), (1, false)],
            ),
        ],
        SouthWestToNorth => [
            face(
                &[Sw, Nw, D],
                &[boundary([1, 0], W), cut([0, 2]), boundary([2, 1], N)],
                &[(1, false), (2, false), (0, false)],
            ),
            face(
                &[Sw, Se, Ne, D],
                &[
                    boundary([0, 1], S),
                    boundary([1, 2], E),
                    boundary([2, 3], N),
                    cut([0, 3]),
                ],
                &[(0, false), (1, false), (2, false), (3, true)],
            ),
        ],
        SouthEastToNorth => [
            face(
                &[Sw, Se, Nw, D],
                &[
                    boundary([0, 1], S),
                    boundary([2, 0], W),
                    cut([1, 3]),
                    boundary([3, 2], N),
                ],
                &[(2, false), (3, false), (1, false), (0, false)],
            ),
            face(
                &[Se, Ne, D],
                &[boundary([0, 1], E), boundary([1, 2], N), cut([0, 2])],
                &[(0, false), (1, false), (2, true)],
            ),
        ],
        SouthEastToWest => [
            face(
                &[Sw, Se, D],
                &[boundary([0, 1], S), cut([1, 2]), boundary([2, 0], W)],
                &[(1, false), (2, false), (0, false)],
            ),
            face(
                &[Se, Ne, Nw, D],
                &[
                    boundary([0, 1], E),
                    boundary([1, 2], N),
                    boundary([2, 3], W),
                    cut([0, 3]),
                ],
                &[(0, false), (1, false), (2, false), (3, true)],
            ),
        ],
        NorthEastToWest => [
            face(
                &[Sw, Se, Ne, D],
                &[
                    boundary([0, 1], S),
                    boundary([1, 2], E),
                    cut([2, 3]),
                    boundary([3, 0], W),
                ],
                &[(2, false), (3, false), (0, false), (1, false)],
            ),
            face(
                &[Ne, Nw, D],
                &[boundary([0, 1], N), boundary([1, 2], W), cut([0, 2])],
                &[(0, false), (1, false), (2, true)],
            ),
        ],
        NorthEastToSouth => [
            face(
                &[Sw, Ne, Nw, D],
                &[
                    boundary([0, 3], S),
                    boundary([1, 2], N),
                    boundary([2, 0], W),
                    cut([1, 3]),
                ],
                &[(1, false), (2, false), (0, false), (3, true)],
            ),
            face(
                &[Se, Ne, D],
                &[boundary([0, 1], E), cut([1, 2]), boundary([2, 0], S)],
                &[(1, false), (2, false), (0, false)],
            ),
        ],
        NorthWestToSouth => [
            face(
                &[Sw, Nw, D],
                &[boundary([0, 2], S), boundary([1, 0], W), cut([1, 2])],
                &[(1, false), (0, false), (2, true)],
            ),
            face(
                &[Se, Ne, Nw, D],
                &[
                    boundary([0, 1], E),
                    boundary([1, 2], N),
                    cut([2, 3]),
                    boundary([3, 0], S),
                ],
                &[(2, false), (3, false), (0, false), (1, false)],
            ),
        ],
        NorthWestToEast => [
            face(
                &[Sw, Se, Nw, D],
                &[
                    boundary([0, 1], S),
                    boundary([1, 3], E),
                    boundary([2, 0], W),
                    cut([2, 3]),
                ],
                &[(2, false), (0, false), (1, false), (3, true)],
            ),
            face(
                &[Ne, Nw, D],
                &[boundary([0, 1], N), cut([1, 2]), boundary([2, 0], E)],
                &[(1, false), (2, false), (0, false)],
            ),
        ],
        SouthWestToNorthEast => [
            face(
                &[Sw, Se, Ne],
                &[boundary([0, 1], S), boundary([1, 2], E), cut([0, 2])],
                &[(0, false), (1, false), (2, true)],
            ),
            face(
                &[Sw, Ne, Nw],
                &[boundary([1, 2], N), boundary([2, 0], W), cut([0, 1])],
                &[(2, false), (0, false), (1, false)],
            ),
        ],
        SouthEastToNorthWest => [
            face(
                &[Se, Ne, Nw],
                &[boundary([0, 1], E), boundary([1, 2], N), cut([0, 2])],
                &[(0, false), (1, false), (2, true)],
            ),
            face(
                &[Sw, Se, Nw],
                &[boundary([0, 1], S), boundary([2, 0], W), cut([1, 2])],
                &[(2, false), (1, false), (0, false)],
            ),
        ],
    }
}

fn try_rectangular_corner_cut_face(
    surface: NurbsSurface,
    reversed: bool,
    bounds: [[Real; 2]; 2],
    destination: Point2,
    cut: SurfaceSplitCurveRef<'_>,
    spec: CornerCutFaceSpec,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let boundary_iso = rectangular_surface_boundary_iso(&surface, bounds);
    let vertex_parameters = spec
        .vertices
        .iter()
        .map(|vertex| match vertex {
            CornerCutVertex::SouthWest => {
                rectangular_surface_corner_parameter(RectangularSurfaceCorner::SouthWest, bounds)
            }
            CornerCutVertex::SouthEast => {
                rectangular_surface_corner_parameter(RectangularSurfaceCorner::SouthEast, bounds)
            }
            CornerCutVertex::NorthEast => {
                rectangular_surface_corner_parameter(RectangularSurfaceCorner::NorthEast, bounds)
            }
            CornerCutVertex::NorthWest => {
                rectangular_surface_corner_parameter(RectangularSurfaceCorner::NorthWest, bounds)
            }
            CornerCutVertex::Destination => Ok(destination),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let edge_specs = spec
        .edges
        .iter()
        .map(|edge| {
            let curve = match edge.kind {
                CornerCutEdgeKind::Boundary(side) => rectangular_surface_boundary_segment(
                    &surface,
                    side,
                    vertex_parameters[edge.vertices[0]],
                    vertex_parameters[edge.vertices[1]],
                )?,
                CornerCutEdgeKind::Cut => cut.spatial.clone(),
            };
            Ok((edge.vertices, curve))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let loop_specs = spec
        .loop_edges
        .into_iter()
        .map(|(edge, reversed)| {
            let iso = match spec.edges[edge].kind {
                CornerCutEdgeKind::Boundary(side) => boundary_iso[side.index()],
                CornerCutEdgeKind::Cut => SurfaceIso::NotIso,
            };
            (edge, reversed, iso)
        })
        .collect::<Vec<_>>();
    try_surface_cutting_face(
        surface,
        reversed,
        vertex_parameters,
        edge_specs,
        loop_specs,
        cut.parameter,
        tolerance,
    )
}

fn rectangular_surface_boundary_segment(
    surface: &NurbsSurface,
    side: RectangularBoundarySide,
    start: Point2,
    end: Point2,
) -> Result<NurbsCurve, GeometryError> {
    let (curve, forward) = match side {
        RectangularBoundarySide::South | RectangularBoundarySide::North => {
            let parameter = start.y();
            let interval = start.x().min(end.x())..=start.x().max(end.x());
            (
                surface.isocurve_u(parameter)?.try_trimmed(interval)?,
                start.x() < end.x(),
            )
        }
        RectangularBoundarySide::East | RectangularBoundarySide::West => {
            let parameter = start.x();
            let interval = start.y().min(end.y())..=start.y().max(end.y());
            (
                surface.isocurve_v(parameter)?.try_trimmed(interval)?,
                start.y() < end.y(),
            )
        }
    };
    if forward { Ok(curve) } else { curve.reversed() }
}

struct ClosedSurfaceCutSegment {
    spatial: NurbsCurve,
    parameter: NurbsCurve2,
    iso: SurfaceIso,
}

fn validate_closed_surface_cut_parameter_curve(
    curve: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let parameter_tolerance = [
        trim_parameter_epsilon(bounds[0], tolerance),
        trim_parameter_epsilon(bounds[1], tolerance),
    ];
    if !parameter_points_near(
        curve.start_point()?,
        curve.end_point()?,
        parameter_tolerance,
    ) {
        return invalid("a closed surface split p-curve must be closed");
    }
    let weight_sign = curve.control_points()[0].weight().is_sign_positive();
    for control in curve.control_points() {
        if control.weight().is_sign_positive() != weight_sign {
            return invalid("a closed surface split p-curve must have one weight sign");
        }
        let point = control.point();
        if point.x() <= bounds[0][0] + parameter_tolerance[0]
            || point.x() >= bounds[0][1] - parameter_tolerance[0]
            || point.y() <= bounds[1][0] + parameter_tolerance[1]
            || point.y() >= bounds[1][1] - parameter_tolerance[1]
        {
            return invalid("a closed surface split p-curve must lie strictly inside the bounds");
        }
    }

    let lifted = NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|control| {
                WeightedPoint3::try_new(
                    Point3::try_new(control.point().x(), control.point().y(), 0.0)?,
                    control.weight(),
                )
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        curve.knots().to_vec(),
    )?;
    let spans = lifted.spans().collect::<Vec<_>>();
    if spans.len() < 2 {
        return invalid("a closed surface split p-curve needs independently simple spans");
    }
    let span_curves = spans
        .iter()
        .map(|span| lifted.try_trimmed_with_normalized_end_weights(span.0..=span.1))
        .collect::<Result<Vec<_>, _>>()?;
    if span_curves
        .iter()
        .any(|span| !closed_surface_cut_span_control_polygon_is_monotone(span, parameter_tolerance))
    {
        return invalid("a closed surface split p-curve span is not coordinate-monotone");
    }

    let intersection_tolerance = Tolerance::try_new(
        parameter_tolerance[0]
            .max(parameter_tolerance[1])
            .max(Real::EPSILON),
        tolerance.relative().max(Real::EPSILON * 64.0),
        tolerance.angular(),
    )?;
    for first in 0..span_curves.len() {
        for second in first + 1..span_curves.len() {
            let consecutive = second == first + 1;
            let seam_pair = first == 0 && second + 1 == span_curves.len();
            let mut allowed = Vec::with_capacity(2);
            if consecutive {
                allowed.push(span_curves[first].evaluate(*span_curves[first].domain().end())?);
            }
            if seam_pair {
                allowed.push(span_curves[first].evaluate(*span_curves[first].domain().start())?);
            }
            for event in span_curves[first]
                .intersection_events_with_curve(&span_curves[second], intersection_tolerance)?
            {
                let CurveCurveIntersectionEvent::Point(intersection) = event else {
                    return invalid("a closed surface split p-curve overlaps itself");
                };
                let point = intersection.point();
                if !allowed.iter().any(|expected| {
                    point
                        .distance_to(*expected)
                        .is_ok_and(|distance| distance <= intersection_tolerance.absolute() * 8.0)
                }) {
                    return invalid("a closed surface split p-curve intersects itself");
                }
            }
        }
    }
    Ok(())
}

fn closed_surface_cut_span_control_polygon_is_monotone(
    curve: &NurbsCurve,
    tolerance: [Real; 2],
) -> bool {
    let controls = curve.control_points();
    let coordinate_is_monotone = |coordinate: usize| {
        let first = controls[0].point().to_array()[coordinate];
        let last = controls[controls.len() - 1].point().to_array()[coordinate];
        let delta = last - first;
        if delta.abs() <= tolerance[coordinate] {
            controls.iter().all(|control| {
                (control.point().to_array()[coordinate] - first).abs() <= tolerance[coordinate]
            })
        } else {
            let direction = delta.signum();
            controls.windows(2).all(|pair| {
                (pair[1].point().to_array()[coordinate] - pair[0].point().to_array()[coordinate])
                    * direction
                    >= -tolerance[coordinate]
            })
        }
    };
    let has_extent = (0..2).any(|coordinate| {
        (controls[controls.len() - 1].point().to_array()[coordinate]
            - controls[0].point().to_array()[coordinate])
            .abs()
            > tolerance[coordinate]
    });
    has_extent && (0..2).all(coordinate_is_monotone)
}

fn closed_surface_cut_segments(
    surface: &NurbsSurface,
    spatial: &NurbsCurve,
    parameter: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> Result<Vec<ClosedSurfaceCutSegment>, GeometryError> {
    let domain = spatial.domain();
    let mut breaks = vec![*domain.start()];
    for (knot, multiplicity) in spatial.interior_knot_groups() {
        if multiplicity == spatial.degree() && spatial.kink_angle_at(knot)? > tolerance.angular() {
            breaks.push(knot);
        }
    }
    breaks.push(*domain.end());
    if breaks.len() - 1 > crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }

    breaks
        .windows(2)
        .map(|interval| {
            let spatial_segment = if interval == [*domain.start(), *domain.end()] {
                spatial.clone()
            } else {
                spatial.try_trimmed_with_normalized_end_weights(interval[0]..=interval[1])?
            };
            let start = parameter.evaluate(interval[0])?;
            let end = parameter.evaluate(interval[1])?;
            let parameter_segment = if interval == [*domain.start(), *domain.end()] {
                parameter.clone()
            } else {
                surface_split_parameter_curve(surface, &spatial_segment, start, end, tolerance)?
            };
            let iso = surface_cut_arrangement_iso(
                &parameter_segment,
                bounds,
                [
                    trim_parameter_epsilon(bounds[0], tolerance),
                    trim_parameter_epsilon(bounds[1], tolerance),
                ],
            )?;
            Ok(ClosedSurfaceCutSegment {
                spatial: spatial_segment,
                parameter: parameter_segment,
                iso,
            })
        })
        .collect()
}

fn closed_surface_cut_edges(
    segments: &[ClosedSurfaceCutSegment],
    vertex_offset: usize,
) -> Result<Vec<BrepEdge>, GeometryError> {
    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            BrepEdge::try_new(
                [
                    vertex_offset + index,
                    vertex_offset + (index + 1) % segments.len(),
                ],
                segment.spatial.clone(),
                0.0,
            )
        })
        .collect()
}

fn closed_surface_cut_loop(
    segments: &[ClosedSurfaceCutSegment],
    vertex_offset: usize,
    edge_offset: usize,
    loop_type: BrepLoopType,
    reverse_source: bool,
) -> Result<BrepLoop, GeometryError> {
    let mut trims = Vec::with_capacity(segments.len());
    for step in 0..segments.len() {
        let index = if reverse_source {
            segments.len() - step - 1
        } else {
            step
        };
        let segment = &segments[index];
        let source_vertices = [
            vertex_offset + index,
            vertex_offset + (index + 1) % segments.len(),
        ];
        let (vertices, parameter) = if reverse_source {
            (
                [source_vertices[1], source_vertices[0]],
                segment.parameter.reversed()?,
            )
        } else {
            (source_vertices, segment.parameter.clone())
        };
        trims.push(BrepTrim::try_new(
            vertices,
            Some(edge_offset + index),
            reverse_source,
            parameter,
            BrepTrimType::Boundary,
            segment.iso,
            [0.0, 0.0],
        )?);
    }
    BrepLoop::try_new(loop_type, trims)
}

#[derive(Clone)]
struct SurfaceCutArrangementCurve {
    spatial: NurbsCurve,
    parameter: NurbsCurve2,
    endpoints: [Point2; 2],
    closed: bool,
    breaks: Vec<SurfaceCutArrangementBreak>,
}

#[derive(Clone, Copy)]
struct SurfaceCutArrangementBreak {
    parameter: Real,
    node: usize,
}

#[derive(Clone, Copy)]
struct SurfaceCutArrangementNode {
    parameter: Point2,
    point: Point3,
}

#[derive(Clone)]
struct SurfaceCutArrangementEdge {
    nodes: [usize; 2],
    spatial: NurbsCurve,
    parameter: NurbsCurve2,
    iso: SurfaceIso,
    kind: SurfaceCutArrangementEdgeKind,
    smooth_continuations: u8,
    coincidences: Vec<SurfaceCutArrangementCoincidence>,
}

struct SurfaceCutArrangementRegion {
    outer: Vec<usize>,
    holes: Vec<Vec<usize>>,
}

struct SurfaceCutArrangementSourceInfo {
    node_rank: Vec<usize>,
    first_closed: Option<usize>,
    closed: Vec<bool>,
    first_nodes: Vec<bool>,
    first_tangent_nodes: Vec<bool>,
    first_overlap_nodes: Vec<bool>,
    first_opposite_overlap_nodes: Vec<bool>,
    first_boundary_overlap_edges: Vec<bool>,
    first_maximum_segment: usize,
}

#[derive(Clone, Copy)]
enum SurfaceCutArrangementEdgeKind {
    Boundary(RectangularBoundarySide),
    Cut { source: usize, segment: usize },
}

#[derive(Clone, Copy)]
struct SurfaceCutArrangementCoincidence {
    source: usize,
    segment: usize,
    same_direction: bool,
}

#[derive(Clone, Copy)]
struct SurfaceCutHalfedgeDirection {
    tangent: [Real; 2],
    angle: Real,
    bend: Real,
}

fn surface_cut_arrangement_edge_contributors(
    edge: &SurfaceCutArrangementEdge,
) -> impl Iterator<Item = SurfaceCutArrangementCoincidence> + '_ {
    let representative = match edge.kind {
        SurfaceCutArrangementEdgeKind::Boundary(_) => None,
        SurfaceCutArrangementEdgeKind::Cut { source, segment } => {
            Some(SurfaceCutArrangementCoincidence {
                source,
                segment,
                same_direction: true,
            })
        }
    };
    representative
        .into_iter()
        .chain(edge.coincidences.iter().copied())
}

fn surface_cut_arrangement_edge_contributor(
    edge: &SurfaceCutArrangementEdge,
    source: usize,
) -> Option<SurfaceCutArrangementCoincidence> {
    surface_cut_arrangement_edge_contributors(edge).find(|contributor| contributor.source == source)
}

fn surface_cut_halfedge_follows_source(
    edge: &SurfaceCutArrangementEdge,
    halfedge: usize,
    source: usize,
) -> Option<bool> {
    surface_cut_arrangement_edge_contributor(edge, source)
        .map(|contributor| halfedge.is_multiple_of(2) == contributor.same_direction)
}

fn try_rectangular_surface_cut_arrangement(
    surface: NurbsSurface,
    bounds: [[Real; 2]; 2],
    cut_curves: Vec<NurbsCurve>,
    reversed: bool,
    tolerance: Tolerance,
) -> Result<Vec<Brep>, GeometryError> {
    let parameter_tolerance = [
        trim_parameter_epsilon(bounds[0], tolerance),
        trim_parameter_epsilon(bounds[1], tolerance),
    ];
    let mut cuts = Vec::<SurfaceCutArrangementCurve>::new();
    for curve in cut_curves {
        let candidate = try_surface_cut_arrangement_curve(
            &surface,
            bounds,
            curve,
            parameter_tolerance,
            tolerance,
        )?;
        let mut duplicate = false;
        for existing in &cuts {
            for event in candidate
                .spatial
                .intersection_events_with_curve(&existing.spatial, tolerance)?
            {
                let CurveCurveIntersectionEvent::Overlap(overlap) = event else {
                    continue;
                };
                if surface_cut_overlap_covers_curves(&candidate.spatial, &existing.spatial, overlap)
                {
                    duplicate = true;
                }
            }
            if duplicate {
                break;
            }
        }
        if !duplicate {
            cuts.push(candidate);
        }
    }
    if cuts.is_empty() {
        return invalid("a surface cutting arrangement requires a distinct curve");
    }
    let closed = cuts.iter().map(|cut| cut.closed).collect::<Vec<_>>();

    let corner_parameters = [
        Point2::try_new(bounds[0][0], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][1])?,
        Point2::try_new(bounds[0][0], bounds[1][1])?,
    ];
    let mut nodes = corner_parameters
        .into_iter()
        .map(|parameter| {
            Ok(SurfaceCutArrangementNode {
                parameter,
                point: surface.evaluate(parameter.x(), parameter.y())?,
            })
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    for cut in &mut cuts {
        let domain = cut.spatial.domain();
        let endpoint_parameters = [*domain.start(), *domain.end()];
        for (end, endpoint_parameter) in endpoint_parameters.into_iter().enumerate() {
            let parameter = cut.endpoints[end];
            let node = surface_cut_arrangement_node(
                &mut nodes,
                parameter,
                surface.evaluate(parameter.x(), parameter.y())?,
                parameter_tolerance,
            );
            cut.breaks.push(SurfaceCutArrangementBreak {
                parameter: endpoint_parameter,
                node,
            });
        }
        if cut.closed {
            for (knot, multiplicity) in cut.spatial.interior_knot_groups() {
                if multiplicity != cut.spatial.degree()
                    || cut.spatial.kink_angle_at(knot)? <= tolerance.angular()
                {
                    continue;
                }
                let parameter = cut.parameter.evaluate(knot)?;
                let node = surface_cut_arrangement_node(
                    &mut nodes,
                    parameter,
                    cut.spatial.evaluate(knot)?,
                    parameter_tolerance,
                );
                cut.breaks.push(SurfaceCutArrangementBreak {
                    parameter: knot,
                    node,
                });
            }
        }
    }

    for first_index in 0..cuts.len() {
        for second_index in first_index + 1..cuts.len() {
            let (first_slice, second_slice) = cuts.split_at_mut(second_index);
            let first = &mut first_slice[first_index];
            let second = &mut second_slice[0];
            let events = first
                .spatial
                .intersection_events_with_curve(&second.spatial, tolerance)?;
            for event in events {
                let contacts = match event {
                    CurveCurveIntersectionEvent::Point(intersection) => [Some(intersection), None],
                    CurveCurveIntersectionEvent::Overlap(overlap) => {
                        [Some(overlap.start()), Some(overlap.end())]
                    }
                };
                for intersection in contacts.into_iter().flatten() {
                    let first_parameter = intersection.first_parameter();
                    let second_parameter = intersection.second_parameter();
                    let first_uv = first.parameter.evaluate(first_parameter)?;
                    let second_uv = second.parameter.evaluate(second_parameter)?;
                    if !parameter_points_near(
                        first_uv,
                        second_uv,
                        parameter_tolerance.map(|value| value * 8.0),
                    ) {
                        return invalid(
                            "a model-space cutter intersection is not shared in parameter space",
                        );
                    }
                    let averaged = Point2::try_new(
                        first_uv.x() * 0.5 + second_uv.x() * 0.5,
                        first_uv.y() * 0.5 + second_uv.y() * 0.5,
                    )?;
                    let (parameter, _) = snap_surface_cut_arrangement_parameter(
                        averaged,
                        bounds,
                        parameter_tolerance,
                    )?;
                    let node = surface_cut_arrangement_node(
                        &mut nodes,
                        parameter,
                        intersection.point(),
                        parameter_tolerance,
                    );
                    push_surface_cut_arrangement_break(first, first_parameter, node)?;
                    push_surface_cut_arrangement_break(second, second_parameter, node)?;
                }
            }
        }
    }

    let mut node_rank = vec![usize::MAX; nodes.len()];
    let mut next_node_rank = 0;
    for rank in node_rank.iter_mut().take(4) {
        *rank = next_node_rank;
        next_node_rank += 1;
    }
    for cut in &cuts {
        let mut breaks = cut.breaks.clone();
        breaks.sort_by(|left, right| left.parameter.total_cmp(&right.parameter));
        for split in breaks {
            if node_rank[split.node] == usize::MAX {
                node_rank[split.node] = next_node_rank;
                next_node_rank += 1;
            }
        }
    }
    if node_rank.contains(&usize::MAX) {
        return invalid("a surface cutting arrangement node is not on a cutter");
    }

    let mut edges = Vec::<SurfaceCutArrangementEdge>::new();
    let boundary_iso = rectangular_surface_boundary_iso(&surface, bounds);
    for side in [
        RectangularBoundarySide::South,
        RectangularBoundarySide::East,
        RectangularBoundarySide::North,
        RectangularBoundarySide::West,
    ] {
        let mut side_nodes = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                surface_cut_parameter_on_side(node.parameter, side, bounds, parameter_tolerance)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        side_nodes.sort_by(|&left, &right| {
            let coordinate = |node: usize| match side {
                RectangularBoundarySide::South | RectangularBoundarySide::North => {
                    nodes[node].parameter.x()
                }
                RectangularBoundarySide::East | RectangularBoundarySide::West => {
                    nodes[node].parameter.y()
                }
            };
            let ordering = coordinate(left).total_cmp(&coordinate(right));
            if matches!(
                side,
                RectangularBoundarySide::North | RectangularBoundarySide::West
            ) {
                ordering.reverse()
            } else {
                ordering
            }
        });
        side_nodes.dedup();
        for pair in side_nodes.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let start = nodes[pair[0]].parameter;
            let end = nodes[pair[1]].parameter;
            push_surface_cut_arrangement_edge(
                &mut edges,
                SurfaceCutArrangementEdge {
                    nodes: [pair[0], pair[1]],
                    spatial: rectangular_surface_boundary_segment(&surface, side, start, end)?,
                    parameter: NurbsCurve2::try_line(start, end)?,
                    iso: boundary_iso[side.index()],
                    kind: SurfaceCutArrangementEdgeKind::Boundary(side),
                    smooth_continuations: 0,
                    coincidences: Vec::new(),
                },
            )?;
        }
    }

    for (cut_index, cut) in cuts.iter_mut().enumerate() {
        cut.breaks
            .sort_by(|left, right| left.parameter.total_cmp(&right.parameter));
        let parameter_epsilon = surface_cut_curve_parameter_epsilon(&cut.spatial);
        let mut unique_breaks = Vec::<SurfaceCutArrangementBreak>::new();
        for split in cut.breaks.iter().copied() {
            if let Some(previous) = unique_breaks.last()
                && (previous.parameter - split.parameter).abs() <= parameter_epsilon
            {
                if previous.node != split.node {
                    return invalid("one cutter parameter maps to distinct arrangement nodes");
                }
                continue;
            }
            unique_breaks.push(split);
        }
        let domain = cut.spatial.domain();
        for (segment_index, pair) in unique_breaks.windows(2).enumerate() {
            let complete_closed_edge = cut.closed
                && unique_breaks.len() == 2
                && pair[0].parameter == *domain.start()
                && pair[1].parameter == *domain.end();
            if (pair[0].node == pair[1].node && !complete_closed_edge)
                || pair[1].parameter - pair[0].parameter <= parameter_epsilon
            {
                continue;
            }
            let spatial =
                if pair[0].parameter == *domain.start() && pair[1].parameter == *domain.end() {
                    cut.spatial.clone()
                } else {
                    cut.spatial.try_trimmed_with_normalized_end_weights(
                        pair[0].parameter..=pair[1].parameter,
                    )?
                };
            let start = nodes[pair[0].node].parameter;
            let end = nodes[pair[1].node].parameter;
            let parameter =
                surface_split_parameter_curve(&surface, &spatial, start, end, tolerance)?;
            let iso = surface_cut_arrangement_iso(&parameter, bounds, parameter_tolerance)?;
            // Rhino retains the coincident contributor that continues
            // tangentially beyond the shared span rather than one that kinks.
            let smooth_continuations = [pair[0].parameter, pair[1].parameter]
                .into_iter()
                .filter(|parameter| {
                    *parameter > *domain.start() + parameter_epsilon
                        && *parameter < *domain.end() - parameter_epsilon
                })
                .map(|parameter| {
                    cut.spatial
                        .kink_angle_at(parameter)
                        .map(|angle| u8::from(angle <= tolerance.angular()))
                })
                .try_fold(0_u8, |total, continuation| {
                    continuation.map(|continuation| total + continuation)
                })?;
            push_unique_surface_cut_arrangement_edge(
                &mut edges,
                SurfaceCutArrangementEdge {
                    nodes: [pair[0].node, pair[1].node],
                    spatial,
                    parameter,
                    iso,
                    kind: SurfaceCutArrangementEdgeKind::Cut {
                        source: cut_index,
                        segment: segment_index,
                    },
                    smooth_continuations,
                    coincidences: Vec::new(),
                },
                tolerance,
                &closed,
            )?;
        }
    }

    // Rhino uses the open curve as the representative of an open/closed
    // overlap. If the closed curve was supplied first, its local vertex order
    // advances from the node following the replaced edge rather than from the
    // original closed seam.
    if closed[0]
        && let Some(overlap_end) = edges.iter().find_map(|edge| {
            let SurfaceCutArrangementEdgeKind::Cut { source, .. } = edge.kind else {
                return None;
            };
            if closed[source] {
                return None;
            }
            edge.coincidences
                .iter()
                .find(|coincidence| coincidence.source == 0)
                .map(|coincidence| edge.nodes[usize::from(coincidence.same_direction)])
        })
    {
        let mut first_nodes = cuts[0]
            .breaks
            .iter()
            .map(|split| split.node)
            .collect::<Vec<_>>();
        first_nodes.sort_by_key(|node| node_rank[*node]);
        first_nodes.dedup();
        if let Some(start) = first_nodes.iter().position(|node| *node == overlap_end) {
            first_nodes.rotate_left(start);
            let mut ranks = first_nodes
                .iter()
                .map(|node| node_rank[*node])
                .collect::<Vec<_>>();
            ranks.sort_unstable();
            for (node, rank) in first_nodes.into_iter().zip(ranks) {
                node_rank[node] = rank;
            }
        }
    }

    let halfedge_count = edges
        .len()
        .checked_mul(2)
        .ok_or(GeometryError::TooManySurfaceWires)?;
    let halfedge_directions = (0..halfedge_count)
        .map(|halfedge| surface_cut_halfedge_direction(&edges, halfedge))
        .collect::<Result<Vec<_>, _>>()?;
    let mut outgoing = vec![Vec::<usize>::new(); nodes.len()];
    for (edge_index, edge) in edges.iter().enumerate() {
        outgoing[edge.nodes[0]].push(edge_index * 2);
        outgoing[edge.nodes[1]].push(edge_index * 2 + 1);
    }
    // A tangential contact has the same unoriented tangent line on the first
    // cutter and a later cutter. Rhino uses a different trim seam at these
    // pinched vertices than it does at a transverse crossing.
    let parallel_tolerance = tolerance.angular().clamp(Real::EPSILON * 256.0, 1.0e-7);
    let first_tangent_nodes = outgoing
        .iter()
        .map(|halfedges| {
            halfedges.iter().copied().any(|first_halfedge| {
                if !matches!(
                    edges[first_halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
                ) {
                    return false;
                }
                let first_tangent = halfedge_directions[first_halfedge].tangent;
                halfedges.iter().copied().any(|second_halfedge| {
                    if !matches!(
                        edges[second_halfedge / 2].kind,
                        SurfaceCutArrangementEdgeKind::Cut { source, .. } if source != 0
                    ) {
                        return false;
                    }
                    let second_tangent = halfedge_directions[second_halfedge].tangent;
                    first_tangent[0]
                        .mul_add(second_tangent[1], -first_tangent[1] * second_tangent[0])
                        .abs()
                        <= parallel_tolerance
                })
            })
        })
        .collect::<Vec<_>>();
    let mut next = vec![usize::MAX; halfedge_count];
    for halfedges in &mut outgoing {
        sort_surface_cut_outgoing_halfedges(halfedges, &halfedge_directions, tolerance.angular());
        for (index, &outgoing_halfedge) in halfedges.iter().enumerate() {
            let clockwise = halfedges[(index + halfedges.len() - 1) % halfedges.len()];
            next[outgoing_halfedge ^ 1] = clockwise;
        }
    }
    if next.contains(&usize::MAX) {
        return invalid("a surface cutting arrangement contains a dangling edge");
    }

    let mut visited = vec![false; halfedge_count];
    let mut outer_cycles = Vec::<Vec<usize>>::new();
    let mut hole_cycles = Vec::<Vec<usize>>::new();
    for start in 0..halfedge_count {
        if visited[start] {
            continue;
        }
        let mut cycle = Vec::new();
        let mut current = start;
        loop {
            if visited[current] {
                if current != start {
                    return invalid("surface cutting arrangement face traversal crossed itself");
                }
                break;
            }
            visited[current] = true;
            cycle.push(current);
            current = next[current];
            if cycle.len() > halfedge_count {
                return invalid("surface cutting arrangement face traversal did not close");
            }
        }
        if cycle.is_empty() {
            continue;
        }
        if let Some(area) = sampled_surface_cut_cycle_signed_area(&edges, &cycle)? {
            if area > 0.0 {
                outer_cycles.push(cycle);
            } else {
                hole_cycles.push(cycle);
            }
        }
    }
    if outer_cycles.len() < 2 {
        return invalid("surface cutting arrangement did not divide the rectangular region");
    }
    // The halfedge walk emits the unbounded exterior clockwise as well as
    // clockwise inner boundaries. Only the exterior contains rectangle edges.
    hole_cycles.retain(|cycle| {
        !cycle.iter().any(|halfedge| {
            matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Boundary(_)
            )
        })
    });
    if outer_cycles.len() + hole_cycles.len() > crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    let first_closed = cuts.iter().position(|cut| cut.closed);
    let mut first_source_nodes = vec![false; nodes.len()];
    let mut first_overlap_nodes = vec![false; nodes.len()];
    let mut first_opposite_overlap_nodes = vec![false; nodes.len()];
    let mut first_boundary_overlap_edges = vec![false; edges.len()];
    let mut first_maximum_segment = 0_usize;
    for (edge_index, edge) in edges.iter().enumerate() {
        if let Some(first_contributor) = surface_cut_arrangement_edge_contributor(edge, 0) {
            first_maximum_segment = first_maximum_segment.max(first_contributor.segment);
            first_boundary_overlap_edges[edge_index] = !edge.coincidences.is_empty()
                && edge.nodes.into_iter().any(|node| {
                    [
                        RectangularBoundarySide::South,
                        RectangularBoundarySide::East,
                        RectangularBoundarySide::North,
                        RectangularBoundarySide::West,
                    ]
                    .into_iter()
                    .any(|side| {
                        surface_cut_parameter_on_side(
                            nodes[node].parameter,
                            side,
                            bounds,
                            parameter_tolerance,
                        )
                    })
                });
            for node in edge.nodes {
                first_source_nodes[node] = true;
                if !edge.coincidences.is_empty() {
                    first_overlap_nodes[node] = true;
                }
                if surface_cut_arrangement_edge_contributors(edge).any(|contributor| {
                    contributor.source != 0
                        && contributor.same_direction != first_contributor.same_direction
                }) {
                    first_opposite_overlap_nodes[node] = true;
                }
            }
        }
    }
    let source_info = SurfaceCutArrangementSourceInfo {
        node_rank,
        first_closed,
        closed,
        first_nodes: first_source_nodes,
        first_tangent_nodes,
        first_overlap_nodes,
        first_opposite_overlap_nodes,
        first_boundary_overlap_edges,
        first_maximum_segment,
    };
    for cycle in &mut outer_cycles {
        rotate_surface_cut_outer_cycle(
            cycle,
            &edges,
            &nodes,
            bounds,
            parameter_tolerance,
            &source_info,
        )?;
    }
    let mut holes_by_outer = vec![Vec::<Vec<usize>>::new(); outer_cycles.len()];
    for mut cycle in hole_cycles {
        rotate_surface_cut_hole_cycle(
            &mut cycle,
            &edges,
            &nodes,
            bounds,
            parameter_tolerance,
            &source_info,
        )?;
        let parent =
            surface_cut_hole_parent(&edges, &outer_cycles, &cycle, bounds, parameter_tolerance)?
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "a surface cutting arrangement hole has no containing face",
                })?;
        holes_by_outer[parent].push(cycle);
    }
    for holes in &mut holes_by_outer {
        holes.sort_by(|left, right| {
            // Rhino places kinky closed loops before smooth one-edge loops.
            // Source order is stable within kinky loops and reversed within
            // the self-edge representation used for smooth closed curves.
            right
                .len()
                .cmp(&left.len())
                .then_with(|| {
                    let left_source = surface_cut_cycle_source_key(&edges, left);
                    let right_source = surface_cut_cycle_source_key(&edges, right);
                    if left.len() == 1 {
                        right_source.cmp(&left_source)
                    } else {
                        left_source.cmp(&right_source)
                    }
                })
                .then_with(|| {
                    surface_cut_cycle_bounds(&edges, &nodes, left)
                        .into_iter()
                        .zip(surface_cut_cycle_bounds(&edges, &nodes, right))
                        .find_map(|(left, right)| {
                            let ordering = left.total_cmp(&right);
                            (ordering != std::cmp::Ordering::Equal).then_some(ordering)
                        })
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }

    let mut regions = outer_cycles
        .into_iter()
        .zip(holes_by_outer)
        .map(|(outer, holes)| SurfaceCutArrangementRegion { outer, holes })
        .collect::<Vec<_>>();
    regions.sort_by(|left, right| {
        surface_cut_cycle_bounds(&edges, &nodes, &left.outer)
            .into_iter()
            .zip(surface_cut_cycle_bounds(&edges, &nodes, &right.outer))
            .find_map(|(left, right)| {
                let ordering = left.total_cmp(&right);
                (ordering != std::cmp::Ordering::Equal).then_some(ordering)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    regions
        .iter()
        .map(|region| {
            try_surface_cut_arrangement_face(
                surface.clone(),
                reversed,
                &nodes,
                &edges,
                region,
                &source_info,
                tolerance,
            )
        })
        .collect()
}

fn try_surface_cut_arrangement_curve(
    surface: &NurbsSurface,
    bounds: [[Real; 2]; 2],
    spatial: NurbsCurve,
    parameter_tolerance: [Real; 2],
    tolerance: Tolerance,
) -> Result<SurfaceCutArrangementCurve, GeometryError> {
    let domain = spatial.domain();
    let closed = spatial.is_closed()?;
    if closed {
        let seam = spatial.evaluate(*domain.start())?;
        let (u, v) = surface.closest_parameters(seam, tolerance)?;
        let seam_parameter = Point2::try_new(u, v)?;
        let parameter = surface_split_parameter_curve(
            surface,
            &spatial,
            seam_parameter,
            seam_parameter,
            tolerance,
        )?;
        validate_closed_surface_cut_parameter_curve(&parameter, bounds, tolerance)?;
        surface_cut_arrangement_iso(&parameter, bounds, parameter_tolerance)?;
        return Ok(SurfaceCutArrangementCurve {
            spatial,
            parameter,
            endpoints: [seam_parameter; 2],
            closed,
            breaks: Vec::new(),
        });
    }
    let mut endpoints = [Point2::try_new(0.0, 0.0)?; 2];
    for (end, curve_parameter) in [*domain.start(), *domain.end()].into_iter().enumerate() {
        let point = spatial.evaluate(curve_parameter)?;
        let (u, v) = surface.closest_parameters(point, tolerance)?;
        let candidate = Point2::try_new(u, v)?;
        let (snapped, on_boundary) =
            snap_surface_cut_arrangement_parameter(candidate, bounds, parameter_tolerance)?;
        if !on_boundary {
            return invalid("a surface cutting arrangement curve must end on the boundary");
        }
        let surface_point = surface.evaluate(snapped.x(), snapped.y())?;
        let coordinate_scale = point
            .to_array()
            .into_iter()
            .chain(surface_point.to_array())
            .map(Real::abs)
            .fold(1.0, Real::max);
        let allowed = tolerance
            .absolute()
            .max(tolerance.relative() * coordinate_scale)
            * 4.0;
        if point.distance_to(surface_point)? > allowed {
            return invalid("a surface cutting arrangement curve endpoint misses the surface");
        }
        endpoints[end] = snapped;
    }
    if parameter_points_near(endpoints[0], endpoints[1], parameter_tolerance) {
        return invalid("a surface cutting arrangement curve has coincident endpoints");
    }
    let parameter =
        surface_split_parameter_curve(surface, &spatial, endpoints[0], endpoints[1], tolerance)?;
    for (span_start, span_end) in parameter.spans() {
        for sample in 0..=8 {
            let fraction = sample as Real / 8.0;
            let curve_parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
            let point = parameter.evaluate(curve_parameter)?;
            snap_surface_cut_arrangement_parameter(point, bounds, parameter_tolerance)?;
        }
    }
    surface_cut_arrangement_iso(&parameter, bounds, parameter_tolerance)?;
    Ok(SurfaceCutArrangementCurve {
        spatial,
        parameter,
        endpoints,
        closed,
        breaks: Vec::new(),
    })
}

fn surface_cut_arrangement_iso(
    curve: &NurbsCurve2,
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
) -> Result<SurfaceIso, GeometryError> {
    let controls = curve.control_points();
    let first = controls[0].point();
    let constant_u = controls
        .iter()
        .all(|control| (control.point().x() - first.x()).abs() <= tolerance[0]);
    let constant_v = controls
        .iter()
        .all(|control| (control.point().y() - first.y()).abs() <= tolerance[1]);
    if constant_u && constant_v {
        return invalid("a surface cutting arrangement curve encloses no extent");
    }
    if constant_u {
        if first.x() <= bounds[0][0] + tolerance[0] || first.x() >= bounds[0][1] - tolerance[0] {
            return invalid("a surface cutting arrangement curve coincides with the boundary");
        }
        Ok(SurfaceIso::InteriorUConstant)
    } else if constant_v {
        if first.y() <= bounds[1][0] + tolerance[1] || first.y() >= bounds[1][1] - tolerance[1] {
            return invalid("a surface cutting arrangement curve coincides with the boundary");
        }
        Ok(SurfaceIso::InteriorVConstant)
    } else {
        Ok(SurfaceIso::NotIso)
    }
}

fn snap_surface_cut_arrangement_parameter(
    parameter: Point2,
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
) -> Result<(Point2, bool), GeometryError> {
    let mut coordinates = parameter.to_array();
    let mut on_boundary = false;
    for coordinate in 0..2 {
        if coordinates[coordinate] < bounds[coordinate][0] - tolerance[coordinate]
            || coordinates[coordinate] > bounds[coordinate][1] + tolerance[coordinate]
        {
            return invalid("a surface cutting arrangement curve leaves the trim bounds");
        }
        coordinates[coordinate] =
            coordinates[coordinate].clamp(bounds[coordinate][0], bounds[coordinate][1]);
        if (coordinates[coordinate] - bounds[coordinate][0]).abs() <= tolerance[coordinate] {
            coordinates[coordinate] = bounds[coordinate][0];
            on_boundary = true;
        } else if (coordinates[coordinate] - bounds[coordinate][1]).abs() <= tolerance[coordinate] {
            coordinates[coordinate] = bounds[coordinate][1];
            on_boundary = true;
        }
    }
    Ok((Point2::try_from(coordinates)?, on_boundary))
}

fn surface_cut_arrangement_node(
    nodes: &mut Vec<SurfaceCutArrangementNode>,
    parameter: Point2,
    point: Point3,
    tolerance: [Real; 2],
) -> usize {
    if let Some(index) = nodes
        .iter()
        .position(|node| parameter_points_near(node.parameter, parameter, tolerance))
    {
        index
    } else {
        let index = nodes.len();
        nodes.push(SurfaceCutArrangementNode { parameter, point });
        index
    }
}

fn surface_cut_curve_parameter_epsilon(curve: &NurbsCurve) -> Real {
    let domain = curve.domain();
    let scale = domain.start().abs().max(domain.end().abs()).max(1.0);
    Real::EPSILON * scale * 4096.0
}

fn push_surface_cut_arrangement_break(
    cut: &mut SurfaceCutArrangementCurve,
    parameter: Real,
    node: usize,
) -> Result<(), GeometryError> {
    let epsilon = surface_cut_curve_parameter_epsilon(&cut.spatial);
    if let Some(existing) = cut
        .breaks
        .iter()
        .find(|existing| (existing.parameter - parameter).abs() <= epsilon)
    {
        if existing.node != node {
            return invalid("one cutter parameter maps to distinct arrangement nodes");
        }
        return Ok(());
    }
    cut.breaks
        .push(SurfaceCutArrangementBreak { parameter, node });
    Ok(())
}

fn surface_cut_parameter_on_side(
    parameter: Point2,
    side: RectangularBoundarySide,
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
) -> bool {
    match side {
        RectangularBoundarySide::South => (parameter.y() - bounds[1][0]).abs() <= tolerance[1],
        RectangularBoundarySide::East => (parameter.x() - bounds[0][1]).abs() <= tolerance[0],
        RectangularBoundarySide::North => (parameter.y() - bounds[1][1]).abs() <= tolerance[1],
        RectangularBoundarySide::West => (parameter.x() - bounds[0][0]).abs() <= tolerance[0],
    }
}

fn push_surface_cut_arrangement_edge(
    edges: &mut Vec<SurfaceCutArrangementEdge>,
    edge: SurfaceCutArrangementEdge,
) -> Result<(), GeometryError> {
    if edge.nodes[0] == edge.nodes[1] && !edge.spatial.is_closed()? {
        return invalid("a surface cutting arrangement edge has coincident nodes");
    }
    if edges.len() == crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    edges.push(edge);
    Ok(())
}

fn push_unique_surface_cut_arrangement_edge(
    edges: &mut Vec<SurfaceCutArrangementEdge>,
    edge: SurfaceCutArrangementEdge,
    tolerance: Tolerance,
    closed_sources: &[bool],
) -> Result<(), GeometryError> {
    let SurfaceCutArrangementEdgeKind::Cut { source, segment } = edge.kind else {
        return push_surface_cut_arrangement_edge(edges, edge);
    };
    for existing in edges.iter_mut() {
        let SurfaceCutArrangementEdgeKind::Cut {
            source: existing_source,
            ..
        } = existing.kind
        else {
            continue;
        };
        if !(existing.nodes == edge.nodes || existing.nodes == [edge.nodes[1], edge.nodes[0]]) {
            continue;
        }
        for event in edge
            .spatial
            .intersection_events_with_curve(&existing.spatial, tolerance)?
        {
            let CurveCurveIntersectionEvent::Overlap(overlap) = event else {
                continue;
            };
            if surface_cut_overlap_covers_curves(&edge.spatial, &existing.spatial, overlap) {
                let same_direction = existing.nodes == edge.nodes;
                // An open contributor wins over a closed loop. Within the
                // same topology class, continuity is Rhino's tie-breaker;
                // exact ties remain stable in cutter order.
                let prefer_incoming = (closed_sources[existing_source] && !closed_sources[source])
                    || (closed_sources[existing_source] == closed_sources[source]
                        && edge.smooth_continuations > existing.smooth_continuations);
                if prefer_incoming {
                    let SurfaceCutArrangementEdgeKind::Cut {
                        source: previous_source,
                        segment: previous_segment,
                    } = existing.kind
                    else {
                        unreachable!("a coincident arrangement edge is a cutter")
                    };
                    let mut coincidences = std::mem::take(&mut existing.coincidences);
                    if !same_direction {
                        for coincidence in &mut coincidences {
                            coincidence.same_direction = !coincidence.same_direction;
                        }
                    }
                    coincidences.push(SurfaceCutArrangementCoincidence {
                        source: previous_source,
                        segment: previous_segment,
                        same_direction,
                    });
                    *existing = edge;
                    existing.coincidences = coincidences;
                } else {
                    existing
                        .coincidences
                        .push(SurfaceCutArrangementCoincidence {
                            source,
                            segment,
                            same_direction,
                        });
                }
                return Ok(());
            }
        }
    }
    push_surface_cut_arrangement_edge(edges, edge)
}

fn surface_cut_overlap_covers_curves(
    first: &NurbsCurve,
    second: &NurbsCurve,
    overlap: CurveCurveOverlap,
) -> bool {
    let first_domain = first.domain();
    let second_domain = second.domain();
    let first_epsilon = surface_cut_curve_parameter_epsilon(first);
    let second_epsilon = surface_cut_curve_parameter_epsilon(second);
    let first_parameters = [
        overlap.start().first_parameter(),
        overlap.end().first_parameter(),
    ];
    let second_parameters = [
        overlap.start().second_parameter(),
        overlap.end().second_parameter(),
    ];
    (first_parameters[0].min(first_parameters[1]) - *first_domain.start()).abs() <= first_epsilon
        && (first_parameters[0].max(first_parameters[1]) - *first_domain.end()).abs()
            <= first_epsilon
        && (second_parameters[0].min(second_parameters[1]) - *second_domain.start()).abs()
            <= second_epsilon
        && (second_parameters[0].max(second_parameters[1]) - *second_domain.end()).abs()
            <= second_epsilon
}

fn surface_cut_halfedge_direction(
    edges: &[SurfaceCutArrangementEdge],
    halfedge: usize,
) -> Result<SurfaceCutHalfedgeDirection, GeometryError> {
    const BEND_SAMPLE_FRACTION: Real = 1.0e-5;

    let edge = &edges[halfedge / 2];
    let reversed = !halfedge.is_multiple_of(2);
    let domain = edge.parameter.domain();
    let parameter = if reversed {
        *domain.end()
    } else {
        *domain.start()
    };
    let (_, derivative) = edge.parameter.evaluate_with_derivative(parameter)?;
    let mut tangent = if reversed {
        [-derivative[0], -derivative[1]]
    } else {
        derivative
    };
    let mut tangent_length = tangent[0].hypot(tangent[1]);
    let regular_endpoint_tangent = tangent_length.is_finite() && tangent_length > 0.0;
    if tangent[0] == 0.0 && tangent[1] == 0.0 {
        let start = edge.parameter.start_point()?;
        let end = edge.parameter.end_point()?;
        tangent = if reversed {
            [start.x() - end.x(), start.y() - end.y()]
        } else {
            [end.x() - start.x(), end.y() - start.y()]
        };
        tangent_length = tangent[0].hypot(tangent[1]);
    }
    if !tangent_length.is_finite() || tangent_length == 0.0 {
        return invalid("a surface cutting arrangement edge has no endpoint tangent");
    }
    tangent = tangent.map(|coordinate| coordinate / tangent_length);
    let angle = tangent[1].atan2(tangent[0]);
    let sample_span = if reversed {
        edge.parameter.spans().last()
    } else {
        edge.parameter.spans().next()
    }
    .ok_or(GeometryError::InvalidBrepTopology {
        context: "a surface cutting arrangement edge has no active parameter span",
    })?;
    let sample_parameter = normalized_span_parameter(
        [sample_span.0, sample_span.1],
        if reversed {
            1.0 - BEND_SAMPLE_FRACTION
        } else {
            BEND_SAMPLE_FRACTION
        },
    )?;
    let (_, sample_derivative) = edge.parameter.evaluate_with_derivative(sample_parameter)?;
    let mut sample_tangent = if reversed {
        [-sample_derivative[0], -sample_derivative[1]]
    } else {
        sample_derivative
    };
    let sample_speed = sample_tangent[0].hypot(sample_tangent[1]);
    let bend = if regular_endpoint_tangent && sample_speed.is_finite() && sample_speed > 0.0 {
        sample_tangent = sample_tangent.map(|coordinate| coordinate / sample_speed);
        let arc_step = (sample_parameter - parameter).abs() * (tangent_length + sample_speed) * 0.5;
        if arc_step.is_finite() && arc_step > 0.0 {
            tangent[0].mul_add(sample_tangent[1], -tangent[1] * sample_tangent[0]) / arc_step
        } else {
            0.0
        }
    } else {
        0.0
    };
    require_finite(
        tangent.into_iter().chain([angle, bend]),
        "surface cutting arrangement halfedge direction",
    )?;
    Ok(SurfaceCutHalfedgeDirection {
        tangent,
        angle,
        bend,
    })
}

fn sort_surface_cut_outgoing_halfedges(
    halfedges: &mut [usize],
    directions: &[SurfaceCutHalfedgeDirection],
    angular_tolerance: Real,
) {
    halfedges.sort_by(|left, right| {
        directions[*left]
            .angle
            .total_cmp(&directions[*right].angle)
            .then_with(|| left.cmp(right))
    });
    if halfedges.len() < 2 {
        return;
    }

    // Linearize the circular order across its largest empty angular interval.
    // This keeps tangent directions straddling -pi/pi next to one another. A
    // cyclic rotation does not change the clockwise successor relation below.
    let mut largest_gap_after = 0_usize;
    let mut largest_gap = Real::NEG_INFINITY;
    for index in 0..halfedges.len() {
        let angle = directions[halfedges[index]].angle;
        let next_angle = directions[halfedges[(index + 1) % halfedges.len()]].angle;
        let gap = if index + 1 == halfedges.len() {
            next_angle + std::f64::consts::TAU - angle
        } else {
            next_angle - angle
        };
        if gap.total_cmp(&largest_gap).is_gt() {
            largest_gap = gap;
            largest_gap_after = index;
        }
    }
    halfedges.rotate_left((largest_gap_after + 1) % halfedges.len());

    // Curves with the same first-order direction are ordered by their signed
    // bend, which is their infinitesimal angular order away from the vertex.
    // Clustering before sorting keeps the comparison itself a total order.
    let parallel_tolerance = angular_tolerance.clamp(Real::EPSILON * 256.0, 1.0e-7);
    let mut group_start = 0_usize;
    while group_start < halfedges.len() {
        let first = directions[halfedges[group_start]];
        let mut group_end = group_start + 1;
        while group_end < halfedges.len() {
            let candidate = directions[halfedges[group_end]];
            let cross = first.tangent[0].mul_add(
                candidate.tangent[1],
                -first.tangent[1] * candidate.tangent[0],
            );
            let dot = first.tangent[0].mul_add(
                candidate.tangent[0],
                first.tangent[1] * candidate.tangent[1],
            );
            if dot <= 0.0 || cross.abs() > parallel_tolerance {
                break;
            }
            group_end += 1;
        }
        halfedges[group_start..group_end].sort_by(|left, right| {
            directions[*left]
                .bend
                .total_cmp(&directions[*right].bend)
                .then_with(|| directions[*left].angle.total_cmp(&directions[*right].angle))
                .then_with(|| left.cmp(right))
        });
        group_start = group_end;
    }
}

fn sampled_surface_cut_cycle_signed_area(
    edges: &[SurfaceCutArrangementEdge],
    cycle: &[usize],
) -> Result<Option<Real>, GeometryError> {
    let points = sampled_surface_cut_cycle_points(edges, cycle)?;
    let Some(origin) = points.first().copied() else {
        return Ok(None);
    };
    let relative = points
        .iter()
        .map(|point| [point.x() - origin.x(), point.y() - origin.y()])
        .collect::<Vec<_>>();
    require_finite(
        relative.iter().flatten().copied(),
        "surface cutting arrangement p-loop coordinates",
    )?;
    let scale = relative
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    let mut sum = 0.0;
    let mut correction = 0.0;
    for index in 0..relative.len() {
        let first = relative[index].map(|value| value / scale);
        let second = relative[(index + 1) % relative.len()].map(|value| value / scale);
        neumaier_add(
            &mut sum,
            &mut correction,
            first[0].mul_add(second[1], -first[1] * second[0]),
        );
    }
    let doubled_area = sum + correction;
    require_finite([doubled_area], "surface cutting arrangement signed area")?;
    Ok((doubled_area.abs() > 1.0e-14).then_some(doubled_area))
}

fn sampled_surface_cut_cycle_points(
    edges: &[SurfaceCutArrangementEdge],
    cycle: &[usize],
) -> Result<Vec<Point2>, GeometryError> {
    let mut points = Vec::new();
    for &halfedge in cycle {
        let edge = &edges[halfedge / 2];
        let curve = if halfedge.is_multiple_of(2) {
            edge.parameter.clone()
        } else {
            edge.parameter.reversed()?
        };
        if points.is_empty() {
            points.push(curve.start_point()?);
        }
        for (start, end) in curve.spans() {
            for sample in 1..=LOOP_SAMPLES_PER_SPAN {
                let fraction = sample as Real / LOOP_SAMPLES_PER_SPAN as Real;
                let parameter = start.mul_add(1.0 - fraction, end * fraction);
                points.push(curve.evaluate(parameter)?);
            }
        }
    }
    Ok(points)
}

fn surface_cut_cycle_bounds(
    edges: &[SurfaceCutArrangementEdge],
    nodes: &[SurfaceCutArrangementNode],
    cycle: &[usize],
) -> [Real; 4] {
    let mut bounds = [
        Real::INFINITY,
        Real::INFINITY,
        Real::NEG_INFINITY,
        Real::NEG_INFINITY,
    ];
    for &halfedge in cycle {
        let node = edges[halfedge / 2].nodes[usize::from(!halfedge.is_multiple_of(2))];
        let parameter = nodes[node].parameter;
        bounds[0] = bounds[0].min(parameter.x());
        bounds[1] = bounds[1].min(parameter.y());
        bounds[2] = bounds[2].max(parameter.x());
        bounds[3] = bounds[3].max(parameter.y());
    }
    bounds
}

fn surface_cut_cycle_source_key(edges: &[SurfaceCutArrangementEdge], cycle: &[usize]) -> usize {
    cycle
        .iter()
        .filter_map(|halfedge| match edges[*halfedge / 2].kind {
            SurfaceCutArrangementEdgeKind::Boundary(_) => None,
            SurfaceCutArrangementEdgeKind::Cut { source, .. } => Some(source),
        })
        .min()
        .unwrap_or(usize::MAX)
}

fn surface_cut_cycle_single_source(
    edges: &[SurfaceCutArrangementEdge],
    cycle: &[usize],
) -> Option<(usize, usize)> {
    let mut source = None;
    let mut maximum_segment = 0;
    for halfedge in cycle {
        let SurfaceCutArrangementEdgeKind::Cut {
            source: edge_source,
            segment,
        } = edges[*halfedge / 2].kind
        else {
            return None;
        };
        if source.is_some_and(|source| source != edge_source) {
            return None;
        }
        source = Some(edge_source);
        maximum_segment = maximum_segment.max(segment);
    }
    source.map(|source| (source, maximum_segment))
}

fn rotate_surface_cut_overlap_outer_cycle(
    cycle: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    nodes: &[SurfaceCutArrangementNode],
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
    source_info: &SurfaceCutArrangementSourceInfo,
) -> bool {
    let touches_marked_node = |edge: &SurfaceCutArrangementEdge, marked: &[bool]| {
        edge.nodes.into_iter().any(|node| marked[node])
    };
    if !cycle.iter().any(|halfedge| {
        touches_marked_node(&edges[*halfedge / 2], &source_info.first_overlap_nodes)
    }) {
        return false;
    }
    let opposite_direction = cycle.iter().any(|halfedge| {
        touches_marked_node(
            &edges[*halfedge / 2],
            &source_info.first_opposite_overlap_nodes,
        )
    });
    let touches_overlap = |edge: &SurfaceCutArrangementEdge| {
        touches_marked_node(edge, &source_info.first_overlap_nodes)
    };

    // A forward branch adjacent to the overlap retains the lowest source
    // segment. Oppositely directed coincident cutters compete by segment
    // number; otherwise the earliest cutter retains the seam.
    let has_forward_first_branch = cycle.iter().any(|halfedge| {
        halfedge.is_multiple_of(2)
            && matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
            )
            && edges[*halfedge / 2].coincidences.is_empty()
            && touches_overlap(&edges[*halfedge / 2])
    });
    if has_forward_first_branch
        && let Some((_, _, anchor)) = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                if !halfedge.is_multiple_of(2) {
                    return None;
                }
                let edge = &edges[*halfedge / 2];
                let SurfaceCutArrangementEdgeKind::Cut { source, segment } = edge.kind else {
                    return None;
                };
                (edge.coincidences.is_empty()
                    && touches_overlap(edge)
                    && (source == 0 || opposite_direction && !source_info.closed[source]))
                    .then_some((segment, source, index))
            })
            .min()
    {
        cycle.rotate_left(anchor);
        return true;
    }

    // Reverse first-cutter branches preserve the vertex following the lowest
    // reverse segment. Include the later cutter only when its overlap runs in
    // the opposite direction.
    let has_reverse_first_branch = cycle.iter().any(|halfedge| {
        !halfedge.is_multiple_of(2)
            && matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
            )
            && edges[*halfedge / 2].coincidences.is_empty()
            && touches_overlap(&edges[*halfedge / 2])
    });
    if has_reverse_first_branch
        && let Some((_, _, anchor)) = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                if halfedge.is_multiple_of(2) {
                    return None;
                }
                let edge = &edges[*halfedge / 2];
                let SurfaceCutArrangementEdgeKind::Cut { source, segment } = edge.kind else {
                    return None;
                };
                (touches_overlap(edge)
                    && (source == 0 || opposite_direction && !source_info.closed[source]))
                    .then_some((
                        segment,
                        source,
                        if source != 0 && opposite_direction && edge.smooth_continuations > 0 {
                            index
                        } else {
                            (index + 1) % cycle.len()
                        },
                    ))
            })
            .min()
    {
        cycle.rotate_left(anchor);
        return true;
    }

    let edge_touches_boundary = |edge: &SurfaceCutArrangementEdge| {
        edge.nodes.into_iter().any(|node| {
            [
                RectangularBoundarySide::South,
                RectangularBoundarySide::East,
                RectangularBoundarySide::North,
                RectangularBoundarySide::West,
            ]
            .into_iter()
            .any(|side| {
                surface_cut_parameter_on_side(nodes[node].parameter, side, bounds, tolerance)
            })
        })
    };

    // If this face contains only the shared first-cutter edge, a reverse edge
    // with matching source directions is itself the seam. For a reverse cutter
    // whose overlap begins at the rectangle boundary, Rhino instead preserves
    // the vertex following the later cutter's reverse branch.
    for (shared_index, &halfedge) in cycle.iter().enumerate() {
        let edge = &edges[halfedge / 2];
        let Some(first_contributor) = surface_cut_arrangement_edge_contributor(edge, 0) else {
            continue;
        };
        if edge.coincidences.is_empty()
            || surface_cut_halfedge_follows_source(edge, halfedge, 0) != Some(false)
        {
            continue;
        }
        let matching_contributors = surface_cut_arrangement_edge_contributors(edge)
            .filter(|contributor| {
                contributor.source != 0
                    && contributor.same_direction == first_contributor.same_direction
                    && !source_info.closed[contributor.source]
            })
            .collect::<Vec<_>>();
        if edge_touches_boundary(edge) && first_contributor.segment == 0 {
            cycle.rotate_left(shared_index);
            return true;
        }
        for contributor in matching_contributors {
            if !edge_touches_boundary(edge) {
                cycle.rotate_left(shared_index);
                return true;
            }
            if let Some((_, anchor)) = cycle
                .iter()
                .enumerate()
                .filter_map(|(index, candidate)| {
                    if candidate.is_multiple_of(2) {
                        return None;
                    }
                    let candidate_edge = &edges[*candidate / 2];
                    match candidate_edge.kind {
                        SurfaceCutArrangementEdgeKind::Cut { source, segment }
                            if source == contributor.source && touches_overlap(candidate_edge) =>
                        {
                            Some((segment, (index + 1) % cycle.len()))
                        }
                        _ => None,
                    }
                })
                .min()
            {
                cycle.rotate_left(anchor);
                return true;
            }
        }
    }

    // A partial interior overlap is represented by the earliest cutter's
    // edge. When this face follows that sole first-cutter edge in a discarded
    // later cutter's forward direction, Rhino places the seam on the later
    // cutter's post-overlap branch. An overlap beginning on the rectangle
    // boundary instead retains the ordinary boundary-derived seam.
    if cycle
        .iter()
        .filter(|halfedge| {
            surface_cut_arrangement_edge_contributor(&edges[**halfedge / 2], 0).is_some()
        })
        .count()
        == 1
    {
        let mut overlap_anchor = None;
        for &halfedge in cycle.iter() {
            let edge = &edges[halfedge / 2];
            if surface_cut_arrangement_edge_contributor(edge, 0).is_none()
                || edge.coincidences.is_empty()
                || edge_touches_boundary(edge)
            {
                continue;
            }
            for contributor in
                surface_cut_arrangement_edge_contributors(edge).filter(|contributor| {
                    contributor.source != 0
                        && surface_cut_halfedge_follows_source(edge, halfedge, contributor.source)
                            == Some(true)
                        && !source_info.closed[contributor.source]
                })
            {
                for (index, &candidate) in cycle.iter().enumerate() {
                    if !candidate.is_multiple_of(2) {
                        continue;
                    }
                    let SurfaceCutArrangementEdgeKind::Cut { source, segment } =
                        edges[candidate / 2].kind
                    else {
                        continue;
                    };
                    if source != contributor.source {
                        continue;
                    }
                    let key = (source, segment, index);
                    if overlap_anchor.is_none_or(|existing| key > existing) {
                        overlap_anchor = Some(key);
                    }
                }
            }
        }
        if let Some((_, _, anchor)) = overlap_anchor {
            cycle.rotate_left(anchor);
            return true;
        }
    }
    false
}

fn rotate_surface_cut_open_closed_overlap_outer_cycle(
    cycle: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    source_info: &SurfaceCutArrangementSourceInfo,
) -> bool {
    // The open representative belongs to both adjacent faces, while the
    // closed source's remaining edges occur only on its own side. Rhino keeps
    // the representative seam on the other side and advances around the
    // closed loop on the enclosed side.
    let Some(closed_source) = source_info.first_closed else {
        return false;
    };
    let shared = cycle.iter().enumerate().find_map(|(index, halfedge)| {
        let edge = &edges[*halfedge / 2];
        let SurfaceCutArrangementEdgeKind::Cut { source, .. } = edge.kind else {
            return None;
        };
        (!source_info.closed[source]
            && edge
                .coincidences
                .iter()
                .any(|coincidence| coincidence.source == closed_source))
        .then_some((index, *halfedge))
    });
    let Some((shared_index, shared_halfedge)) = shared else {
        return false;
    };

    if closed_source == 0 {
        let has_closed_edge = cycle.iter().any(|halfedge| {
            matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
            )
        });
        if let Some((_, anchor)) = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                if !halfedge.is_multiple_of(2) {
                    return None;
                }
                match edges[*halfedge / 2].kind {
                    SurfaceCutArrangementEdgeKind::Cut { source: 0, segment } => {
                        Some((segment, index))
                    }
                    _ => None,
                }
            })
            .min()
        {
            cycle.rotate_left(anchor);
        } else if !has_closed_edge && shared_halfedge.is_multiple_of(2) {
            cycle.rotate_left((shared_index + 1) % cycle.len());
        } else {
            cycle.rotate_left(shared_index);
        }
        return true;
    }

    let forward_anchor = cycle
        .iter()
        .enumerate()
        .filter_map(|(index, halfedge)| {
            if !halfedge.is_multiple_of(2) {
                return None;
            }
            match edges[*halfedge / 2].kind {
                SurfaceCutArrangementEdgeKind::Cut { source, segment }
                    if source == closed_source =>
                {
                    Some((segment, index))
                }
                _ => None,
            }
        })
        .max();
    if let Some((_, anchor)) = forward_anchor {
        cycle.rotate_left(anchor);
        return true;
    }
    // Reversed closed loops retain the segment immediately before their
    // ordinary maximum-segment seam.
    let maximum_reverse_segment = cycle.iter().filter_map(|halfedge| {
        if halfedge.is_multiple_of(2) {
            return None;
        }
        match edges[*halfedge / 2].kind {
            SurfaceCutArrangementEdgeKind::Cut { source, segment } if source == closed_source => {
                Some(segment)
            }
            _ => None,
        }
    });
    if let Some(target_segment) = maximum_reverse_segment
        .max()
        .map(|segment| segment.saturating_sub(1))
        && let Some(anchor) = cycle.iter().position(|halfedge| {
            !halfedge.is_multiple_of(2)
                && matches!(
                    edges[*halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut { source, segment }
                        if source == closed_source && segment == target_segment
                )
        })
    {
        cycle.rotate_left(anchor);
        return true;
    }
    false
}

fn rotate_surface_cut_outer_cycle(
    cycle: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    nodes: &[SurfaceCutArrangementNode],
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
    source_info: &SurfaceCutArrangementSourceInfo,
) -> Result<(), GeometryError> {
    let first_closed_source = source_info.first_closed;
    let closed_sources = &source_info.closed;
    let first_source_nodes = &source_info.first_nodes;
    let first_tangent_nodes = &source_info.first_tangent_nodes;
    if let Some((source, maximum_segment)) = surface_cut_cycle_single_source(edges, cycle)
        && first_closed_source.is_some()
        && closed_sources.get(source).copied().unwrap_or(false)
    {
        let first_cutter = source == 0;
        let forward = first_cutter || cycle.iter().any(|halfedge| halfedge.is_multiple_of(2));
        let target_segment = if first_cutter || !forward {
            0
        } else {
            maximum_segment
        };
        if let Some(anchor) = cycle.iter().position(|halfedge| {
            halfedge.is_multiple_of(2) == forward
                && matches!(
                    edges[*halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut {
                        source: edge_source,
                        segment,
                    } if edge_source == source && segment == target_segment
                )
        }) {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }

    if !closed_sources[0]
        && rotate_surface_cut_overlap_outer_cycle(
            cycle,
            edges,
            nodes,
            bounds,
            tolerance,
            source_info,
        )
    {
        return Ok(());
    }

    if rotate_surface_cut_open_closed_overlap_outer_cycle(cycle, edges, source_info) {
        return Ok(());
    }

    if let Some(first_closed_source) = first_closed_source
        // Intersecting loops start at the last forward segment from a later
        // cutter. At a one-point contact, a complete self-edge remains and
        // Rhino instead retains the earliest cutter's seam.
        && !cycle.iter().any(|halfedge| {
            let edge = &edges[*halfedge / 2];
            edge.nodes[0] == edge.nodes[1]
        })
        && let Some((_, _, anchor)) = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                if !halfedge.is_multiple_of(2) {
                    return None;
                }
                let SurfaceCutArrangementEdgeKind::Cut { source, segment } =
                    edges[*halfedge / 2].kind
                else {
                    return None;
                };
                (source != first_closed_source
                    && closed_sources.get(source).copied().unwrap_or(false))
                .then_some((source, segment, index))
            })
            .max_by_key(|(source, segment, _)| (*source, *segment))
    {
        cycle.rotate_left(anchor);
        return Ok(());
    }

    // Faces incident to an open/open tangency retain the first cutter's seam:
    // its earliest forward segment, or the vertex after that segment when the
    // face traverses the cutter backward.
    if first_closed_source.is_none()
        && source_info.first_maximum_segment > 0
        && cycle.iter().any(|halfedge| {
            edges[*halfedge / 2]
                .nodes
                .into_iter()
                .any(|node| first_tangent_nodes[node])
        })
    {
        let forward_anchor = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                if !halfedge.is_multiple_of(2) {
                    return None;
                }
                match edges[*halfedge / 2].kind {
                    SurfaceCutArrangementEdgeKind::Cut { source: 0, segment } => {
                        Some((segment, index))
                    }
                    _ => None,
                }
            })
            .min_by_key(|(segment, _)| *segment)
            .map(|(_, anchor)| anchor);
        let anchor = forward_anchor.or_else(|| {
            cycle
                .iter()
                .enumerate()
                .filter_map(|(index, halfedge)| {
                    if halfedge.is_multiple_of(2) {
                        return None;
                    }
                    match edges[*halfedge / 2].kind {
                        SurfaceCutArrangementEdgeKind::Cut { source: 0, segment } => {
                            Some((segment, (index + 1) % cycle.len()))
                        }
                        _ => None,
                    }
                })
                .min_by_key(|(segment, _)| *segment)
                .map(|(_, anchor)| anchor)
        });
        if let Some(anchor) = anchor {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }

    // A face on the far side of a one-point contact can omit every edge from
    // the first cutter while still sharing its contact node. Closed first
    // cutters use this rule at any shared node; an open first cutter uses it
    // only at an interior tangency, not a transverse crossing or shared corner.
    if !cycle.iter().any(|halfedge| {
        matches!(
            edges[*halfedge / 2].kind,
            SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
        )
    }) && cycle.iter().any(|halfedge| {
        edges[*halfedge / 2].nodes.into_iter().any(|node| {
            if first_closed_source == Some(0) {
                return first_source_nodes[node];
            }
            first_tangent_nodes[node]
                && [
                    RectangularBoundarySide::South,
                    RectangularBoundarySide::East,
                    RectangularBoundarySide::North,
                    RectangularBoundarySide::West,
                ]
                .into_iter()
                .all(|side| {
                    !surface_cut_parameter_on_side(nodes[node].parameter, side, bounds, tolerance)
                })
        })
    }) && let Some(source) = cycle
        .iter()
        .filter_map(|halfedge| match edges[*halfedge / 2].kind {
            SurfaceCutArrangementEdgeKind::Cut { source, .. } => Some(source),
            SurfaceCutArrangementEdgeKind::Boundary(_) => None,
        })
        .max()
    {
        let forward = cycle.iter().any(|halfedge| {
            halfedge.is_multiple_of(2)
                && matches!(
                    edges[*halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut {
                        source: edge_source,
                        ..
                    } if edge_source == source
                )
        });
        let anchor = cycle.iter().enumerate().filter_map(|(index, halfedge)| {
            if halfedge.is_multiple_of(2) != forward {
                return None;
            }
            match edges[*halfedge / 2].kind {
                SurfaceCutArrangementEdgeKind::Cut {
                    source: edge_source,
                    segment,
                } if edge_source == source => Some((segment, index)),
                _ => None,
            }
        });
        let anchor = if forward {
            anchor.max_by_key(|(segment, _)| *segment)
        } else {
            anchor.min_by_key(|(segment, _)| *segment)
        };
        if let Some((_, anchor)) = anchor {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }

    let closed_source = cycle
        .iter()
        .filter_map(|halfedge| match edges[*halfedge / 2].kind {
            SurfaceCutArrangementEdgeKind::Cut { source, .. }
                if closed_sources.get(source).copied().unwrap_or(false) =>
            {
                Some(source)
            }
            _ => None,
        })
        .min();
    if closed_source == Some(0) {
        let source = 0;
        let seam_node = edges.iter().find_map(|edge| match edge.kind {
            SurfaceCutArrangementEdgeKind::Cut {
                source: edge_source,
                segment: 0,
            } if edge_source == source => Some(edge.nodes[0]),
            _ => None,
        });
        if let Some(seam_node) = seam_node
            && let Some(anchor) = cycle.iter().position(|halfedge| {
                surface_cut_halfedge_nodes(&edges[*halfedge / 2], *halfedge)[0] == seam_node
            })
        {
            cycle.rotate_left(anchor);
            return Ok(());
        }

        let anchor = cycle
            .iter()
            .enumerate()
            .filter_map(|(index, halfedge)| {
                let SurfaceCutArrangementEdgeKind::Cut {
                    source: edge_source,
                    segment,
                } = edges[*halfedge / 2].kind
                else {
                    return None;
                };
                if edge_source != source {
                    return None;
                }
                let next_edge = &edges[cycle[(index + 1) % cycle.len()] / 2];
                (!matches!(
                    next_edge.kind,
                    SurfaceCutArrangementEdgeKind::Cut {
                        source: next_source,
                        ..
                    } if next_source == source
                ))
                .then_some((segment, (index + 1) % cycle.len()))
            })
            .min_by_key(|(segment, _)| *segment)
            .map(|(_, anchor)| anchor);
        if let Some(anchor) = anchor {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }
    if first_closed_source.is_some()
        && cycle.iter().all(|halfedge| {
            matches!(
                edges[*halfedge / 2].kind,
                SurfaceCutArrangementEdgeKind::Boundary(_)
            )
        })
        && let Some(anchor) = cycle.iter().position(|halfedge| {
            halfedge.is_multiple_of(2)
                && matches!(
                    edges[*halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Boundary(RectangularBoundarySide::South)
                )
        })
    {
        cycle.rotate_left(anchor);
        return Ok(());
    }
    rotate_surface_cut_cycle(cycle, edges, nodes, bounds, tolerance)
}

fn rotate_surface_cut_hole_cycle(
    cycle: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    nodes: &[SurfaceCutArrangementNode],
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
    source_info: &SurfaceCutArrangementSourceInfo,
) -> Result<(), GeometryError> {
    let first_closed_source = source_info.first_closed;
    let closed_sources = &source_info.closed;
    if let Some((source, maximum_segment)) = surface_cut_cycle_single_source(edges, cycle)
        && first_closed_source.is_some()
    {
        let target_segment = if source == 0 {
            maximum_segment
        } else {
            maximum_segment.saturating_sub(1)
        };
        if let Some(anchor) = cycle.iter().position(|halfedge| {
            !halfedge.is_multiple_of(2)
                && matches!(
                    edges[*halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut {
                        source: edge_source,
                        segment,
                    } if edge_source == source && segment == target_segment
                )
        }) {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }
    for (source, &closed) in closed_sources.iter().enumerate() {
        if !closed {
            continue;
        }
        let seam_node = edges.iter().find_map(|edge| match edge.kind {
            SurfaceCutArrangementEdgeKind::Cut {
                source: edge_source,
                segment: 0,
            } if edge_source == source => Some(edge.nodes[0]),
            _ => None,
        });
        if let Some(seam_node) = seam_node
            && let Some(anchor) = cycle.iter().position(|halfedge| {
                surface_cut_halfedge_nodes(&edges[*halfedge / 2], *halfedge)[0] == seam_node
            })
        {
            cycle.rotate_left(anchor);
            return Ok(());
        }
    }
    rotate_surface_cut_cycle(cycle, edges, nodes, bounds, tolerance)
}

fn surface_cut_hole_parent(
    edges: &[SurfaceCutArrangementEdge],
    outer_cycles: &[Vec<usize>],
    hole: &[usize],
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
) -> Result<Option<usize>, GeometryError> {
    let scale = (bounds[0][1] - bounds[0][0]).max(bounds[1][1] - bounds[1][0]);
    if !scale.is_finite() || scale <= 0.0 {
        return invalid("surface cutting arrangement bounds have no finite extent");
    }
    let normalize = |point: Point2| {
        [
            (point.x() - bounds[0][0]) / scale,
            (point.y() - bounds[1][0]) / scale,
        ]
    };
    let outer_polygons = outer_cycles
        .iter()
        .map(|cycle| {
            sampled_surface_cut_cycle_points(edges, cycle).map(|points| {
                points
                    .into_iter()
                    .map(&normalize)
                    .collect::<Vec<[Real; 2]>>()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outer_areas = outer_polygons
        .iter()
        .map(|polygon| {
            polygon
                .iter()
                .copied()
                .zip(polygon.iter().copied().cycle().skip(1))
                .take(polygon.len())
                .map(|(start, end)| start[0] * end[1] - end[0] * start[1])
                .sum::<Real>()
                .abs()
        })
        .collect::<Vec<_>>();
    let epsilon = (tolerance[0].max(tolerance[1]) / scale * 16.0).max(Real::EPSILON * 256.0);
    let base_step = (epsilon * 32.0).max(1.0e-7);

    for &halfedge in hole {
        let edge = &edges[halfedge / 2];
        let reversed = !halfedge.is_multiple_of(2);
        let domain = edge.parameter.domain();
        for fraction in [0.5, 0.25, 0.75] {
            let parameter = domain
                .start()
                .mul_add(1.0 - fraction, domain.end() * fraction);
            let (point, derivative) = edge.parameter.evaluate_with_derivative(parameter)?;
            let tangent = if reversed {
                [-derivative[0], -derivative[1]]
            } else {
                derivative
            };
            let tangent_length = tangent[0].hypot(tangent[1]);
            if !tangent_length.is_finite() || tangent_length == 0.0 {
                continue;
            }
            let point = normalize(point);
            let left = [-tangent[1] / tangent_length, tangent[0] / tangent_length];
            for step in [base_step, base_step * 0.25, base_step * 4.0] {
                let candidate = [point[0] + left[0] * step, point[1] + left[1] * step];
                if outer_polygons
                    .iter()
                    .any(|polygon| point_on_trim_polygon(candidate, polygon, epsilon))
                {
                    continue;
                }
                let parent = outer_polygons
                    .iter()
                    .enumerate()
                    .filter(|(_, polygon)| point_in_trim_polygon(candidate, polygon, epsilon))
                    .min_by(|(left, _), (right, _)| {
                        outer_areas[*left].total_cmp(&outer_areas[*right])
                    })
                    .map(|(index, _)| index);
                if parent.is_some() {
                    return Ok(parent);
                }
            }
        }
    }
    Ok(None)
}

fn rotate_surface_cut_cycle(
    cycle: &mut [usize],
    edges: &[SurfaceCutArrangementEdge],
    nodes: &[SurfaceCutArrangementNode],
    bounds: [[Real; 2]; 2],
    tolerance: [Real; 2],
) -> Result<(), GeometryError> {
    let face_nodes = cycle
        .iter()
        .map(|halfedge| surface_cut_halfedge_nodes(&edges[*halfedge / 2], *halfedge)[0])
        .collect::<Vec<_>>();
    let minimum_u = face_nodes
        .iter()
        .map(|node| nodes[*node].parameter.x())
        .fold(Real::INFINITY, Real::min);
    let lower_left = face_nodes
        .iter()
        .copied()
        .filter(|node| nodes[*node].parameter.x() <= minimum_u + tolerance[0])
        .min_by(|left, right| {
            nodes[*left]
                .parameter
                .y()
                .total_cmp(&nodes[*right].parameter.y())
                .then_with(|| left.cmp(right))
        })
        .ok_or(GeometryError::InvalidBrepTopology {
            context: "a surface cutting arrangement face has no vertices",
        })?;
    let south_west = (nodes[lower_left].parameter.x() - bounds[0][0]).abs() <= tolerance[0]
        && (nodes[lower_left].parameter.y() - bounds[1][0]).abs() <= tolerance[1];
    let contains_south_boundary = cycle.iter().any(|halfedge| {
        matches!(
            edges[*halfedge / 2].kind,
            SurfaceCutArrangementEdgeKind::Boundary(RectangularBoundarySide::South)
        )
    });
    let same_side_cut = cycle.iter().position(|halfedge| {
        let edge = &edges[*halfedge / 2];
        matches!(edge.kind, SurfaceCutArrangementEdgeKind::Cut { .. })
            && [
                RectangularBoundarySide::South,
                RectangularBoundarySide::East,
                RectangularBoundarySide::North,
                RectangularBoundarySide::West,
            ]
            .into_iter()
            .any(|side| {
                edge.nodes.iter().all(|node| {
                    surface_cut_parameter_on_side(nodes[*node].parameter, side, bounds, tolerance)
                })
            })
    });
    let anchor = if let Some(index) = same_side_cut {
        if cycle[index].is_multiple_of(2) {
            Some(index)
        } else {
            Some((index + 1) % cycle.len())
        }
    } else if south_west && contains_south_boundary {
        cycle.iter().position(|halfedge| {
            let edge = &edges[*halfedge / 2];
            matches!(
                edge.kind,
                SurfaceCutArrangementEdgeKind::Boundary(RectangularBoundarySide::West)
            ) && surface_cut_halfedge_nodes(edge, *halfedge)[1] == lower_left
        })
    } else {
        cycle
            .iter()
            .position(|halfedge| {
                surface_cut_halfedge_nodes(&edges[*halfedge / 2], *halfedge)[0] == lower_left
            })
            .map(|index| {
                let halfedge = cycle[index];
                // Rhino starts after a lower boundary cutter when that
                // boundary is traversed opposite to its source direction.
                if matches!(
                    edges[halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut { .. }
                ) && !halfedge.is_multiple_of(2)
                {
                    (index + 1) % cycle.len()
                } else {
                    index
                }
            })
    }
    .unwrap_or(0);
    cycle.rotate_left(anchor);
    Ok(())
}

fn surface_cut_halfedge_nodes(edge: &SurfaceCutArrangementEdge, halfedge: usize) -> [usize; 2] {
    if halfedge.is_multiple_of(2) {
        edge.nodes
    } else {
        [edge.nodes[1], edge.nodes[0]]
    }
}

fn try_surface_cut_arrangement_face(
    surface: NurbsSurface,
    reversed: bool,
    nodes: &[SurfaceCutArrangementNode],
    edges: &[SurfaceCutArrangementEdge],
    region: &SurfaceCutArrangementRegion,
    source_info: &SurfaceCutArrangementSourceInfo,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let node_rank = &source_info.node_rank;
    let cycles = std::iter::once(region.outer.as_slice())
        .chain(region.holes.iter().map(Vec::as_slice))
        .collect::<Vec<_>>();
    // When an oppositely directed smooth contributor displaces the first
    // cutter at a boundary overlap, Rhino walks non-corner vertices backward
    // from the promoted seam even though the trim loop itself remains forward.
    let reverse_promoted_overlap_vertex_order = region.outer.first().is_some_and(|halfedge| {
        let edge = &edges[*halfedge / 2];
        source_info.first_boundary_overlap_edges[*halfedge / 2]
            && edge.smooth_continuations > 0
            && surface_cut_arrangement_edge_contributor(edge, 0)
                .is_some_and(|contributor| !contributor.same_direction)
            && surface_cut_halfedge_follows_source(edge, *halfedge, 0) == Some(false)
            && matches!(
                edge.kind,
                SurfaceCutArrangementEdgeKind::Cut { source, .. }
                    if source != 0
                        && !source_info.closed[0]
                        && !source_info.closed[source]
            )
    });
    let overlap_cycle_order = region.outer.first().is_some_and(|first_halfedge| {
        let first_edge = &edges[*first_halfedge / 2];
        region
            .outer
            .iter()
            .any(|halfedge| source_info.first_boundary_overlap_edges[*halfedge / 2])
            && ((!first_halfedge.is_multiple_of(2)
                && source_info.first_boundary_overlap_edges[*first_halfedge / 2])
                || first_halfedge.is_multiple_of(2)
                    && first_edge.coincidences.is_empty()
                    && matches!(
                        first_edge.kind,
                        SurfaceCutArrangementEdgeKind::Cut { source: 0, .. }
                    )
                    && first_edge
                        .nodes
                        .into_iter()
                        .any(|node| source_info.first_overlap_nodes[node]))
    });
    let mut face_nodes = cycles
        .iter()
        .flat_map(|cycle| cycle.iter())
        .flat_map(|halfedge| edges[*halfedge / 2].nodes)
        .collect::<Vec<_>>();
    if reverse_promoted_overlap_vertex_order {
        let present = face_nodes.iter().copied().collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(present.len());
        let mut included = BTreeSet::new();
        for node in 0..nodes.len().min(4) {
            if present.contains(&node) && included.insert(node) {
                ordered.push(node);
            }
        }
        let first = region.outer[0];
        let first_node = surface_cut_halfedge_nodes(&edges[first / 2], first)[0];
        if included.insert(first_node) {
            ordered.push(first_node);
        }
        for &halfedge in region.outer.iter().skip(1).rev() {
            let node = surface_cut_halfedge_nodes(&edges[halfedge / 2], halfedge)[0];
            if included.insert(node) {
                ordered.push(node);
            }
        }
        face_nodes.sort_by_key(|node| node_rank[*node]);
        for node in face_nodes {
            if included.insert(node) {
                ordered.push(node);
            }
        }
        face_nodes = ordered;
    } else if overlap_cycle_order {
        let present = face_nodes.iter().copied().collect::<BTreeSet<_>>();
        let mut ordered = Vec::with_capacity(present.len());
        let mut included = BTreeSet::new();
        for node in 0..nodes.len().min(4) {
            if present.contains(&node) && included.insert(node) {
                ordered.push(node);
            }
        }
        for cycle in &cycles {
            for &halfedge in *cycle {
                if !matches!(
                    edges[halfedge / 2].kind,
                    SurfaceCutArrangementEdgeKind::Cut { .. }
                ) {
                    continue;
                }
                for node in surface_cut_halfedge_nodes(&edges[halfedge / 2], halfedge) {
                    if included.insert(node) {
                        ordered.push(node);
                    }
                }
            }
        }
        face_nodes.sort_by_key(|node| node_rank[*node]);
        for node in face_nodes {
            if included.insert(node) {
                ordered.push(node);
            }
        }
        face_nodes = ordered;
    } else {
        face_nodes.sort_by_key(|node| node_rank[*node]);
        face_nodes.dedup();
    }
    let mut vertex_map = BTreeMap::<usize, usize>::new();
    let vertices = face_nodes
        .into_iter()
        .enumerate()
        .map(|(vertex, node)| {
            vertex_map.insert(node, vertex);
            BrepVertex::try_new(nodes[node].point, 0.0)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut ordered_halfedges = cycles
        .iter()
        .flat_map(|cycle| cycle.iter().copied())
        .collect::<Vec<_>>();
    let cycle_edge_order = ordered_halfedges
        .iter()
        .enumerate()
        .map(|(order, halfedge)| (*halfedge / 2, order))
        .collect::<BTreeMap<_, _>>();
    ordered_halfedges.sort_by_key(|halfedge| {
        let edge = &edges[*halfedge / 2];
        match edge.kind {
            SurfaceCutArrangementEdgeKind::Boundary(side) => {
                (if edge.nodes[0] < 4 { 0 } else { 2 }, side.index(), 0)
            }
            SurfaceCutArrangementEdgeKind::Cut { .. } if overlap_cycle_order => {
                (1, cycle_edge_order[&(*halfedge / 2)], 0)
            }
            SurfaceCutArrangementEdgeKind::Cut { source, segment } => (1, source, segment),
        }
    });
    ordered_halfedges.dedup_by_key(|halfedge| *halfedge / 2);
    let edge_map = ordered_halfedges
        .iter()
        .enumerate()
        .map(|(local, halfedge)| (*halfedge / 2, local))
        .collect::<BTreeMap<_, _>>();
    let mut face_edges = Vec::<BrepEdge>::with_capacity(ordered_halfedges.len());
    for &halfedge in &ordered_halfedges {
        let edge = &edges[halfedge / 2];
        let edge_vertices = edge.nodes.map(|node| vertex_map[&node]);
        face_edges.push(BrepEdge::try_new(edge_vertices, edge.spatial.clone(), 0.0)?);
    }
    let mut loops = Vec::with_capacity(cycles.len());
    for (index, cycle) in cycles.into_iter().enumerate() {
        let mut trims = Vec::<BrepTrim>::with_capacity(cycle.len());
        for &halfedge in cycle {
            let edge = &edges[halfedge / 2];
            let edge_index = edge_map[&(halfedge / 2)];
            let reversed_3d = !halfedge.is_multiple_of(2);
            let trim_vertices = oriented_edge_vertices(&face_edges[edge_index], reversed_3d);
            let parameter_curve = if reversed_3d {
                edge.parameter.reversed()?
            } else {
                edge.parameter.clone()
            };
            trims.push(BrepTrim::try_new(
                trim_vertices,
                Some(edge_index),
                reversed_3d,
                parameter_curve,
                BrepTrimType::Boundary,
                edge.iso,
                [0.0, 0.0],
            )?);
        }
        loops.push(BrepLoop::try_new(
            if index == 0 {
                BrepLoopType::Outer
            } else {
                BrepLoopType::Inner
            },
            trims,
        )?);
    }
    let face = BrepFace::try_new(surface, reversed, loops)?;
    Brep::try_new(vertices, face_edges, vec![face], tolerance)
}

fn orient_surface_split_curve(
    curve: &NurbsCurve,
    start: Point3,
    end: Point3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let domain = curve.domain();
    let curve_start = curve.evaluate(*domain.start())?;
    let curve_end = curve.evaluate(*domain.end())?;
    if curve_start.is_near(start, tolerance) && curve_end.is_near(end, tolerance) {
        return Ok(curve.clone());
    }
    if curve_start.is_near(end, tolerance) && curve_end.is_near(start, tolerance) {
        return curve.reversed();
    }
    Err(GeometryError::InvalidBrepTopology {
        context: "a surface split curve must meet both requested boundary locations",
    })
}

fn surface_split_parameter_curve(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    start: Point2,
    end: Point2,
    tolerance: Tolerance,
) -> Result<NurbsCurve2, GeometryError> {
    let curve_domain = curve.domain();
    let line = NurbsCurve2::try_new(
        1,
        vec![start, end],
        vec![
            *curve_domain.start(),
            *curve_domain.start(),
            *curve_domain.end(),
            *curve_domain.end(),
        ],
    )?;
    if parameter_curve_matches_spatial_curve(surface, &line, curve, tolerance)? {
        return Ok(line);
    }

    let parameter_curve = surface.try_pullback_bilinear_curve(curve, tolerance)?;
    let parameter_tolerance = [
        trim_parameter_epsilon(
            [*surface.domain_u().start(), *surface.domain_u().end()],
            tolerance,
        ),
        trim_parameter_epsilon(
            [*surface.domain_v().start(), *surface.domain_v().end()],
            tolerance,
        ),
    ];
    if !parameter_points_near(parameter_curve.start_point()?, start, parameter_tolerance)
        || !parameter_points_near(parameter_curve.end_point()?, end, parameter_tolerance)
    {
        return Err(GeometryError::InvalidBrepTopology {
            context: "a surface split p-curve must meet both requested boundary parameters",
        });
    }
    Ok(parameter_curve)
}

fn parameter_curve_matches_spatial_curve(
    surface: &NurbsSurface,
    parameter_curve: &NurbsCurve2,
    spatial_curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    const SAMPLES_PER_SPAN: usize = 16;
    let spatial_domain = spatial_curve.domain();
    let parameter_domain = parameter_curve.domain();
    let spatial_extent = *spatial_domain.end() - *spatial_domain.start();
    let parameter_extent = *parameter_domain.end() - *parameter_domain.start();
    require_finite(
        [spatial_extent, parameter_extent],
        "surface split curve parameter extents",
    )?;
    for (span_start, span_end) in spatial_curve.spans() {
        for sample in 0..=SAMPLES_PER_SPAN {
            let span_fraction = sample as Real / SAMPLES_PER_SPAN as Real;
            let spatial_parameter =
                span_start.mul_add(1.0 - span_fraction, span_end * span_fraction);
            let normalized = (spatial_parameter - *spatial_domain.start()) / spatial_extent;
            let parameter = normalized.mul_add(parameter_extent, *parameter_domain.start());
            let uv = parameter_curve.evaluate(parameter)?;
            let surface_point = surface.evaluate(uv.x(), uv.y())?;
            let spatial_point = spatial_curve.evaluate(spatial_parameter)?;
            let coordinate_scale = surface_point
                .to_array()
                .into_iter()
                .chain(spatial_point.to_array())
                .map(Real::abs)
                .fold(1.0, Real::max);
            let allowed = tolerance
                .absolute()
                .max(tolerance.relative() * coordinate_scale);
            if surface_point.distance_to(spatial_point)? > allowed {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn try_surface_cutting_face(
    surface: NurbsSurface,
    reversed: bool,
    vertex_parameters: impl AsRef<[Point2]>,
    edge_specs: impl IntoIterator<Item = ([usize; 2], NurbsCurve)>,
    loop_specs: impl IntoIterator<Item = (usize, bool, SurfaceIso)>,
    cut_parameter_curve: &NurbsCurve2,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    let vertex_parameters = vertex_parameters.as_ref();
    let vertices = vertex_parameters
        .iter()
        .map(|parameter| BrepVertex::try_new(surface.evaluate(parameter.x(), parameter.y())?, 0.0))
        .collect::<Result<Vec<_>, _>>()?;
    let edges = edge_specs
        .into_iter()
        .map(|(vertices, curve)| BrepEdge::try_new(vertices, curve, 0.0))
        .collect::<Result<Vec<_>, _>>()?;
    let trims = loop_specs
        .into_iter()
        .map(|(edge_index, reversed_3d, iso)| {
            let trim_vertices = oriented_edge_vertices(&edges[edge_index], reversed_3d);
            let parameter_curve = if iso == SurfaceIso::NotIso {
                if reversed_3d {
                    cut_parameter_curve.reversed()?
                } else {
                    cut_parameter_curve.clone()
                }
            } else {
                NurbsCurve2::try_line(
                    vertex_parameters[trim_vertices[0]],
                    vertex_parameters[trim_vertices[1]],
                )?
            };
            BrepTrim::try_new(
                trim_vertices,
                Some(edge_index),
                reversed_3d,
                parameter_curve,
                BrepTrimType::Boundary,
                iso,
                [0.0, 0.0],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let face = BrepFace::try_new(
        surface,
        reversed,
        vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
    )?;
    Brep::try_new(vertices, edges, vec![face], tolerance)
}

fn ordered_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn canonical_brep_coordinate_bits(coordinate: Real) -> u64 {
    if coordinate == 0.0 {
        0
    } else {
        coordinate.to_bits()
    }
}

fn validate_pyramid_dimensions<const N: usize>(
    side_count: usize,
    radii: [Real; N],
    height: Real,
    context: &'static str,
) -> Result<(), GeometryError> {
    require_finite(radii.into_iter().chain(std::iter::once(height)), context)?;
    if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
        return Err(GeometryError::InvalidRegularPolygonSides {
            actual: side_count,
            maximum: MAX_REGULAR_POLYGON_SIDES,
        });
    }
    if radii.into_iter().any(|radius| radius <= 0.0) || height == 0.0 {
        return Err(GeometryError::Degenerate { context });
    }
    Ok(())
}

fn regular_polygon_ring(
    frame: Frame3,
    side_count: usize,
    radius: Real,
    height: Real,
) -> Result<Vec<Point3>, GeometryError> {
    let mut points = Vec::with_capacity(side_count);
    for index in 0..side_count {
        let angle = std::f64::consts::TAU * index as Real / side_count as Real;
        points.push(frame_point(frame, radius, angle, height)?);
    }
    Ok(points)
}

fn frame_point(
    frame: Frame3,
    radius: Real,
    angle: Real,
    height: Real,
) -> Result<Point3, GeometryError> {
    frame
        .origin()
        .translated(frame.x_axis().as_vector().scaled(radius * angle.cos())?)?
        .translated(frame.y_axis().as_vector().scaled(radius * angle.sin())?)?
        .translated(frame.z_axis().as_vector().scaled(height)?)
}

fn push_line_edge(
    edges: &mut Vec<BrepEdge>,
    vertices: &[BrepVertex],
    indices: [usize; 2],
    domain: [Real; 2],
) -> Result<usize, GeometryError> {
    let curve = NurbsCurve::try_new(
        1,
        vec![vertices[indices[0]].point, vertices[indices[1]].point],
        vec![domain[0], domain[0], domain[1], domain[1]],
    )?;
    let index = edges.len();
    edges.push(BrepEdge::try_new(indices, curve, 0.0)?);
    Ok(index)
}

fn fitted_polygon_cap_frame(
    points: &[Point3],
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    debug_assert!(points.len() >= 3);
    let origin = Point3::try_new(
        (points[0].x() + points[1].x() + points[2].x()) / 3.0,
        (points[0].y() + points[1].y() + points[2].y()) / 3.0,
        (points[0].z() + points[1].z() + points[2].z()) / 3.0,
    )?;
    Frame3::try_from_x_and_normal(
        origin,
        points[0].vector_to(points[1])?,
        normal.as_vector(),
        tolerance,
    )
}

#[allow(clippy::too_many_arguments)]
fn polygon_cap_face(
    frame: Frame3,
    points: &[Point3],
    vertices: &[usize],
    edges: &[usize],
    edges_reversed: bool,
    face_reversed: bool,
    tolerance: Tolerance,
) -> Result<BrepFace, GeometryError> {
    debug_assert_eq!(points.len(), vertices.len());
    debug_assert_eq!(points.len(), edges.len());
    let mut parameters = Vec::with_capacity(points.len());
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for &point in points {
        let relative = frame.origin().vector_to(point)?;
        let parameter = Point2::try_new(
            relative.dot(frame.x_axis().as_vector())?,
            relative.dot(frame.y_axis().as_vector())?,
        )?;
        bounds[0][0] = bounds[0][0].min(parameter.x());
        bounds[0][1] = bounds[0][1].max(parameter.x());
        bounds[1][0] = bounds[1][0].min(parameter.y());
        bounds[1][1] = bounds[1][1].max(parameter.y());
        parameters.push(parameter);
    }
    let surface = planar_cap_surface(frame, Vector3::try_new(0.0, 0.0, 0.0)?, bounds)?;
    let mut trims = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        trims.push(BrepTrim::try_new(
            [vertices[index], vertices[next]],
            Some(edges[index]),
            edges_reversed,
            NurbsCurve2::try_line(parameters[index], parameters[next])?,
            BrepTrimType::Mated,
            cap_trim_iso(parameters[index], parameters[next], bounds, tolerance),
            [0.0, 0.0],
        )?);
    }
    BrepFace::try_new(
        surface,
        face_reversed,
        vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
    )
}

fn cap_trim_iso(
    start: Point2,
    end: Point2,
    bounds: [[Real; 2]; 2],
    tolerance: Tolerance,
) -> SurfaceIso {
    if tolerance.approx_eq(start.y(), bounds[1][0]) && tolerance.approx_eq(end.y(), bounds[1][0]) {
        SurfaceIso::South
    } else if tolerance.approx_eq(start.x(), bounds[0][1])
        && tolerance.approx_eq(end.x(), bounds[0][1])
    {
        SurfaceIso::East
    } else if tolerance.approx_eq(start.y(), bounds[1][1])
        && tolerance.approx_eq(end.y(), bounds[1][1])
    {
        SurfaceIso::North
    } else if tolerance.approx_eq(start.x(), bounds[0][0])
        && tolerance.approx_eq(end.x(), bounds[0][0])
    {
        SurfaceIso::West
    } else {
        SurfaceIso::NotIso
    }
}

fn mesh_face_bilinear_surface(corners: [Point3; 4]) -> Result<NurbsSurface, GeometryError> {
    const OPENNURBS_ZERO_TOLERANCE: Real = 2.328_306_436_538_696_3e-10;
    let mut domain_u_end = corners[0]
        .distance_to(corners[1])?
        .max(corners[3].distance_to(corners[2])?);
    if domain_u_end <= OPENNURBS_ZERO_TOLERANCE {
        domain_u_end = 1.0;
    }
    let mut domain_v_end = corners[1]
        .distance_to(corners[2])?
        .max(corners[0].distance_to(corners[3])?);
    if domain_v_end <= OPENNURBS_ZERO_TOLERANCE {
        domain_v_end = 1.0;
    }
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![corners[0], corners[1], corners[3], corners[2]],
        vec![0.0, 0.0, domain_u_end, domain_u_end],
        vec![0.0, 0.0, domain_v_end, domain_v_end],
    )
}

fn recompute_duplicated_face_vertex_tolerances(
    vertices: &mut [BrepVertex],
    edges: &[BrepEdge],
    faces: &[BrepFace],
) -> Result<(), GeometryError> {
    for (vertex_index, vertex) in vertices.iter_mut().enumerate() {
        let point = vertex.point;
        let mut maximum_distance = 0.0_f64;
        for edge in edges {
            let domain = edge.curve.domain();
            for (end, parameter) in [*domain.start(), *domain.end()].into_iter().enumerate() {
                if edge.vertices[end] == vertex_index {
                    maximum_distance =
                        maximum_distance.max(point.distance_to(edge.curve.evaluate(parameter)?)?);
                }
            }
        }
        for face in faces {
            for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
                if trim.edge.is_none() {
                    continue;
                }
                let parameters = [trim.curve.start_point()?, trim.curve.end_point()?];
                for (end, parameter) in parameters.into_iter().enumerate() {
                    if trim.vertices[end] == vertex_index {
                        maximum_distance =
                            maximum_distance.max(point.distance_to(
                                face.surface.evaluate(parameter.x(), parameter.y())?,
                            )?);
                    }
                }
            }
        }
        vertex.tolerance = if maximum_distance <= CURVE_COINCIDENCE_ABSOLUTE {
            0.0
        } else {
            let expanded = maximum_distance * 1.001;
            require_finite([expanded], "duplicated B-rep face vertex tolerance")?;
            expanded
        };
    }
    Ok(())
}

fn push_brep_wire(curves: &mut Vec<NurbsCurve>, curve: NurbsCurve) -> Result<(), GeometryError> {
    let first = curve.control_points()[0].point();
    if curve
        .control_points()
        .iter()
        .all(|control| control.point() == first)
    {
        return Ok(());
    }
    if curves.len() == crate::MAX_SURFACE_WIRES {
        return Err(GeometryError::TooManySurfaceWires);
    }
    curves.push(curve);
    Ok(())
}

#[derive(Clone, Copy)]
struct TrimUse<'a> {
    face: usize,
    face_loop: usize,
    trim: &'a BrepTrim,
}

#[derive(Clone, Copy)]
struct RectangularTrimSpec {
    vertices: [usize; 2],
    edge: Option<usize>,
    reversed_3d: bool,
    trim_type: BrepTrimType,
}

struct PlanarCurveProjection {
    frame: Frame3,
    coordinate_bounds: [[Real; 2]; 2],
    curve: NurbsCurve2,
    maximum_residual: Real,
}

impl RectangularTrimSpec {
    const fn edge(
        vertices: [usize; 2],
        edge: usize,
        reversed_3d: bool,
        trim_type: BrepTrimType,
    ) -> Self {
        Self {
            vertices,
            edge: Some(edge),
            reversed_3d,
            trim_type,
        }
    }

    const fn singular(vertex: usize) -> Self {
        Self {
            vertices: [vertex, vertex],
            edge: None,
            reversed_3d: false,
            trim_type: BrepTrimType::Singular,
        }
    }
}

fn rectangular_surface_loop(
    surface: &NurbsSurface,
    specs: [RectangularTrimSpec; 4],
) -> Result<BrepLoop, GeometryError> {
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    let parameter_corners = [
        Point2::try_new(*domain_u.start(), *domain_v.start())?,
        Point2::try_new(*domain_u.end(), *domain_v.start())?,
        Point2::try_new(*domain_u.end(), *domain_v.end())?,
        Point2::try_new(*domain_u.start(), *domain_v.end())?,
    ];
    let iso = [
        SurfaceIso::South,
        SurfaceIso::East,
        SurfaceIso::North,
        SurfaceIso::West,
    ];
    let trims = specs
        .into_iter()
        .enumerate()
        .map(|(index, spec)| {
            BrepTrim::try_new(
                spec.vertices,
                spec.edge,
                spec.reversed_3d,
                NurbsCurve2::try_line(
                    parameter_corners[index],
                    parameter_corners[(index + 1) % 4],
                )?,
                spec.trim_type,
                iso[index],
                [0.0, 0.0],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    BrepLoop::try_new(BrepLoopType::Outer, trims)
}

fn single_edge_loop(
    vertex: usize,
    edge: usize,
    loop_type: BrepLoopType,
    curve: NurbsCurve2,
    reversed_3d: bool,
    trim_type: BrepTrimType,
    tolerance: [Real; 2],
) -> Result<BrepLoop, GeometryError> {
    BrepLoop::try_new(
        loop_type,
        vec![BrepTrim::try_new(
            [vertex, vertex],
            Some(edge),
            reversed_3d,
            curve,
            trim_type,
            SurfaceIso::NotIso,
            tolerance,
        )?],
    )
}

fn tube_wall_surface(
    frame: Frame3,
    radius: Real,
    height: Real,
    domain_u: [Real; 2],
    clockwise: bool,
) -> Result<NurbsSurface, GeometryError> {
    require_finite(domain_u, "tube wall parameter domain")?;
    if domain_u[0] >= domain_u[1] {
        return Err(GeometryError::Degenerate {
            context: "tube wall parameter domain",
        });
    }
    let source = NurbsSurface::try_cylinder(frame, radius, 0.0, height)?;
    let count_u = source.control_point_count_u();
    let count_v = source.control_point_count_v();
    let mut controls = Vec::with_capacity(source.control_points().len());
    for row in source.control_points().chunks_exact(count_u) {
        if clockwise {
            controls.extend(row.iter().rev().copied());
        } else {
            controls.extend_from_slice(row);
        }
    }
    let tau = std::f64::consts::TAU;
    let knots_u = source
        .knots_u()
        .iter()
        .map(|knot| {
            let mapped = if *knot == 0.0 {
                domain_u[0]
            } else if *knot == tau {
                domain_u[1]
            } else {
                radius.mul_add(*knot, domain_u[0])
            };
            require_finite([mapped], "tube wall knot")?;
            Ok(mapped)
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        count_u,
        count_v,
        controls,
        knots_u,
        source.knots_v().to_vec(),
    )
}

fn circular_parameter_curve(radius: Real) -> Result<NurbsCurve2, GeometryError> {
    require_finite([radius], "circular trim radius")?;
    if radius <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "circular trim",
        });
    }
    let coordinates = [
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [-1.0, 1.0],
        [-1.0, 0.0],
        [-1.0, -1.0],
        [0.0, -1.0],
        [1.0, -1.0],
        [1.0, 0.0],
    ];
    let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
    let weights = [
        1.0,
        diagonal_weight,
        1.0,
        diagonal_weight,
        1.0,
        diagonal_weight,
        1.0,
        diagonal_weight,
        1.0,
    ];
    let controls = coordinates
        .into_iter()
        .zip(weights)
        .map(|([x, y], weight)| {
            WeightedPoint2::try_new(Point2::try_new(radius * x, radius * y)?, weight)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let half_pi = std::f64::consts::FRAC_PI_2;
    let pi = std::f64::consts::PI;
    let tau = std::f64::consts::TAU;
    NurbsCurve2::try_new_rational(
        2,
        controls,
        vec![
            0.0,
            0.0,
            0.0,
            half_pi,
            half_pi,
            pi,
            pi,
            3.0 * half_pi,
            3.0 * half_pi,
            tau,
            tau,
            tau,
        ],
    )
}

fn oriented_cap_curve(mut curve: NurbsCurve2) -> Result<(NurbsCurve2, bool), GeometryError> {
    let trial_loop = BrepLoop::try_new(
        BrepLoopType::Outer,
        vec![BrepTrim::try_new(
            [0, 0],
            Some(0),
            false,
            curve.clone(),
            BrepTrimType::Mated,
            SurfaceIso::NotIso,
            [0.0, 0.0],
        )?],
    )?;
    let reversed = sampled_loop_signed_area(&trial_loop)? < 0.0;
    if reversed {
        curve = curve.reversed()?;
    }
    Ok((curve, reversed))
}

fn project_planar_curve(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<PlanarCurveProjection, GeometryError> {
    let controls = curve.control_points();
    let origin = curve.evaluate(*curve.domain().start())?;
    let stride = (controls.len() / 64).max(1);
    let mut largest_cross = None;
    let mut largest_area = 0.0;
    for first_index in (1..controls.len()).step_by(stride) {
        let first = origin.vector_to(controls[first_index].point())?;
        for second_index in ((first_index + stride)..controls.len()).step_by(stride) {
            let second = origin.vector_to(controls[second_index].point())?;
            let cross = first.cross(second)?;
            let area = cross.length()?;
            if area > largest_area {
                largest_area = area;
                largest_cross = Some(cross);
            }
        }
    }
    let normal = largest_cross
        .ok_or(GeometryError::InvalidCappedExtrusionProfile)?
        .normalized_nonzero()?;
    let frame = Frame3::try_from_normal(origin, normal.as_vector(), tolerance)?;
    let mut coordinate_bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    let mut maximum_residual: Real = 0.0;
    let mut projected = Vec::with_capacity(controls.len());
    for control in controls {
        let relative = origin.vector_to(control.point())?;
        let x = relative.dot(frame.x_axis().as_vector())?;
        let y = relative.dot(frame.y_axis().as_vector())?;
        let residual = relative.dot(frame.z_axis().as_vector())?.abs();
        if residual > tolerance.absolute() {
            return Err(GeometryError::InvalidCappedExtrusionProfile);
        }
        coordinate_bounds[0][0] = coordinate_bounds[0][0].min(x);
        coordinate_bounds[0][1] = coordinate_bounds[0][1].max(x);
        coordinate_bounds[1][0] = coordinate_bounds[1][0].min(y);
        coordinate_bounds[1][1] = coordinate_bounds[1][1].max(y);
        maximum_residual = maximum_residual.max(residual);
        projected.push(WeightedPoint2::try_new(
            Point2::try_new(x, y)?,
            control.weight(),
        )?);
    }
    for bounds in coordinate_bounds {
        let width = bounds[1] - bounds[0];
        require_finite([width], "planar curve coordinate bounds")?;
        if width <= tolerance.absolute() {
            return Err(GeometryError::InvalidCappedExtrusionProfile);
        }
    }
    Ok(PlanarCurveProjection {
        frame,
        coordinate_bounds,
        curve: NurbsCurve2::try_new_rational(curve.degree(), projected, curve.knots().to_vec())?,
        maximum_residual,
    })
}

fn project_curve_to_frame(
    curve: &NurbsCurve,
    frame: Frame3,
    tolerance: Tolerance,
) -> Result<(NurbsCurve2, Real), GeometryError> {
    let mut maximum_residual: Real = 0.0;
    let projected = curve
        .control_points()
        .iter()
        .map(|control| {
            let relative = frame.origin().vector_to(control.point())?;
            let x = relative.dot(frame.x_axis().as_vector())?;
            let y = relative.dot(frame.y_axis().as_vector())?;
            let residual = relative.dot(frame.z_axis().as_vector())?.abs();
            if residual > tolerance.absolute() {
                return Err(GeometryError::InvalidPlanarFaceBoundary);
            }
            maximum_residual = maximum_residual.max(residual);
            WeightedPoint2::try_new(Point2::try_new(x, y)?, control.weight())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        NurbsCurve2::try_new_rational(curve.degree(), projected, curve.knots().to_vec())?,
        maximum_residual,
    ))
}

fn planar_cap_surface(
    frame: Frame3,
    offset: Vector3,
    bounds: [[Real; 2]; 2],
) -> Result<NurbsSurface, GeometryError> {
    let origin = frame.origin().translated(offset)?;
    let x_axis = frame.x_axis().as_vector();
    let y_axis = frame.y_axis().as_vector();
    let point = |x: Real, y: Real| {
        origin
            .translated(x_axis.scaled(x)?)?
            .translated(y_axis.scaled(y)?)
    };
    let corners = [
        point(bounds[0][0], bounds[1][0])?,
        point(bounds[0][1], bounds[1][0])?,
        point(bounds[0][0], bounds[1][1])?,
        point(bounds[0][1], bounds[1][1])?,
    ];
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        corners.to_vec(),
        vec![bounds[0][0], bounds[0][0], bounds[0][1], bounds[0][1]],
        vec![bounds[1][0], bounds[1][0], bounds[1][1], bounds[1][1]],
    )
}

fn extrusion_closure_tolerance(
    seam_vertices: [Point3; 2],
    profile_edges: [&NurbsCurve; 2],
    cap_surfaces: [&NurbsSurface; 2],
    cap_curve: &NurbsCurve2,
    cap_curve_reversed: bool,
    planar_residual: Real,
) -> Result<Real, GeometryError> {
    let mut maximum = planar_residual;
    for index in 0..2 {
        maximum = maximum.max(cap_closure_tolerance(
            seam_vertices[index],
            profile_edges[index],
            cap_surfaces[index],
            cap_curve,
            cap_curve_reversed,
            planar_residual,
        )?);
    }
    require_nonnegative_finite(maximum, "capped extrusion closure tolerance")?;
    Ok(maximum)
}

fn cap_closure_tolerance(
    seam_vertex: Point3,
    profile_edge: &NurbsCurve,
    cap_surface: &NurbsSurface,
    cap_curve: &NurbsCurve2,
    cap_curve_reversed: bool,
    planar_residual: Real,
) -> Result<Real, GeometryError> {
    if profile_edge.control_points().len() != cap_curve.control_points().len() {
        return invalid("a capped extrusion rim and p-curve have different control counts");
    }
    let cap_parameters = [cap_curve.start_point()?, cap_curve.end_point()?];
    let mut maximum = planar_residual;
    let domain = profile_edge.domain();
    for parameter in [*domain.start(), *domain.end()] {
        maximum = maximum.max(profile_edge.evaluate(parameter)?.distance_to(seam_vertex)?);
    }
    for parameter in cap_parameters {
        maximum = maximum.max(
            cap_surface
                .evaluate(parameter.x(), parameter.y())?
                .distance_to(seam_vertex)?,
        );
    }
    for (parameter_index, parameter_control) in cap_curve.control_points().iter().enumerate() {
        let profile_index = if cap_curve_reversed {
            profile_edge.control_points().len() - 1 - parameter_index
        } else {
            parameter_index
        };
        maximum = maximum.max(
            cap_surface
                .evaluate(parameter_control.point().x(), parameter_control.point().y())?
                .distance_to(profile_edge.control_points()[profile_index].point())?,
        );
    }
    require_nonnegative_finite(maximum, "capped extrusion closure tolerance")?;
    Ok(maximum)
}

fn frame_at_height(
    frame: Frame3,
    height: Real,
    tolerance: Tolerance,
) -> Result<Frame3, GeometryError> {
    let origin = frame
        .origin()
        .translated(frame.z_axis().as_vector().scaled(height)?)?;
    Frame3::try_from_directions(
        origin,
        frame.x_axis().as_vector(),
        frame.y_axis().as_vector(),
        tolerance,
    )
}

fn surface_u_control_curve(
    surface: &NurbsSurface,
    v_index: usize,
) -> Result<NurbsCurve, GeometryError> {
    let controls = (0..surface.control_point_count_u())
        .map(|u_index| {
            surface
                .control_point(u_index, v_index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "a requested surface boundary control row is missing",
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(surface.degree_u(), controls, surface.knots_u().to_vec())
}

fn surface_v_control_curve(
    surface: &NurbsSurface,
    u_index: usize,
) -> Result<NurbsCurve, GeometryError> {
    let controls = (0..surface.control_point_count_v())
        .map(|v_index| {
            surface
                .control_point(u_index, v_index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "a requested surface boundary control column is missing",
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(surface.degree_v(), controls, surface.knots_v().to_vec())
}

fn centered_surface(
    surface: &NurbsSurface,
    reference: Point3,
) -> Result<NurbsSurface, GeometryError> {
    let controls = surface
        .control_points()
        .iter()
        .map(|control| {
            let point = control.point();
            WeightedPoint3::try_new(
                Point3::try_new(
                    point.x() - reference.x(),
                    point.y() - reference.y(),
                    point.z() - reference.z(),
                )?,
                control.weight(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        controls,
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )
}

/// Homogeneous numerator of one p-curve coordinate minus a scan value.
/// Refining this scalar spline to Bernstein spans lets sign variation isolate
/// every transverse trim crossing without tessellating the trim.
struct ScalarSpline {
    degree: usize,
    controls: Vec<Real>,
    knots: Vec<Real>,
}

struct ScalarBezierSpan {
    parameter: [Real; 2],
    coefficients: Vec<Real>,
}

fn trim_isocurve_to_intervals(
    curve: NurbsCurve,
    intervals: Vec<[Real; 2]>,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let domain = curve.domain();
    intervals
        .into_iter()
        .filter(|interval| interval[0] < interval[1])
        .map(|interval| {
            if interval[0] == *domain.start() && interval[1] == *domain.end() {
                Ok(curve.clone())
            } else {
                curve.try_trimmed(interval[0]..=interval[1])
            }
        })
        .filter_map(|result| match result {
            Ok(curve)
                if curve
                    .control_points()
                    .iter()
                    .skip(1)
                    .any(|control| control.point() != curve.control_points()[0].point()) =>
            {
                Some(Ok(curve))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn trimmed_isocurve_intervals(
    face: &BrepFace,
    varying_axis: usize,
    fixed_value: Real,
    tolerance: Tolerance,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    debug_assert!(varying_axis < 2);
    require_finite([fixed_value], "B-rep isocurve parameter")?;
    let fixed_axis = 1 - varying_axis;
    let varying_domain = if varying_axis == 0 {
        face.surface.domain_u()
    } else {
        face.surface.domain_v()
    };
    let fixed_domain = if fixed_axis == 0 {
        face.surface.domain_u()
    } else {
        face.surface.domain_v()
    };
    if fixed_value < *fixed_domain.start() || fixed_value > *fixed_domain.end() {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter: fixed_value,
            domain_start: *fixed_domain.start(),
            domain_end: *fixed_domain.end(),
        });
    }

    let mut epsilon =
        trim_parameter_epsilon([*varying_domain.start(), *varying_domain.end()], tolerance);
    let mut events = Vec::new();
    let mut overlaps = Vec::new();
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        epsilon = epsilon.max(trim.tolerance[varying_axis]);
        collect_trim_scan_data(
            &trim.curve,
            fixed_axis,
            varying_axis,
            fixed_value,
            &mut events,
            &mut overlaps,
        )?;
    }

    let domain = [*varying_domain.start(), *varying_domain.end()];
    events.retain(|value| parameter_interval_contains(domain, *value, epsilon));
    for value in &mut events {
        *value = value.clamp(domain[0], domain[1]);
    }
    events.sort_by(Real::total_cmp);

    let mut intervals = Vec::new();
    let mut inside_start = None;
    let mut index = 0;
    while index < events.len() {
        let mut after = index + 1;
        while after < events.len() && events[after] - events[index] <= epsilon {
            after += 1;
        }
        if (after - index) % 2 == 1 {
            let crossing = events[index] * 0.5 + events[after - 1] * 0.5;
            if let Some(start) = inside_start.take() {
                if start < crossing {
                    intervals.push([start, crossing]);
                }
            } else {
                inside_start = Some(crossing);
            }
        }
        index = after;
    }
    if let Some(start) = inside_start
        && start < domain[1]
    {
        intervals.push([start, domain[1]]);
    }

    for mut overlap in overlaps {
        overlap.sort_by(Real::total_cmp);
        overlap[0] = overlap[0].clamp(domain[0], domain[1]);
        overlap[1] = overlap[1].clamp(domain[0], domain[1]);
        if overlap[0] < overlap[1] {
            intervals.push(overlap);
        }
    }
    merge_trim_intervals(intervals, epsilon)
}

fn trim_parameter_epsilon(domain: [Real; 2], tolerance: Tolerance) -> Real {
    let scale = domain[0].abs().max(domain[1].abs()).max(1.0);
    floating_parameter_epsilon(domain).max(tolerance.relative() * scale)
}

fn floating_parameter_epsilon(domain: [Real; 2]) -> Real {
    256.0 * Real::EPSILON * domain[0].abs().max(domain[1].abs()).max(1.0)
}

fn parameter_interval_contains(interval: [Real; 2], value: Real, epsilon: Real) -> bool {
    (value >= interval[0] || interval[0] - value <= epsilon)
        && (value <= interval[1] || value - interval[1] <= epsilon)
}

fn merge_trim_intervals(
    mut intervals: Vec<[Real; 2]>,
    epsilon: Real,
) -> Result<Vec<[Real; 2]>, GeometryError> {
    intervals.sort_by(|left, right| {
        left[0]
            .total_cmp(&right[0])
            .then_with(|| left[1].total_cmp(&right[1]))
    });
    let mut merged: Vec<[Real; 2]> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        if let Some(previous) = merged.last_mut()
            && interval[0] - previous[1] <= epsilon
        {
            previous[1] = previous[1].max(interval[1]);
        } else {
            merged.push(interval);
        }
    }
    if merged.iter().any(|interval| {
        !interval[0].is_finite() || !interval[1].is_finite() || interval[0] >= interval[1]
    }) {
        return Err(GeometryError::TrimIntersectionDidNotConverge);
    }
    Ok(merged)
}

fn collect_trim_scan_data(
    curve: &NurbsCurve2,
    fixed_axis: usize,
    varying_axis: usize,
    fixed_value: Real,
    events: &mut Vec<Real>,
    overlaps: &mut Vec<[Real; 2]>,
) -> Result<(), GeometryError> {
    let spans = scalar_bezier_spans(curve, fixed_axis, fixed_value)?;
    let mut roots = Vec::new();
    for span in spans {
        if span
            .coefficients
            .iter()
            .all(|coefficient| *coefficient == 0.0)
        {
            let start = curve.evaluate(span.parameter[0])?;
            let end = curve.evaluate(span.parameter[1])?;
            overlaps.push([
                parameter_coordinate(start, varying_axis),
                parameter_coordinate(end, varying_axis),
            ]);
        } else {
            collect_bernstein_roots(
                &span.coefficients,
                span.parameter,
                0,
                true,
                true,
                &mut roots,
            );
        }
    }

    roots.sort_by(Real::total_cmp);
    let curve_domain = curve.domain();
    let root_epsilon = floating_parameter_epsilon([*curve_domain.start(), *curve_domain.end()]);
    let mut unique_roots = Vec::with_capacity(roots.len());
    for root in roots {
        let root = if root - *curve_domain.start() <= root_epsilon {
            *curve_domain.start()
        } else if *curve_domain.end() - root <= root_epsilon {
            *curve_domain.end()
        } else {
            root
        };
        if unique_roots
            .last()
            .is_none_or(|previous| root - *previous > root_epsilon)
        {
            unique_roots.push(root);
        }
    }

    for (index, root) in unique_roots.iter().copied().enumerate() {
        let before_bound = index
            .checked_sub(1)
            .map_or(*curve_domain.start(), |previous| unique_roots[previous]);
        let after_bound = unique_roots
            .get(index + 1)
            .copied()
            .unwrap_or(*curve_domain.end());
        let before = (root > *curve_domain.start())
            .then(|| coordinate_sign_between(curve, fixed_axis, fixed_value, before_bound, root))
            .transpose()?
            .flatten();
        let after = (root < *curve_domain.end())
            .then(|| coordinate_sign_between(curve, fixed_axis, fixed_value, root, after_bound))
            .transpose()?
            .flatten();
        let root_coordinate = parameter_coordinate(curve.evaluate(root)?, fixed_axis);
        let at_root = if root_coordinate < fixed_value {
            Some(-1)
        } else if root_coordinate > fixed_value {
            Some(1)
        } else {
            None
        };
        let toggles = if root == *curve_domain.start() {
            at_root.map_or(after == Some(1), |at_root| {
                after.is_some_and(|after| after != at_root)
            })
        } else if root == *curve_domain.end() {
            at_root.map_or(before == Some(1), |at_root| {
                before.is_some_and(|before| before != at_root)
            })
        } else {
            matches!((before, after), (Some(-1), Some(1)) | (Some(1), Some(-1)))
        };
        if toggles {
            let point = curve.evaluate(root)?;
            events.push(parameter_coordinate(point, varying_axis));
        }
    }
    Ok(())
}

fn coordinate_sign_between(
    curve: &NurbsCurve2,
    fixed_axis: usize,
    fixed_value: Real,
    start: Real,
    end: Real,
) -> Result<Option<i8>, GeometryError> {
    for fraction in [0.5, 0.25, 0.75, 0.125, 0.875] {
        let parameter = start.mul_add(1.0 - fraction, end * fraction);
        if parameter <= start || parameter >= end {
            continue;
        }
        let coordinate = parameter_coordinate(curve.evaluate(parameter)?, fixed_axis);
        if coordinate < fixed_value {
            return Ok(Some(-1));
        }
        if coordinate > fixed_value {
            return Ok(Some(1));
        }
    }
    Ok(None)
}

fn parameter_coordinate(point: Point2, axis: usize) -> Real {
    if axis == 0 { point.x() } else { point.y() }
}

fn scalar_bezier_spans(
    curve: &NurbsCurve2,
    fixed_axis: usize,
    fixed_value: Real,
) -> Result<Vec<ScalarBezierSpan>, GeometryError> {
    let coordinate_scale = curve
        .control_points()
        .iter()
        .map(|control| parameter_coordinate(control.point(), fixed_axis).abs())
        .fold(fixed_value.abs(), Real::max)
        .max(1.0);
    let weight_scale = curve
        .control_points()
        .iter()
        .map(|control| control.weight().abs())
        .fold(0.0, Real::max);
    let controls = curve
        .control_points()
        .iter()
        .map(|control| {
            let coordinate = parameter_coordinate(control.point(), fixed_axis);
            let difference = if coordinate == fixed_value {
                0.0
            } else {
                coordinate / coordinate_scale - fixed_value / coordinate_scale
            };
            difference * (control.weight() / weight_scale)
        })
        .collect::<Vec<_>>();
    require_finite(
        controls.iter().copied(),
        "B-rep trim intersection polynomial",
    )?;
    let mut spline = ScalarSpline {
        degree: curve.degree(),
        controls,
        knots: curve.knots().to_vec(),
    };
    spline.clamp_to_domain()?;
    spline.refine_internal_knots()?;

    let mut spans = Vec::new();
    for span in spline.degree..spline.controls.len() {
        if spline.knots[span] < spline.knots[span + 1] {
            spans.push(ScalarBezierSpan {
                parameter: [spline.knots[span], spline.knots[span + 1]],
                coefficients: spline.controls[span - spline.degree..=span].to_vec(),
            });
        }
    }
    Ok(spans)
}

impl ScalarSpline {
    fn clamp_to_domain(&mut self) -> Result<(), GeometryError> {
        self.clamp_start()?;
        self.clamp_end()
    }

    fn clamp_start(&mut self) -> Result<(), GeometryError> {
        let start = self.knots[self.degree];
        if self.knots[..=self.degree].iter().all(|knot| *knot == start) {
            return Ok(());
        }
        let span = find_span_in_knots(&self.knots, self.degree, self.controls.len(), start);
        let (_, right) =
            scalar_de_boor_sides(&self.controls, &self.knots, self.degree, span, start)?;
        let mut controls = Vec::with_capacity(self.controls.len() - (span - self.degree));
        controls.extend(right);
        controls.extend_from_slice(&self.controls[span + 1..]);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.resize(self.degree + 1, start);
        knots.extend_from_slice(&self.knots[span + 1..]);
        self.controls = controls;
        self.knots = knots;
        Ok(())
    }

    fn clamp_end(&mut self) -> Result<(), GeometryError> {
        let end = self.knots[self.controls.len()];
        if self.knots[self.knots.len() - self.degree - 1..]
            .iter()
            .all(|knot| *knot == end)
        {
            return Ok(());
        }
        let span = find_span_in_knots(&self.knots, self.degree, self.controls.len(), end);
        let (left, _) = scalar_de_boor_sides(&self.controls, &self.knots, self.degree, span, end)?;
        let first_active = span - self.degree;
        let mut controls = Vec::with_capacity(span + 1);
        controls.extend_from_slice(&self.controls[..first_active]);
        controls.extend(left);
        let mut knots = Vec::with_capacity(controls.len() + self.degree + 1);
        knots.extend_from_slice(&self.knots[..=span]);
        knots.resize(knots.len() + self.degree + 1, end);
        self.controls = controls;
        self.knots = knots;
        Ok(())
    }

    fn refine_internal_knots(&mut self) -> Result<(), GeometryError> {
        let domain_start = self.knots[self.degree];
        let domain_end = self.knots[self.controls.len()];
        let mut internal = Vec::new();
        for knot in self.knots.iter().copied() {
            if knot > domain_start
                && knot < domain_end
                && internal.last().is_none_or(|previous| *previous != knot)
            {
                internal.push(knot);
            }
        }
        for knot in internal {
            while self.knots.iter().filter(|value| **value == knot).count() < self.degree {
                self.insert_once(knot)?;
            }
        }
        Ok(())
    }

    fn insert_once(&mut self, parameter: Real) -> Result<(), GeometryError> {
        let span = self.knots.partition_point(|knot| *knot <= parameter) - 1;
        let multiplicity = self.knots.iter().filter(|knot| **knot == parameter).count();
        let first_unchanged = span - self.degree;
        let first_shifted = span - multiplicity + 1;
        let mut controls = Vec::with_capacity(self.controls.len() + 1);
        for new_index in 0..=self.controls.len() {
            let control = if new_index <= first_unchanged {
                self.controls[new_index]
            } else if new_index < first_shifted {
                let alpha = crate::nurbs::interval_fraction(
                    parameter,
                    self.knots[new_index],
                    self.knots[new_index + self.degree],
                )?;
                scalar_blend(
                    self.controls[new_index - 1],
                    self.controls[new_index],
                    alpha,
                )?
            } else {
                self.controls[new_index - 1]
            };
            controls.push(control);
        }
        self.knots.insert(span + 1, parameter);
        self.controls = controls;
        Ok(())
    }
}

fn scalar_de_boor_sides(
    controls: &[Real],
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: Real,
) -> Result<(Vec<Real>, Vec<Real>), GeometryError> {
    let mut work = controls[span - degree..=span].to_vec();
    let mut left = Vec::with_capacity(degree + 1);
    let mut right = Vec::with_capacity(degree + 1);
    left.push(work[0]);
    right.push(work[degree]);
    for level in 1..=degree {
        for local_index in (level..=degree).rev() {
            let knot_index = span - degree + local_index;
            let alpha = crate::nurbs::interval_fraction(
                parameter,
                knots[knot_index],
                knots[knot_index + degree - level + 1],
            )?;
            work[local_index] = scalar_blend(work[local_index - 1], work[local_index], alpha)?;
        }
        left.push(work[level]);
        right.push(work[degree]);
    }
    right.reverse();
    Ok((left, right))
}

fn scalar_blend(left: Real, right: Real, alpha: Real) -> Result<Real, GeometryError> {
    let value = left.mul_add(1.0 - alpha, right * alpha);
    require_finite([value], "B-rep trim intersection polynomial")?;
    Ok(value)
}

fn collect_bernstein_roots(
    coefficients: &[Real],
    parameter: [Real; 2],
    depth: usize,
    include_start: bool,
    include_end: bool,
    roots: &mut Vec<Real>,
) {
    if include_start && coefficients[0] == 0.0 {
        roots.push(parameter[0]);
    }
    if include_end && coefficients[coefficients.len() - 1] == 0.0 {
        roots.push(parameter[1]);
    }
    let sign_changes = bernstein_sign_changes(coefficients);
    if sign_changes == 0 {
        return;
    }
    let middle = parameter[0] * 0.5 + parameter[1] * 0.5;
    if depth >= MAX_TRIM_ROOT_DEPTH || middle <= parameter[0] || middle >= parameter[1] {
        roots.push(middle);
        return;
    }
    let (left, right) = subdivide_bernstein_half(coefficients);
    collect_bernstein_roots(
        &left,
        [parameter[0], middle],
        depth + 1,
        include_start,
        true,
        roots,
    );
    collect_bernstein_roots(
        &right,
        [middle, parameter[1]],
        depth + 1,
        false,
        include_end,
        roots,
    );
}

fn bernstein_sign_changes(coefficients: &[Real]) -> usize {
    let mut previous = 0_i8;
    let mut changes = 0;
    for coefficient in coefficients {
        let sign = if *coefficient < 0.0 {
            -1
        } else if *coefficient > 0.0 {
            1
        } else {
            continue;
        };
        if previous != 0 && sign != previous {
            changes += 1;
        }
        previous = sign;
    }
    changes
}

fn subdivide_bernstein_half(coefficients: &[Real]) -> (Vec<Real>, Vec<Real>) {
    let degree = coefficients.len() - 1;
    let mut work = coefficients.to_vec();
    let mut left = Vec::with_capacity(coefficients.len());
    let mut right = vec![0.0; coefficients.len()];
    left.push(work[0]);
    right[degree] = work[degree];
    for level in 1..=degree {
        for index in 0..=degree - level {
            work[index] = work[index] * 0.5 + work[index + 1] * 0.5;
        }
        left.push(work[0]);
        right[degree - level] = work[degree - level];
    }
    (left, right)
}

#[derive(Clone, Copy)]
struct PlanarSurfacePlane {
    point: Point3,
    normal: UnitVector3,
}

fn planar_surface_plane(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Option<PlanarSurfacePlane>, GeometryError> {
    Ok(surface.plane(tolerance)?.map(|plane| PlanarSurfacePlane {
        point: plane.origin(),
        normal: plane.normal(),
    }))
}

fn integrate_planar_trimmed_face_volume(
    face: &BrepFace,
    surface: &NurbsSurface,
    plane: PlanarSurfacePlane,
    absolute_area_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let doubled_area = integrate_planar_trimmed_face_doubled_area(
        face,
        surface,
        plane,
        absolute_area_tolerance,
        relative_tolerance,
    )?;
    let plane_position = Vector3::try_new(plane.point.x(), plane.point.y(), plane.point.z())?;
    let plane_distance = plane_position.dot(plane.normal.as_vector())?;
    let magnitude = product_three(
        plane_distance.abs(),
        doubled_area.abs(),
        1.0 / 6.0,
        "planar B-rep face volume",
    )?;
    let orientation = if face.reversed { -1.0 } else { 1.0 };
    Ok(orientation * plane_distance.signum() * doubled_area.signum() * magnitude)
}

fn integrate_planar_trimmed_face_doubled_area(
    face: &BrepFace,
    surface: &NurbsSurface,
    plane: PlanarSurfacePlane,
    absolute_area_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let span_count = face
        .loops
        .iter()
        .flat_map(|face_loop| &face_loop.trims)
        .map(|trim| trim.curve.spans().count())
        .try_fold(0_usize, |total, count| {
            total
                .checked_add(count)
                .ok_or(GeometryError::NumericalIntegrationDidNotConverge)
        })?;
    if span_count == 0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let span_tolerance = (absolute_area_tolerance / span_count as Real).max(Real::MIN_POSITIVE);
    let mut sum = 0.0;
    let mut correction = 0.0;
    for trim in face.loops.iter().flat_map(|face_loop| &face_loop.trims) {
        for (start, end) in trim.curve.spans() {
            let doubled_area = integrate_adaptive(
                start,
                end,
                span_tolerance,
                relative_tolerance,
                |parameter| {
                    let (surface_parameter, parameter_derivative) =
                        trim.curve.evaluate_with_derivative(parameter)?;
                    let (point, derivative_u, derivative_v) = surface
                        .evaluate_with_derivatives(surface_parameter.x(), surface_parameter.y())?;
                    let derivative = Vector3::try_new(
                        derivative_u.x().mul_add(
                            parameter_derivative[0],
                            derivative_v.x() * parameter_derivative[1],
                        ),
                        derivative_u.y().mul_add(
                            parameter_derivative[0],
                            derivative_v.y() * parameter_derivative[1],
                        ),
                        derivative_u.z().mul_add(
                            parameter_derivative[0],
                            derivative_v.z() * parameter_derivative[1],
                        ),
                    )?;
                    let position = Vector3::try_new(point.x(), point.y(), point.z())?;
                    position.cross(derivative)?.dot(plane.normal.as_vector())
                },
            )?;
            neumaier_add(&mut sum, &mut correction, doubled_area);
        }
    }
    let doubled_area = sum + correction;
    require_finite([doubled_area], "planar B-rep doubled face area")?;
    Ok(doubled_area)
}

fn integrate_volume_patch(
    surface: &NurbsSurface,
    reversed: bool,
    u: [Real; 2],
    v: [Real; 2],
    absolute_tolerance: Real,
    relative_tolerance: Real,
) -> Result<Real, GeometryError> {
    let half_u = u[1] * 0.5 - u[0] * 0.5;
    let half_v = v[1] * 0.5 - v[0] * 0.5;
    require_finite([half_u, half_v], "B-rep volume parameter span")?;
    if half_u <= 0.0 || half_v <= 0.0 {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    let inner_tolerance = (absolute_tolerance * 0.25).max(Real::MIN_POSITIVE);
    integrate_adaptive(
        0.0,
        1.0,
        absolute_tolerance,
        relative_tolerance,
        |normalized_u| {
            let parameter_u = normalized_span_parameter(u, normalized_u)?;
            integrate_adaptive(
                0.0,
                1.0,
                inner_tolerance,
                relative_tolerance,
                |normalized_v| {
                    let parameter_v = normalized_span_parameter(v, normalized_v)?;
                    let (point, derivative_u, derivative_v) =
                        surface.evaluate_with_derivatives(parameter_u, parameter_v)?;
                    let position = Vector3::try_new(point.x(), point.y(), point.z())?;
                    let normalized_u = derivative_u.scaled(half_u)?;
                    let normalized_v = derivative_v.scaled(half_v)?;
                    let triple = position.dot(normalized_u.cross(normalized_v)?)?;
                    let magnitude =
                        product_three(triple.abs(), 4.0, 1.0 / 3.0, "B-rep volume integrand")?;
                    let orientation = if reversed { -1.0 } else { 1.0 };
                    Ok(orientation * triple.signum() * magnitude)
                },
            )
        },
    )
}

fn normalized_span_parameter(span: [Real; 2], normalized: Real) -> Result<Real, GeometryError> {
    let parameter = span[0].mul_add(1.0 - normalized, span[1] * normalized);
    require_finite([parameter], "B-rep volume parameter")?;
    Ok(parameter)
}

fn neumaier_add(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

fn brep_span_parameter(
    start: Real,
    end: Real,
    sample: usize,
    sample_count: usize,
    domain_end: Real,
) -> Real {
    let fraction = sample as Real / sample_count as Real;
    let parameter = start.mul_add(1.0 - fraction, end * fraction);
    if sample == sample_count && end < domain_end {
        parameter.next_down().max(start)
    } else {
        parameter
    }
}

fn snap_point_to_candidates(
    point: &mut Point3,
    candidates: &[BoundarySnapPoint],
    tolerance: Tolerance,
) {
    let mut nearest = None;
    for candidate in candidates {
        let scale = point
            .to_array()
            .into_iter()
            .chain(candidate.point.to_array())
            .map(Real::abs)
            .fold(0.0, Real::max);
        let allowed = candidate.tolerance.max(tolerance.relative() * scale);
        if let Ok(distance) = point.distance_to(candidate.point)
            && distance <= allowed
            && nearest.is_none_or(|(nearest_distance, _)| distance < nearest_distance)
        {
            nearest = Some((distance, candidate.point));
        }
    }
    if let Some((_, candidate)) = nearest {
        *point = candidate;
    }
}

fn snap_points_to_candidates(
    points: &mut [Point3],
    mut candidates: Vec<BoundarySnapPoint>,
    tolerance: Tolerance,
) {
    let candidate_scale = candidates
        .iter()
        .flat_map(|candidate| candidate.point.to_array().map(Real::abs))
        .fold(0.0, Real::max);
    let component_tolerance = candidates
        .iter()
        .map(|candidate| candidate.tolerance)
        .fold(tolerance.absolute(), Real::max);
    candidates.sort_by(|left, right| left.point.x().total_cmp(&right.point.x()));

    for point in points {
        let point_scale = point
            .to_array()
            .into_iter()
            .map(Real::abs)
            .fold(0.0, Real::max);
        let search_radius =
            component_tolerance.max(tolerance.relative() * point_scale.max(candidate_scale));
        let lower = point.x() - search_radius;
        let upper = point.x() + search_radius;
        let start = if lower.is_finite() {
            candidates.partition_point(|candidate| candidate.point.x() < lower)
        } else {
            0
        };
        let end = if upper.is_finite() {
            candidates.partition_point(|candidate| candidate.point.x() <= upper)
        } else {
            candidates.len()
        };
        snap_point_to_candidates(point, &candidates[start..end], tolerance);
    }
}

fn validate_iso(
    face: &BrepFace,
    trim: &BrepTrim,
    tolerance: [Real; 2],
) -> Result<(), GeometryError> {
    let domain_u = face.surface.domain_u();
    let domain_v = face.surface.domain_v();
    let (coordinate, expected, allowed, interior) = match trim.iso {
        SurfaceIso::NotIso => return Ok(()),
        SurfaceIso::InteriorUConstant => (
            0,
            trim.curve.control_points()[0].point().x(),
            tolerance[0],
            true,
        ),
        SurfaceIso::InteriorVConstant => (
            1,
            trim.curve.control_points()[0].point().y(),
            tolerance[1],
            true,
        ),
        SurfaceIso::South => (1, *domain_v.start(), tolerance[1], false),
        SurfaceIso::East => (0, *domain_u.end(), tolerance[0], false),
        SurfaceIso::North => (1, *domain_v.end(), tolerance[1], false),
        SurfaceIso::West => (0, *domain_u.start(), tolerance[0], false),
    };
    let domain = if coordinate == 0 { domain_u } else { domain_v };
    if interior && (expected <= *domain.start() || expected >= *domain.end()) {
        return invalid("an interior isoparametric trim is not inside its surface domain");
    }
    if trim.curve.control_points().iter().any(|control| {
        let point = control.point();
        let value = if coordinate == 0 {
            point.x()
        } else {
            point.y()
        };
        (value - expected).abs() > allowed
    }) {
        invalid("an isoparametric trim leaves its declared surface side")
    } else {
        Ok(())
    }
}

fn rectangular_face_trim_bounds(
    face: &BrepFace,
    tolerance: Tolerance,
) -> Result<Option<[[Real; 2]; 2]>, GeometryError> {
    if face.loops.len() != 1
        || face.loops[0].loop_type != BrepLoopType::Outer
        || face.loops[0].trims.len() != 4
    {
        return Ok(None);
    }
    let trims = &face.loops[0].trims;
    if trims
        .iter()
        .any(|trim| trim.curve.degree() != 1 || trim.curve.control_points().len() != 2)
    {
        return Ok(None);
    }
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    for trim in trims {
        for point in [trim.curve.start_point()?, trim.curve.end_point()?] {
            bounds[0][0] = bounds[0][0].min(point.x());
            bounds[0][1] = bounds[0][1].max(point.x());
            bounds[1][0] = bounds[1][0].min(point.y());
            bounds[1][1] = bounds[1][1].max(point.y());
        }
    }
    if bounds[0][0] >= bounds[0][1] || bounds[1][0] >= bounds[1][1] {
        return Ok(None);
    }
    let corners = [
        Point2::try_new(bounds[0][0], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][0])?,
        Point2::try_new(bounds[0][1], bounds[1][1])?,
        Point2::try_new(bounds[0][0], bounds[1][1])?,
    ];
    let mut seen = [false; 4];
    for trim in trims {
        let allowed = [
            tolerance.absolute().max(trim.tolerance[0]),
            tolerance.absolute().max(trim.tolerance[1]),
        ];
        let start = trim.curve.start_point()?;
        let end = trim.curve.end_point()?;
        let Some(side) = (0..4).find(|&side| {
            parameter_points_near(start, corners[side], allowed)
                && parameter_points_near(end, corners[(side + 1) % 4], allowed)
        }) else {
            return Ok(None);
        };
        if seen[side] {
            return Ok(None);
        }
        seen[side] = true;
    }
    if !seen.into_iter().all(|side| side) {
        return Ok(None);
    }
    let domain_u = face.surface.domain_u();
    let domain_v = face.surface.domain_v();
    let allowed = tolerance.absolute();
    if bounds[0][0] < *domain_u.start() - allowed
        || bounds[0][1] > *domain_u.end() + allowed
        || bounds[1][0] < *domain_v.start() - allowed
        || bounds[1][1] > *domain_v.end() + allowed
    {
        return invalid("a rectangular face trim leaves its underlying surface domain");
    }
    Ok(Some(bounds))
}

pub(crate) fn face_covers_full_surface_domain(
    face: &BrepFace,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if face.loops.len() != 1 || face.loops[0].trims.len() != 4 {
        return Ok(false);
    }
    let domain_u = face.surface.domain_u();
    let domain_v = face.surface.domain_v();
    let corners = [
        Point2::try_new(*domain_u.start(), *domain_v.start())?,
        Point2::try_new(*domain_u.end(), *domain_v.start())?,
        Point2::try_new(*domain_u.end(), *domain_v.end())?,
        Point2::try_new(*domain_u.start(), *domain_v.end())?,
    ];
    let mut seen = [false; 4];
    for trim in &face.loops[0].trims {
        let side = match trim.iso {
            SurfaceIso::South => 0,
            SurfaceIso::East => 1,
            SurfaceIso::North => 2,
            SurfaceIso::West => 3,
            SurfaceIso::NotIso | SurfaceIso::InteriorUConstant | SurfaceIso::InteriorVConstant => {
                return Ok(false);
            }
        };
        if seen[side] {
            return Ok(false);
        }
        seen[side] = true;
        let allowed = [
            tolerance.absolute().max(trim.tolerance[0]),
            tolerance.absolute().max(trim.tolerance[1]),
        ];
        if !parameter_points_near(trim.curve.start_point()?, corners[side], allowed)
            || !parameter_points_near(trim.curve.end_point()?, corners[(side + 1) % 4], allowed)
        {
            return Ok(false);
        }
    }
    Ok(seen.into_iter().all(|side| side))
}

fn sample_trim_loop(
    face_loop: &BrepLoop,
    samples_per_span: usize,
) -> Result<Vec<Point2>, GeometryError> {
    let span_count = face_loop
        .trims
        .iter()
        .map(|trim| trim.curve.spans().count())
        .try_fold(0_usize, |total, count| {
            total
                .checked_add(count)
                .ok_or(GeometryError::TooManyMeshVertices)
        })?;
    let capacity = span_count
        .checked_mul(samples_per_span)
        .and_then(|count| count.checked_add(1))
        .ok_or(GeometryError::TooManyMeshVertices)?;
    if capacity > u32::MAX as usize {
        return Err(GeometryError::TooManyMeshVertices);
    }
    let mut points = Vec::with_capacity(capacity);
    for trim in &face_loop.trims {
        for (start, end) in trim.curve.spans() {
            if points.is_empty() {
                points.push(trim.curve.evaluate(start)?);
            }
            for sample in 1..=samples_per_span {
                let parameter = normalized_span_parameter(
                    [start, end],
                    sample as Real / samples_per_span as Real,
                )?;
                points.push(trim.curve.evaluate(parameter)?);
            }
        }
    }
    if points.len() > 1 {
        points.pop();
    }
    Ok(points)
}

fn append_trimmed_surface_grid_parameters(
    parameters: &mut Vec<Point2>,
    loop_lengths: &[usize],
    surface: &NurbsSurface,
    samples_per_span: usize,
) -> Result<(), GeometryError> {
    let boundary_vertex_count = loop_lengths
        .iter()
        .try_fold(0_usize, |total, length| total.checked_add(*length))
        .ok_or(GeometryError::TooManyMeshVertices)?;
    if boundary_vertex_count != parameters.len() || loop_lengths.is_empty() {
        return invalid("trimmed surface grid requires complete sampled boundary loops");
    }
    if boundary_vertex_count > MAX_CONSTRAINED_TRIM_VERTICES {
        return Err(GeometryError::TooManyMeshVertices);
    }
    let Some(normalization) = TrimParameterNormalization::try_from_points(parameters)? else {
        return Ok(());
    };
    let normalized_boundary = parameters
        .iter()
        .map(|parameter| normalization.normalize(*parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let mut loop_ranges = Vec::with_capacity(loop_lengths.len());
    let mut loop_start = 0;
    for length in loop_lengths {
        let loop_end = loop_start + *length;
        loop_ranges.push(loop_start..loop_end);
        loop_start = loop_end;
    }
    let epsilon = 64.0 * Real::EPSILON;
    let strictly_inside = |parameter: Point2| -> Result<bool, GeometryError> {
        let point = normalization.normalize(parameter)?;
        if loop_ranges
            .iter()
            .any(|range| point_on_trim_polygon(point, &normalized_boundary[range.clone()], epsilon))
        {
            return Ok(false);
        }
        if !point_in_trim_polygon(point, &normalized_boundary[loop_ranges[0].clone()], epsilon) {
            return Ok(false);
        }
        Ok(!loop_ranges[1..].iter().any(|range| {
            point_in_trim_polygon(point, &normalized_boundary[range.clone()], epsilon)
        }))
    };

    let sample_direction = |spans: Vec<(Real, Real)>| -> Result<Vec<Real>, GeometryError> {
        let capacity = spans
            .len()
            .checked_mul(samples_per_span)
            .and_then(|count| count.checked_add(1))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if capacity > MAX_CONSTRAINED_TRIM_VERTICES {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let mut result = Vec::with_capacity(capacity);
        for (span_index, span) in spans.into_iter().enumerate() {
            let first_sample = usize::from(span_index != 0);
            for sample in first_sample..=samples_per_span {
                result.push(normalized_span_parameter(
                    [span.0, span.1],
                    sample as Real / samples_per_span as Real,
                )?);
            }
        }
        Ok(result)
    };
    let parameters_u = sample_direction(surface.spans_u().collect())?;
    let parameters_v = sample_direction(surface.spans_v().collect())?;
    if parameters_u
        .len()
        .checked_mul(parameters_v.len())
        .is_none_or(|count| count > MAX_CONSTRAINED_TRIM_VERTICES)
    {
        return Err(GeometryError::TooManyMeshVertices);
    }
    for v in parameters_v {
        for &u in &parameters_u {
            let parameter = Point2::try_new(u, v)?;
            if strictly_inside(parameter)? {
                if parameters.len() == MAX_CONSTRAINED_TRIM_VERTICES {
                    return Err(GeometryError::TooManyMeshVertices);
                }
                parameters.push(parameter);
            }
        }
    }
    Ok(())
}

fn triangulate_simple_trim_polygon(
    parameters: &mut Vec<Point2>,
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    let Some(normalized) = normalized_trim_polygon(parameters)? else {
        return Ok(None);
    };
    let epsilon = 64.0 * Real::EPSILON;
    let vertex_count = normalized.len();
    if vertex_count < 3 {
        return Ok(None);
    }
    let doubled_area = (0..vertex_count)
        .map(|index| {
            let first = normalized[index];
            let second = normalized[(index + 1) % vertex_count];
            first[0].mul_add(second[1], -first[1] * second[0])
        })
        .fold((0.0, 0.0), |(mut sum, mut correction), value| {
            neumaier_add(&mut sum, &mut correction, value);
            (sum, correction)
        });
    if doubled_area.0 + doubled_area.1 <= epsilon {
        return Ok(None);
    }

    let is_convex = (0..vertex_count).all(|index| {
        polygon_cross(
            normalized[(index + vertex_count - 1) % vertex_count],
            normalized[index],
            normalized[(index + 1) % vertex_count],
        ) >= -epsilon
    });
    if is_convex && vertex_count > 3 {
        let center = [
            normalized.iter().map(|point| point[0]).sum::<Real>() / vertex_count as Real,
            normalized.iter().map(|point| point[1]).sum::<Real>() / vertex_count as Real,
        ];
        if (0..vertex_count).any(|index| {
            polygon_cross(
                normalized[index],
                normalized[(index + 1) % vertex_count],
                center,
            ) <= epsilon
        }) {
            return Ok(None);
        }
        let center_index =
            u32::try_from(parameters.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
        let parameter_center = stable_parameter_average(parameters)?;
        parameters.push(parameter_center);
        let triangles = (0..vertex_count)
            .map(|index| {
                Ok([
                    u32::try_from(index).map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from((index + 1) % vertex_count)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    center_index,
                ])
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        return Ok(Some(triangles));
    }
    if vertex_count > MAX_EAR_CLIP_VERTICES {
        return Ok(None);
    }

    let mut remaining = (0..vertex_count).collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(vertex_count - 2);
    while remaining.len() > 3 {
        let mut ear = None;
        for position in 0..remaining.len() {
            let previous = remaining[(position + remaining.len() - 1) % remaining.len()];
            let current = remaining[position];
            let next = remaining[(position + 1) % remaining.len()];
            let triangle = [normalized[previous], normalized[current], normalized[next]];
            if polygon_cross(triangle[0], triangle[1], triangle[2]) <= epsilon {
                continue;
            }
            let contains_vertex = remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_ccw_triangle(normalized[candidate], triangle, epsilon)
            });
            if !contains_vertex {
                ear = Some((position, [previous, current, next]));
                break;
            }
        }
        let Some((position, triangle)) = ear else {
            return Ok(None);
        };
        triangles.push(triangle.map(|index| index as u32));
        remaining.remove(position);
    }
    let final_triangle = [remaining[0], remaining[1], remaining[2]];
    if polygon_cross(
        normalized[final_triangle[0]],
        normalized[final_triangle[1]],
        normalized[final_triangle[2]],
    ) <= epsilon
    {
        return Ok(None);
    }
    triangles.push(final_triangle.map(|index| index as u32));
    Ok(Some(triangles))
}

fn triangulate_trim_region(
    parameters: &[Point2],
    loop_lengths: &[usize],
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    let Some(boundary_vertex_count) = loop_lengths
        .iter()
        .try_fold(0_usize, |total, length| total.checked_add(*length))
    else {
        return Ok(None);
    };
    if loop_lengths.is_empty()
        || loop_lengths.iter().any(|length| *length < 3)
        || parameters.len() > MAX_CONSTRAINED_TRIM_VERTICES
        || boundary_vertex_count > parameters.len()
    {
        return Ok(None);
    }
    let Some(normalized) = normalized_trim_polygon(parameters)? else {
        return Ok(None);
    };
    let mut loop_ranges = Vec::with_capacity(loop_lengths.len());
    let mut start = 0_usize;
    for length in loop_lengths {
        let end = start + *length;
        loop_ranges.push(start..end);
        start = end;
    }
    let loop_bounds = loop_ranges
        .iter()
        .map(|range| {
            normalized[range.clone()].iter().fold(
                [[Real::INFINITY, Real::NEG_INFINITY]; 2],
                |mut bounds, point| {
                    for coordinate in 0..2 {
                        bounds[coordinate][0] = bounds[coordinate][0].min(point[coordinate]);
                        bounds[coordinate][1] = bounds[coordinate][1].max(point[coordinate]);
                    }
                    bounds
                },
            )
        })
        .collect::<Vec<_>>();
    let epsilon = 64.0 * Real::EPSILON;
    let outer = &normalized[loop_ranges[0].clone()];

    // A valid Rhino loop can revisit a point where otherwise-disjoint boundary
    // branches touch. Spade requires one vertex per coordinate, so retain the
    // first sampled occurrence as the triangulation representative.
    let (vertices, representatives) = unique_trim_triangulation_vertices(&normalized, epsilon);
    let unique_vertex_count = vertices.len();
    let mut triangulation =
        match ConstrainedDelaunayTriangulation::<TrimTriangulationVertex>::bulk_load(vertices) {
            Ok(triangulation) => triangulation,
            Err(_) => return Ok(None),
        };
    if triangulation.num_vertices() != unique_vertex_count {
        return Ok(None);
    }
    let mut representative_handles = vec![None; parameters.len()];
    for vertex in triangulation.vertices() {
        representative_handles[vertex.data().source_index] = Some(vertex.fix());
    }
    let handles = representatives
        .iter()
        .map(|representative| representative_handles[*representative])
        .collect::<Vec<_>>();
    let Some(constraints) =
        trim_triangulation_constraints(&normalized, &loop_ranges, &representatives, epsilon)
    else {
        return Ok(None);
    };
    for [from_index, to_index] in &constraints {
        let Some(from) = handles[*from_index] else {
            return Ok(None);
        };
        let Some(to) = handles[*to_index] else {
            return Ok(None);
        };
        if triangulation.try_add_constraint(from, to).is_empty() {
            return Ok(None);
        }
    }
    if triangulation.num_constraints() < constraints.len() {
        return Ok(None);
    }

    let mut triangles = Vec::new();
    let mut actual_area = 0.0;
    let mut actual_area_correction = 0.0;
    for face in triangulation.inner_faces() {
        let handles = face.vertices();
        let points = handles.map(|vertex| {
            let point = vertex.data().position;
            [point.x, point.y]
        });
        let centroid = [
            (points[0][0] + points[1][0] + points[2][0]) / 3.0,
            (points[0][1] + points[1][1] + points[2][1]) / 3.0,
        ];
        if !point_in_trim_polygon(centroid, outer, epsilon)
            || loop_ranges[1..]
                .iter()
                .zip(&loop_bounds[1..])
                .any(|(range, bounds)| {
                    point_in_trim_bounds(centroid, *bounds, epsilon)
                        && point_in_trim_polygon(centroid, &normalized[range.clone()], epsilon)
                })
        {
            continue;
        }
        let doubled_area = polygon_cross(points[0], points[1], points[2]);
        if doubled_area <= epsilon {
            return Ok(None);
        }
        neumaier_add(&mut actual_area, &mut actual_area_correction, doubled_area);
        let mut triangle = [0_u32; 3];
        for (target, vertex) in triangle.iter_mut().zip(handles) {
            *target = u32::try_from(vertex.data().source_index)
                .map_err(|_| GeometryError::TooManyMeshVertices)?;
        }
        triangles.push(triangle);
    }
    if triangles.is_empty() {
        return Ok(None);
    }

    let mut expected_area = 0.0;
    let mut expected_area_correction = 0.0;
    for range in &loop_ranges {
        for index in range.clone() {
            let next = if index + 1 == range.end {
                range.start
            } else {
                index + 1
            };
            let contribution = normalized[index][0].mul_add(
                normalized[next][1],
                -normalized[index][1] * normalized[next][0],
            );
            neumaier_add(
                &mut expected_area,
                &mut expected_area_correction,
                contribution,
            );
        }
    }
    let expected_area = expected_area + expected_area_correction;
    let actual_area = actual_area + actual_area_correction;
    let area_error_tolerance = 1024.0 * Real::EPSILON * parameters.len() as Real;
    if expected_area <= epsilon || (actual_area - expected_area).abs() > area_error_tolerance {
        return Ok(None);
    }
    Ok(Some(triangles))
}

fn unique_trim_triangulation_vertices(
    normalized: &[[Real; 2]],
    epsilon: Real,
) -> (Vec<TrimTriangulationVertex>, Vec<usize>) {
    let cell = |coordinate: Real| (coordinate / epsilon).floor() as i64;
    let mut buckets = BTreeMap::<[i64; 2], Vec<usize>>::new();
    let mut vertices = Vec::with_capacity(normalized.len());
    let mut representatives = Vec::with_capacity(normalized.len());
    for (source_index, &point) in normalized.iter().enumerate() {
        let point_cell = [cell(point[0]), cell(point[1])];
        let mut representative = None;
        'neighbors: for x_delta in -1_i64..=1 {
            for y_delta in -1_i64..=1 {
                let neighbor = [point_cell[0] + x_delta, point_cell[1] + y_delta];
                let Some(candidates) = buckets.get(&neighbor) else {
                    continue;
                };
                for &candidate in candidates {
                    if (normalized[candidate][0] - point[0]).abs() <= epsilon
                        && (normalized[candidate][1] - point[1]).abs() <= epsilon
                    {
                        representative = Some(candidate);
                        break 'neighbors;
                    }
                }
            }
        }
        let representative = representative.unwrap_or_else(|| {
            buckets.entry(point_cell).or_default().push(source_index);
            vertices.push(TrimTriangulationVertex {
                position: TriangulationPoint2::new(point[0], point[1]),
                source_index,
            });
            source_index
        });
        representatives.push(representative);
    }
    (vertices, representatives)
}

fn trim_triangulation_constraints(
    normalized: &[[Real; 2]],
    loop_ranges: &[std::ops::Range<usize>],
    representatives: &[usize],
    epsilon: Real,
) -> Option<Vec<[usize; 2]>> {
    const MAX_CONTACT_REFINEMENT_COMPARISONS: usize = 1_000_000;

    let mut occurrence_counts = vec![0_usize; normalized.len()];
    for &representative in representatives {
        occurrence_counts[representative] += 1;
    }
    let mut neighbors = BTreeMap::<usize, Vec<usize>>::new();
    for range in loop_ranges {
        for index in range.clone() {
            let contact = representatives[index];
            if occurrence_counts[contact] < 2 {
                continue;
            }
            let previous = if index == range.start {
                range.end - 1
            } else {
                index - 1
            };
            let next = if index + 1 == range.end {
                range.start
            } else {
                index + 1
            };
            neighbors
                .entry(contact)
                .or_default()
                .extend([representatives[previous], representatives[next]]);
        }
    }
    for candidates in neighbors.values_mut() {
        candidates.sort_unstable();
        candidates.dedup();
    }

    let mut constraints = Vec::new();
    let mut seen = BTreeSet::<[usize; 2]>::new();
    let mut comparison_count = 0_usize;
    for range in loop_ranges {
        for source_from in range.clone() {
            let source_to = if source_from + 1 == range.end {
                range.start
            } else {
                source_from + 1
            };
            let from = representatives[source_from];
            let to = representatives[source_to];
            if from == to {
                return None;
            }
            let start = normalized[from];
            let end = normalized[to];
            let direction = [end[0] - start[0], end[1] - start[1]];
            let squared_length = direction[0].mul_add(direction[0], direction[1] * direction[1]);
            if !squared_length.is_finite() || squared_length == 0.0 {
                return None;
            }
            let mut splits = vec![(0.0, from), (1.0, to)];
            for contact in [from, to] {
                let Some(candidates) = neighbors.get(&contact) else {
                    continue;
                };
                for &candidate in candidates {
                    comparison_count = comparison_count.checked_add(1)?;
                    if comparison_count > MAX_CONTACT_REFINEMENT_COMPARISONS {
                        return None;
                    }
                    if candidate == from || candidate == to {
                        continue;
                    }
                    let point = normalized[candidate];
                    let cross = polygon_cross(start, end, point);
                    if !point_on_trim_segment(point, start, end, cross, epsilon) {
                        continue;
                    }
                    let offset = [point[0] - start[0], point[1] - start[1]];
                    let fraction =
                        offset[0].mul_add(direction[0], offset[1] * direction[1]) / squared_length;
                    if fraction > 0.0 && fraction < 1.0 {
                        splits.push((fraction, candidate));
                    }
                }
            }
            splits.sort_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            });
            splits.dedup_by(|left, right| left.1 == right.1);
            for pair in splits.windows(2) {
                let [from, to] = [pair[0].1, pair[1].1];
                if from == to {
                    continue;
                }
                let key = [from.min(to), from.max(to)];
                if seen.insert(key) {
                    constraints.push([from, to]);
                }
            }
        }
    }
    Some(constraints)
}

fn point_in_trim_bounds(point: [Real; 2], bounds: [[Real; 2]; 2], epsilon: Real) -> bool {
    (0..2).all(|coordinate| {
        point[coordinate] >= bounds[coordinate][0] - epsilon
            && point[coordinate] <= bounds[coordinate][1] + epsilon
    })
}

fn point_in_trim_polygon(point: [Real; 2], polygon: &[[Real; 2]], epsilon: Real) -> bool {
    let mut winding = 0_i64;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let cross = polygon_cross(start, end, point);
        if point_on_trim_segment(point, start, end, cross, epsilon) {
            return true;
        }
        if start[1] <= point[1] {
            if end[1] > point[1] && cross > epsilon {
                winding += 1;
            }
        } else if end[1] <= point[1] && cross < -epsilon {
            winding -= 1;
        }
    }
    winding != 0
}

fn point_on_trim_polygon(point: [Real; 2], polygon: &[[Real; 2]], epsilon: Real) -> bool {
    (0..polygon.len()).any(|index| {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        point_on_trim_segment(point, start, end, polygon_cross(start, end, point), epsilon)
    })
}

fn point_on_trim_segment(
    point: [Real; 2],
    start: [Real; 2],
    end: [Real; 2],
    cross: Real,
    epsilon: Real,
) -> bool {
    cross.abs() <= epsilon
        && point[0] >= start[0].min(end[0]) - epsilon
        && point[0] <= start[0].max(end[0]) + epsilon
        && point[1] >= start[1].min(end[1]) - epsilon
        && point[1] <= start[1].max(end[1]) + epsilon
}

#[derive(Clone, Copy)]
struct TrimParameterNormalization {
    coordinate_scale: Real,
    origin: [Real; 2],
    relative_scale: Real,
}

impl TrimParameterNormalization {
    fn try_from_points(parameters: &[Point2]) -> Result<Option<Self>, GeometryError> {
        let Some(origin) = parameters.first() else {
            return Ok(None);
        };
        let direct_relative = parameters
            .iter()
            .map(|point| [point.x() - origin.x(), point.y() - origin.y()])
            .collect::<Vec<_>>();
        if direct_relative
            .iter()
            .flatten()
            .all(|value| value.is_finite())
        {
            let relative_scale = direct_relative
                .iter()
                .flatten()
                .map(|value| value.abs())
                .fold(0.0, Real::max);
            return Ok((relative_scale > 0.0).then_some(Self {
                coordinate_scale: 1.0,
                origin: [origin.x(), origin.y()],
                relative_scale,
            }));
        }

        let coordinate_scale = parameters
            .iter()
            .flat_map(|point| [point.x().abs(), point.y().abs()])
            .fold(0.0, Real::max);
        if coordinate_scale == 0.0 {
            return Ok(None);
        }
        let scaled_origin = [origin.x() / coordinate_scale, origin.y() / coordinate_scale];
        let relative_scale = parameters
            .iter()
            .flat_map(|point| {
                [
                    point.x() / coordinate_scale - scaled_origin[0],
                    point.y() / coordinate_scale - scaled_origin[1],
                ]
            })
            .map(Real::abs)
            .fold(0.0, Real::max);
        require_finite(
            [coordinate_scale, relative_scale],
            "trim parameter normalization",
        )?;
        Ok((relative_scale > 0.0).then_some(Self {
            coordinate_scale,
            origin: scaled_origin,
            relative_scale,
        }))
    }

    fn normalize(self, parameter: Point2) -> Result<[Real; 2], GeometryError> {
        let normalized = [
            (parameter.x() / self.coordinate_scale - self.origin[0]) / self.relative_scale,
            (parameter.y() / self.coordinate_scale - self.origin[1]) / self.relative_scale,
        ];
        require_finite(normalized, "normalized trim parameter")?;
        Ok(normalized)
    }
}

fn normalized_trim_polygon(parameters: &[Point2]) -> Result<Option<Vec<[Real; 2]>>, GeometryError> {
    let Some(normalization) = TrimParameterNormalization::try_from_points(parameters)? else {
        return Ok(None);
    };
    Ok(Some(
        parameters
            .iter()
            .map(|parameter| normalization.normalize(*parameter))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn stable_parameter_average(parameters: &[Point2]) -> Result<Point2, GeometryError> {
    let average_coordinate = |coordinate: fn(Point2) -> Real| {
        let scale = parameters
            .iter()
            .map(|point| coordinate(*point).abs())
            .fold(0.0, Real::max);
        if scale == 0.0 {
            return 0.0;
        }
        let count = parameters.len() as Real;
        let mut sum = 0.0;
        let mut correction = 0.0;
        for point in parameters {
            neumaier_add(
                &mut sum,
                &mut correction,
                coordinate(*point) / scale / count,
            );
        }
        let minimum = parameters
            .iter()
            .map(|point| coordinate(*point) / scale)
            .fold(Real::INFINITY, Real::min);
        let maximum = parameters
            .iter()
            .map(|point| coordinate(*point) / scale)
            .fold(Real::NEG_INFINITY, Real::max);
        (sum + correction).clamp(minimum, maximum) * scale
    };
    Point2::try_new(average_coordinate(Point2::x), average_coordinate(Point2::y))
}

fn polygon_cross(first: [Real; 2], second: [Real; 2], third: [Real; 2]) -> Real {
    let first_edge = [second[0] - first[0], second[1] - first[1]];
    let second_edge = [third[0] - first[0], third[1] - first[1]];
    first_edge[0].mul_add(second_edge[1], -first_edge[1] * second_edge[0])
}

fn point_in_ccw_triangle(point: [Real; 2], triangle: [[Real; 2]; 3], epsilon: Real) -> bool {
    polygon_cross(triangle[0], triangle[1], point) >= -epsilon
        && polygon_cross(triangle[1], triangle[2], point) >= -epsilon
        && polygon_cross(triangle[2], triangle[0], point) >= -epsilon
}

fn sampled_loop_signed_area(face_loop: &BrepLoop) -> Result<Real, GeometryError> {
    let mut points = Vec::new();
    for trim in &face_loop.trims {
        if points.is_empty() {
            points.push(trim.curve.start_point()?);
        }
        for (start, end) in trim.curve.spans() {
            for sample in 1..=LOOP_SAMPLES_PER_SPAN {
                let fraction = sample as Real / LOOP_SAMPLES_PER_SPAN as Real;
                let parameter = start.mul_add(1.0 - fraction, end * fraction);
                points.push(trim.curve.evaluate(parameter)?);
            }
        }
    }
    let origin = points[0];
    let relative = points
        .iter()
        .map(|point| [point.x() - origin.x(), point.y() - origin.y()])
        .collect::<Vec<_>>();
    require_finite(
        relative.iter().flatten().copied(),
        "B-rep p-loop coordinates",
    )?;
    let scale = relative
        .iter()
        .flat_map(|point| point.iter())
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return invalid("a p-loop encloses no parameter-space area");
    }
    let mut sum = 0.0;
    let mut correction = 0.0;
    for index in 0..relative.len() {
        let first = relative[index].map(|value| value / scale);
        let second = relative[(index + 1) % relative.len()].map(|value| value / scale);
        let cross = first[0].mul_add(second[1], -first[1] * second[0]);
        let next = sum + cross;
        if sum.abs() >= cross.abs() {
            correction += (sum - next) + cross;
        } else {
            correction += (cross - next) + sum;
        }
        sum = next;
    }
    let doubled_area = sum + correction;
    if !doubled_area.is_finite() || doubled_area.abs() <= 1.0e-14 {
        invalid("a p-loop encloses no stable parameter-space area")
    } else {
        Ok(doubled_area)
    }
}

fn parameter_points_near(first: Point2, second: Point2, tolerance: [Real; 2]) -> bool {
    (first.x() - second.x()).abs() <= tolerance[0] && (first.y() - second.y()).abs() <= tolerance[1]
}

fn require_nonnegative_finite(value: Real, context: &'static str) -> Result<(), GeometryError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        invalid(context)
    }
}

fn scaled_tolerance(value: Real, scale: Real) -> Result<Real, GeometryError> {
    if value == 0.0 {
        return Ok(0.0);
    }
    let scaled = value * scale;
    require_nonnegative_finite(scaled, "transformed B-rep component tolerance")?;
    Ok(scaled)
}

fn invalid<T>(context: &'static str) -> Result<T, GeometryError> {
    Err(GeometryError::InvalidBrepTopology { context })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle3, ControlPointCurveClosure, Polyline3, UnitVector3, Vector3};

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn planar_polygon_brep(paths: &[Vec<Point3>]) -> Brep {
        assert!(!paths.is_empty() && paths.iter().all(|path| path.len() >= 3));
        let bounds = BoundingBox3::from_points(paths[0].iter().copied()).unwrap();
        let min = bounds.min();
        let max = bounds.max();
        assert!(min.x() < max.x() && min.y() < max.y() && min.z() == max.z());
        let surface = NurbsSurface::try_bilinear([
            min,
            point(max.x(), min.y(), min.z()),
            max,
            point(min.x(), max.y(), min.z()),
        ])
        .unwrap();
        let coordinate_scale = paths
            .iter()
            .flatten()
            .flat_map(|point| point.to_array().map(Real::abs))
            .fold(0.0, Real::max);
        let component_tolerance = 64.0 * Real::EPSILON * coordinate_scale;
        let vertices = paths
            .iter()
            .flatten()
            .map(|point| BrepVertex::try_new(*point, component_tolerance).unwrap())
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        let mut loops = Vec::new();
        let mut vertex_offset = 0_usize;
        for (loop_index, path) in paths.iter().enumerate() {
            let mut trims = Vec::new();
            for index in 0..path.len() {
                let from = vertex_offset + index;
                let to = vertex_offset + (index + 1) % path.len();
                let edge_index = edges.len();
                edges.push(
                    BrepEdge::try_new(
                        [from, to],
                        LineSegment::try_new(
                            path[index],
                            path[(index + 1) % path.len()],
                            Tolerance::DEFAULT,
                        )
                        .unwrap()
                        .to_nurbs()
                        .unwrap(),
                        component_tolerance,
                    )
                    .unwrap(),
                );
                let parameter = |point: Point3| {
                    Point2::try_new(
                        (point.x() - min.x()) / (max.x() - min.x()),
                        (point.y() - min.y()) / (max.y() - min.y()),
                    )
                    .unwrap()
                };
                trims.push(
                    BrepTrim::try_new(
                        [from, to],
                        Some(edge_index),
                        false,
                        NurbsCurve2::try_line(
                            parameter(path[index]),
                            parameter(path[(index + 1) % path.len()]),
                        )
                        .unwrap(),
                        BrepTrimType::Boundary,
                        SurfaceIso::NotIso,
                        [0.0, 0.0],
                    )
                    .unwrap(),
                );
            }
            loops.push(
                BrepLoop::try_new(
                    if loop_index == 0 {
                        BrepLoopType::Outer
                    } else {
                        BrepLoopType::Inner
                    },
                    trims,
                )
                .unwrap(),
            );
            vertex_offset += path.len();
        }
        Brep::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(surface, false, loops).unwrap()],
            Tolerance::DEFAULT,
        )
        .unwrap()
    }

    #[test]
    fn rectangular_surface_face_retains_underlying_surface_and_tessellates_its_trim() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 2.0),
            point(10.0, 10.0, 5.0),
            point(0.0, 10.0, -1.0),
        ])
        .unwrap()
        .try_reparameterized(2.0..=12.0, -3.0..=7.0)
        .unwrap();
        let brep = Brep::try_rectangular_surface_face(
            surface.clone(),
            2.0..=6.0,
            -3.0..=3.0,
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_eq!(brep.faces().len(), 1);
        let face = &brep.faces()[0];
        assert_eq!(face.surface(), &surface);
        assert_eq!(
            face.rectangular_trim_bounds(Tolerance::DEFAULT).unwrap(),
            Some([[2.0, 6.0], [-3.0, 3.0]])
        );
        assert_eq!(
            face.loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::iso)
                .collect::<Vec<_>>(),
            vec![
                SurfaceIso::South,
                SurfaceIso::InteriorUConstant,
                SurfaceIso::InteriorVConstant,
                SurfaceIso::West,
            ]
        );
        assert!(
            face.loops()[0]
                .trims()
                .iter()
                .all(|trim| !trim.is_reversed_3d())
        );
        assert_eq!(brep.edges()[2].curve().domain(), -6.0..=-2.0);
        assert_eq!(brep.edges()[3].curve().domain(), -3.0..=3.0);
        let bounds = brep.bounds();
        assert!(bounds.min().x().abs() <= 1.0e-12);
        assert!(bounds.min().y().abs() <= 1.0e-12);
        assert!(bounds.max().x() <= 4.0 + 1.0e-12);
        assert!(bounds.max().y() <= 6.0 + 1.0e-12);
        for mesh in [
            brep.tessellate(2, Tolerance::DEFAULT).unwrap(),
            brep.polygon_mesh(0.5, false, false, Tolerance::DEFAULT)
                .unwrap(),
        ] {
            assert!(!mesh.faces().is_empty());
            assert!(mesh.vertices().iter().all(|point| {
                point.x() >= -1.0e-12
                    && point.x() <= 4.0 + 1.0e-12
                    && point.y() >= -1.0e-12
                    && point.y() <= 6.0 + 1.0e-12
            }));
        }
        let trimmed_area = surface
            .try_trimmed(2.0..=6.0, -3.0..=3.0)
            .unwrap()
            .area(Tolerance::DEFAULT)
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(brep.area(Tolerance::DEFAULT).unwrap(), trimmed_area));

        let full = Brep::try_surface_face(surface.clone(), Tolerance::DEFAULT).unwrap();
        assert_eq!(full.faces()[0].surface(), &surface);
        assert!(face_covers_full_surface_domain(&full.faces()[0], Tolerance::DEFAULT).unwrap());
    }

    #[test]
    fn rectangular_surface_cutting_split_uses_rhino_topology_order() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();

        let [west, east] = Brep::try_split_rectangular_surface_face_u(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            4.0,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            west.vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 10.0, 0.0),
            ]
        );
        assert_eq!(
            west.edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 2], [1, 0], [2, 3], [3, 1]]
        );
        assert_eq!(
            west.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(2), false, SurfaceIso::InteriorUConstant),
                (Some(3), false, SurfaceIso::North),
                (Some(1), false, SurfaceIso::West),
                (Some(0), false, SurfaceIso::South),
            ]
        );
        assert_eq!(
            east.edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [1, 3], [2, 3], [2, 0]]
        );
        assert_eq!(
            east.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(3), false, SurfaceIso::South),
                (Some(0), false, SurfaceIso::East),
                (Some(1), false, SurfaceIso::North),
                (Some(2), true, SurfaceIso::InteriorUConstant),
            ]
        );

        let [south, north] = Brep::try_split_rectangular_surface_face_v(
            surface,
            0.0..=10.0,
            0.0..=10.0,
            6.0,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(south.faces()[0].is_reversed());
        assert!(north.faces()[0].is_reversed());
        assert_eq!(
            south
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 6.0, 0.0),
                point(10.0, 6.0, 0.0),
            ]
        );
        assert_eq!(
            south
                .edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [1, 3], [2, 3], [2, 0]]
        );
        assert_eq!(
            south.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(3), false, SurfaceIso::West),
                (Some(0), false, SurfaceIso::South),
                (Some(1), false, SurfaceIso::East),
                (Some(2), true, SurfaceIso::InteriorVConstant),
            ]
        );
        assert_eq!(
            north
                .edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [1, 2], [2, 3], [3, 0]]
        );
        assert_eq!(
            north.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(2), false, SurfaceIso::InteriorVConstant),
                (Some(3), false, SurfaceIso::East),
                (Some(0), false, SurfaceIso::North),
                (Some(1), false, SurfaceIso::West),
            ]
        );
    }

    #[test]
    fn rectangular_surface_diagonal_splits_match_rhino_topology() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let diagonal_length = 136.0_f64.sqrt();
        let west_east_curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 2.0, 0.0), point(10.0, 8.0, 0.0)],
            vec![0.0, 0.0, diagonal_length, diagonal_length],
        )
        .unwrap();
        let [south, north] = Brep::try_split_rectangular_surface_face_west_east(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [2.0, 8.0],
            west_east_curve,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            south
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(10.0, 8.0, 0.0),
            ]
        );
        assert_eq!(
            south
                .edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [1, 3], [2, 3], [2, 0]]
        );
        assert_eq!(south.edges()[2].curve().domain(), 0.0..=diagonal_length);
        assert_eq!(
            south.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(3), false, SurfaceIso::West),
                (Some(0), false, SurfaceIso::South),
                (Some(1), false, SurfaceIso::East),
                (Some(2), true, SurfaceIso::NotIso),
            ]
        );
        assert_eq!(
            north
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect::<Vec<_>>(),
            vec![
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(10.0, 8.0, 0.0),
            ]
        );
        assert_eq!(
            north.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(2), false, SurfaceIso::NotIso),
                (Some(3), false, SurfaceIso::East),
                (Some(0), false, SurfaceIso::North),
                (Some(1), false, SurfaceIso::West),
            ]
        );

        let south_north_curve = NurbsCurve::try_new(
            1,
            vec![point(2.0, 0.0, 0.0), point(8.0, 10.0, 0.0)],
            vec![0.0, 0.0, diagonal_length, diagonal_length],
        )
        .unwrap();
        let [west, east] = Brep::try_split_rectangular_surface_face_south_north(
            surface,
            0.0..=10.0,
            0.0..=10.0,
            [2.0, 8.0],
            south_north_curve,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(west.faces()[0].is_reversed());
        assert!(east.faces()[0].is_reversed());
        assert_eq!(
            west.edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 2], [1, 0], [2, 3], [3, 1]]
        );
        assert_eq!(
            west.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(2), false, SurfaceIso::NotIso),
                (Some(3), false, SurfaceIso::North),
                (Some(1), false, SurfaceIso::West),
                (Some(0), false, SurfaceIso::South),
            ]
        );
        assert_eq!(
            east.edges()
                .iter()
                .map(BrepEdge::vertices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [1, 3], [2, 3], [2, 0]]
        );
        assert_eq!(
            east.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d(), trim.iso()))
                .collect::<Vec<_>>(),
            vec![
                (Some(3), false, SurfaceIso::South),
                (Some(0), false, SurfaceIso::East),
                (Some(1), false, SurfaceIso::North),
                (Some(2), true, SurfaceIso::NotIso),
            ]
        );

        for brep in [&south, &north, &west, &east] {
            brep.tessellate(2, Tolerance::DEFAULT).unwrap();
        }
        assert!(Tolerance::DEFAULT.approx_eq(
            south.area(Tolerance::DEFAULT).unwrap() + north.area(Tolerance::DEFAULT).unwrap(),
            100.0,
        ));
        assert!(Tolerance::DEFAULT.approx_eq(
            west.area(Tolerance::DEFAULT).unwrap() + east.area(Tolerance::DEFAULT).unwrap(),
            100.0,
        ));
    }

    #[test]
    fn rectangular_surface_curve_arrangements_split_parallel_crossing_and_mixed_cuts() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let line = |start: [Real; 2], end: [Real; 2]| {
            NurbsCurve::try_new(
                1,
                vec![point(start[0], start[1], 0.0), point(end[0], end[1], 0.0)],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let assert_pieces = |pieces: &[Brep], expected_count: usize| {
            assert_eq!(pieces.len(), expected_count);
            let mut area = 0.0;
            for piece in pieces {
                assert_eq!(piece.faces().len(), 1);
                assert_eq!(piece.faces()[0].surface(), &surface);
                piece.tessellate(3, Tolerance::DEFAULT).unwrap();
                area += piece.area(Tolerance::DEFAULT).unwrap();
            }
            assert!(Tolerance::DEFAULT.approx_eq(area, 100.0));
        };

        let parallel = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [line([0.0, 2.0], [10.0, 4.0]), line([0.0, 6.0], [10.0, 8.0])],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&parallel, 3);
        assert!(parallel.iter().all(|piece| !piece.faces()[0].is_reversed()));
        assert_eq!(
            parallel
                .iter()
                .map(|piece| {
                    piece.faces()[0].loops()[0]
                        .trims()
                        .iter()
                        .filter(|trim| trim.iso() == SurfaceIso::NotIso)
                        .count()
                })
                .collect::<Vec<_>>(),
            vec![1, 2, 1]
        );

        let duplicate_cut = line([0.0, 2.0], [10.0, 8.0]);
        let duplicate = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [duplicate_cut.clone(), duplicate_cut.reversed().unwrap()],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&duplicate, 2);

        let shared_start = [
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 5.0, 0.0),
                    point(5.0, 5.0, 0.0),
                    point(10.0, 5.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap(),
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, 5.0, 0.0),
                    point(5.0, 5.0, 0.0),
                    point(10.0, 8.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap(),
        ];
        let shared_start_pieces = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            shared_start.clone(),
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&shared_start_pieces, 3);
        let mut shared_start_edge_counts = shared_start_pieces
            .iter()
            .map(|piece| piece.edges().len())
            .collect::<Vec<_>>();
        shared_start_edge_counts.sort_unstable();
        assert_eq!(shared_start_edge_counts, vec![3, 5, 5]);
        for cutters in [
            vec![shared_start[1].clone(), shared_start[0].clone()],
            vec![
                shared_start[0].reversed().unwrap(),
                shared_start[1].reversed().unwrap(),
            ],
            vec![shared_start[0].reversed().unwrap(), shared_start[1].clone()],
            vec![shared_start[0].clone(), shared_start[1].reversed().unwrap()],
        ] {
            let pieces = Brep::try_split_rectangular_surface_face_with_curves(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                cutters,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_pieces(&pieces, 3);
        }

        let shared_quadratic = [
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 5.0, 0.0),
                    point(2.5, 7.0, 0.0),
                    point(5.0, 5.0, 0.0),
                    point(7.5, 3.0, 0.0),
                    point(10.0, 4.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
            NurbsCurve::try_new(
                2,
                vec![
                    point(0.0, 5.0, 0.0),
                    point(2.5, 7.0, 0.0),
                    point(5.0, 5.0, 0.0),
                    point(7.5, 7.0, 0.0),
                    point(10.0, 8.0, 0.0),
                ],
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            )
            .unwrap(),
        ];
        let shared_quadratic_pieces = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            shared_quadratic,
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&shared_quadratic_pieces, 3);

        let lower = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 3.0, 0.0),
                point(3.0, 5.0, 0.0),
                point(7.0, 5.0, 0.0),
                point(10.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        )
        .unwrap();
        let upper = NurbsCurve::try_new(
            1,
            vec![
                point(0.0, 7.0, 0.0),
                point(3.0, 5.0, 0.0),
                point(7.0, 5.0, 0.0),
                point(10.0, 7.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        )
        .unwrap();
        for cutters in [
            vec![lower.clone(), upper.clone()],
            vec![upper.clone(), lower.clone()],
            vec![lower.reversed().unwrap(), upper.reversed().unwrap()],
            vec![lower.reversed().unwrap(), upper.clone()],
            vec![lower.clone(), upper.reversed().unwrap()],
        ] {
            let shared_middle_pieces = Brep::try_split_rectangular_surface_face_with_curves(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                cutters,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_pieces(&shared_middle_pieces, 4);
            let mut edge_counts = shared_middle_pieces
                .iter()
                .map(|piece| piece.edges().len())
                .collect::<Vec<_>>();
            edge_counts.sort_unstable();
            assert_eq!(edge_counts, vec![3, 3, 6, 6]);
        }

        let smooth = NurbsCurve::try_new(
            1,
            vec![
                point(10.0, 5.0, 0.0),
                point(5.0, 5.0, 0.0),
                point(0.0, 5.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        )
        .unwrap();
        let promoted_smooth = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [shared_start[1].clone(), smooth],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&promoted_smooth, 3);
        assert_eq!(
            promoted_smooth[0]
                .vertices()
                .iter()
                .copied()
                .map(BrepVertex::point)
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(5.0, 5.0, 0.0),
                point(10.0, 5.0, 0.0),
                point(0.0, 5.0, 0.0),
            ]
        );
        assert_eq!(promoted_smooth[0].edges()[3].curve().domain(), 1.0..=2.0);
        assert_eq!(
            promoted_smooth[0].faces()[0].loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::edge)
                .collect::<Vec<_>>(),
            vec![Some(3), Some(4), Some(0), Some(1), Some(2)]
        );

        let third = |left_y: Real| {
            NurbsCurve::try_new(
                1,
                vec![
                    point(0.0, left_y, 0.0),
                    point(3.0, 5.0, 0.0),
                    point(7.0, 5.0, 0.0),
                    point(10.0, 9.0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
            )
            .unwrap()
        };
        let three_way_smooth = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [lower.clone(), upper.clone(), third(5.0)],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&three_way_smooth, 6);
        assert_eq!(
            three_way_smooth[0]
                .edges()
                .iter()
                .map(|edge| edge.curve().domain())
                .collect::<Vec<_>>(),
            vec![
                0.0..=10.0,
                0.0..=3.0,
                0.0..=1.0,
                2.0..=3.0,
                1.0..=2.0,
                -3.0..=-0.0,
            ]
        );
        assert_eq!(
            three_way_smooth[3].faces()[0].loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::edge)
                .collect::<Vec<_>>(),
            vec![Some(4), Some(5), Some(0), Some(1), Some(2), Some(3)]
        );

        let three_way_kinked = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [lower.clone(), upper.clone(), third(4.0)],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&three_way_kinked, 6);

        let polygon = NurbsCurve::try_new(
            1,
            vec![
                point(3.0, 3.0, 0.0),
                point(7.0, 3.0, 0.0),
                point(7.0, 7.0, 0.0),
                point(3.0, 7.0, 0.0),
                point(3.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        let horizontal = line([0.0, 3.0], [10.0, 3.0]);
        for cutters in [
            vec![polygon.clone(), horizontal.clone()],
            vec![horizontal.clone(), polygon.clone()],
            vec![polygon.reversed().unwrap(), horizontal.reversed().unwrap()],
            vec![horizontal.reversed().unwrap(), polygon.reversed().unwrap()],
        ] {
            let pieces = Brep::try_split_rectangular_surface_face_with_curves(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                cutters,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_pieces(&pieces, 3);
            let mut edge_counts = pieces
                .iter()
                .map(|piece| piece.edges().len())
                .collect::<Vec<_>>();
            edge_counts.sort_unstable();
            assert_eq!(edge_counts, vec![4, 6, 8]);
        }

        let left = NurbsCurve::try_new(
            1,
            vec![
                point(2.0, 3.0, 0.0),
                point(5.0, 3.0, 0.0),
                point(5.0, 7.0, 0.0),
                point(2.0, 7.0, 0.0),
                point(2.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        let right = NurbsCurve::try_new(
            1,
            vec![
                point(5.0, 3.0, 0.0),
                point(8.0, 3.0, 0.0),
                point(8.0, 7.0, 0.0),
                point(5.0, 7.0, 0.0),
                point(5.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        for cutters in [
            vec![left.clone(), right.clone()],
            vec![right.clone(), left.clone()],
        ] {
            let pieces = Brep::try_split_rectangular_surface_face_with_curves(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                cutters,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_pieces(&pieces, 3);
        }

        let crossing = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [line([0.0, 2.0], [10.0, 8.0]), line([0.0, 8.0], [10.0, 2.0])],
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&crossing, 4);
        for piece in &crossing {
            assert!(piece.faces()[0].is_reversed());
            assert!(piece.vertices().iter().any(|vertex| {
                vertex
                    .point()
                    .is_near(point(5.0, 5.0, 0.0), Tolerance::DEFAULT)
            }));
        }

        let mixed = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [line([0.0, 2.0], [10.0, 8.0]), line([5.0, 0.0], [5.0, 10.0])],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_pieces(&mixed, 4);
        assert!(mixed.iter().all(|piece| !piece.faces()[0].is_reversed()));
        assert!(mixed.iter().all(|piece| {
            let trims = piece.faces()[0].loops()[0].trims();
            trims.iter().any(|trim| trim.iso() == SurfaceIso::NotIso)
                && trims
                    .iter()
                    .any(|trim| trim.iso() == SurfaceIso::InteriorUConstant)
        }));
    }

    #[test]
    fn rectangular_surface_curve_arrangements_retain_nonzero_two_edge_cells() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let cut = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 2.0, 0.0),
                point(5.0, 5.0, 0.0),
                point(0.0, 8.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let pieces = Brep::try_split_rectangular_surface_face_with_curves(
            surface,
            0.0..=10.0,
            0.0..=10.0,
            [cut],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(pieces.len(), 2);
        let mut edge_counts = pieces
            .iter()
            .map(|piece| piece.edges().len())
            .collect::<Vec<_>>();
        edge_counts.sort_unstable();
        assert_eq!(edge_counts, vec![2, 6]);
        assert!(pieces.iter().all(|piece| {
            piece.faces()[0].loops()[0]
                .trims()
                .iter()
                .any(|trim| trim.iso() == SurfaceIso::NotIso)
        }));
        let area = pieces
            .iter()
            .map(|piece| piece.area(Tolerance::DEFAULT).unwrap())
            .sum::<Real>();
        assert!(Tolerance::DEFAULT.approx_eq(area, 100.0));
    }

    #[test]
    fn rectangular_surface_closed_curve_splits_create_inner_and_holed_faces() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let polygon = NurbsCurve::try_new(
            1,
            vec![
                point(3.0, 3.0, 0.0),
                point(7.0, 3.0, 0.0),
                point(7.0, 7.0, 0.0),
                point(3.0, 7.0, 0.0),
                point(3.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
        let circle = NurbsCurve::try_new_rational(
            2,
            [
                ([7.0, 5.0], 1.0),
                ([7.0, 7.0], diagonal_weight),
                ([5.0, 7.0], 1.0),
                ([3.0, 7.0], diagonal_weight),
                ([3.0, 5.0], 1.0),
                ([3.0, 3.0], diagonal_weight),
                ([5.0, 3.0], 1.0),
                ([7.0, 3.0], diagonal_weight),
                ([7.0, 5.0], 1.0),
            ]
            .into_iter()
            .map(|(coordinates, weight)| {
                WeightedPoint3::try_new(point(coordinates[0], coordinates[1], 0.0), weight).unwrap()
            })
            .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
        )
        .unwrap();

        for (cut, expected_vertices, expected_edges, source_clockwise) in [
            (polygon.clone(), [8, 4], [8, 4], false),
            (polygon.reversed().unwrap(), [8, 4], [8, 4], true),
            (circle.clone(), [5, 1], [5, 1], false),
            (circle.reversed().unwrap(), [5, 1], [5, 1], true),
        ] {
            let [outside, inside] = Brep::try_split_rectangular_surface_face_with_closed_curve(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                cut,
                false,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(outside.vertices().len(), expected_vertices[0]);
            assert_eq!(outside.edges().len(), expected_edges[0]);
            assert_eq!(inside.vertices().len(), expected_vertices[1]);
            assert_eq!(inside.edges().len(), expected_edges[1]);
            assert_eq!(
                outside.faces()[0]
                    .loops()
                    .iter()
                    .map(BrepLoop::loop_type)
                    .collect::<Vec<_>>(),
                vec![BrepLoopType::Outer, BrepLoopType::Inner]
            );
            assert!(
                outside.faces()[0].loops()[1]
                    .trims()
                    .iter()
                    .all(|trim| trim.is_reversed_3d() != source_clockwise)
            );
            assert_eq!(
                inside.faces()[0].loops()[0].loop_type(),
                BrepLoopType::Outer
            );
            assert!(
                inside.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .all(|trim| trim.is_reversed_3d() == source_clockwise)
            );
            outside.tessellate(4, Tolerance::DEFAULT).unwrap();
            inside.tessellate(4, Tolerance::DEFAULT).unwrap();
            assert!(Tolerance::DEFAULT.approx_eq(
                outside.area(Tolerance::DEFAULT).unwrap()
                    + inside.area(Tolerance::DEFAULT).unwrap(),
                100.0,
            ));
        }

        let bow_tie = NurbsCurve::try_new(
            1,
            vec![
                point(3.0, 3.0, 0.0),
                point(7.0, 7.0, 0.0),
                point(3.0, 7.0, 0.0),
                point(7.0, 3.0, 0.0),
                point(3.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        assert!(
            Brep::try_split_rectangular_surface_face_with_closed_curve(
                surface,
                0.0..=10.0,
                0.0..=10.0,
                bow_tie,
                false,
                Tolerance::DEFAULT,
            )
            .is_err()
        );
    }

    #[test]
    fn surface_cut_halfedge_sort_orders_tangent_branches_by_bend_across_angle_seam() {
        let directions = [
            SurfaceCutHalfedgeDirection {
                tangent: [1.0, 0.0],
                angle: 0.0,
                bend: 0.0,
            },
            SurfaceCutHalfedgeDirection {
                tangent: [-1.0, 0.0],
                angle: std::f64::consts::PI,
                bend: 1.0,
            },
            SurfaceCutHalfedgeDirection {
                tangent: [-1.0, -0.0],
                angle: -std::f64::consts::PI,
                bend: -1.0,
            },
        ];
        let mut halfedges = [1, 0, 2];
        sort_surface_cut_outgoing_halfedges(
            &mut halfedges,
            &directions,
            Tolerance::DEFAULT.angular(),
        );
        assert_eq!(halfedges, [0, 2, 1]);
    }

    #[test]
    fn rectangular_surface_curve_arrangements_support_multiple_closed_regions() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let polygon = |x0: Real, x1: Real, y0: Real, y1: Real| {
            NurbsCurve::try_new(
                1,
                vec![
                    point(x0, y0, 0.0),
                    point(x1, y0, 0.0),
                    point(x1, y1, 0.0),
                    point(x0, y1, 0.0),
                    point(x0, y0, 0.0),
                ],
                vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
            )
            .unwrap()
        };
        let line = |start: [Real; 2], end: [Real; 2]| {
            NurbsCurve::try_new(
                1,
                vec![point(start[0], start[1], 0.0), point(end[0], end[1], 0.0)],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let circle = |center: [Real; 2], radius: Real| {
            let [x, y] = center;
            let diagonal_weight = std::f64::consts::FRAC_1_SQRT_2;
            NurbsCurve::try_new_rational(
                2,
                [
                    ([x + radius, y], 1.0),
                    ([x + radius, y + radius], diagonal_weight),
                    ([x, y + radius], 1.0),
                    ([x - radius, y + radius], diagonal_weight),
                    ([x - radius, y], 1.0),
                    ([x - radius, y - radius], diagonal_weight),
                    ([x, y - radius], 1.0),
                    ([x + radius, y - radius], diagonal_weight),
                    ([x + radius, y], 1.0),
                ]
                .into_iter()
                .map(|(coordinates, weight)| {
                    WeightedPoint3::try_new(point(coordinates[0], coordinates[1], 0.0), weight)
                        .unwrap()
                })
                .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
            )
            .unwrap()
        };
        let assert_case =
            |cuts: Vec<NurbsCurve>, expected_edges: &[usize], expected_loops: &[usize]| {
                let pieces = Brep::try_split_rectangular_surface_face_with_curves(
                    surface.clone(),
                    0.0..=10.0,
                    0.0..=10.0,
                    cuts,
                    false,
                    Tolerance::DEFAULT,
                )
                .unwrap();
                let mut edge_counts = pieces
                    .iter()
                    .map(|piece| piece.edges().len())
                    .collect::<Vec<_>>();
                let mut loop_counts = pieces
                    .iter()
                    .map(|piece| piece.faces()[0].loops().len())
                    .collect::<Vec<_>>();
                edge_counts.sort_unstable();
                loop_counts.sort_unstable();
                assert_eq!(edge_counts, expected_edges);
                assert_eq!(loop_counts, expected_loops);

                let mut area = 0.0;
                for piece in &pieces {
                    assert_eq!(piece.faces().len(), 1);
                    assert_eq!(piece.faces()[0].surface(), &surface);
                    assert_eq!(piece.faces()[0].loops()[0].loop_type(), BrepLoopType::Outer);
                    assert!(
                        piece.faces()[0]
                            .loops()
                            .iter()
                            .skip(1)
                            .all(|loop_| loop_.loop_type() == BrepLoopType::Inner)
                    );
                    piece.tessellate(4, Tolerance::DEFAULT).unwrap();
                    area += piece.area(Tolerance::DEFAULT).unwrap();
                }
                assert!(Tolerance::DEFAULT.approx_eq(area, 100.0));
            };

        assert_case(
            vec![polygon(1.0, 4.0, 2.0, 5.0), polygon(6.0, 9.0, 5.0, 8.0)],
            &[4, 4, 12],
            &[1, 1, 3],
        );
        assert_case(
            vec![polygon(2.0, 8.0, 2.0, 8.0), polygon(4.0, 6.0, 4.0, 6.0)],
            &[4, 8, 8],
            &[1, 2, 2],
        );
        assert_case(
            vec![polygon(3.0, 7.0, 3.0, 7.0), line([0.0, 5.0], [10.0, 5.0])],
            &[4, 4, 8, 8],
            &[1, 1, 1, 1],
        );
        assert_case(
            vec![polygon(2.0, 6.0, 2.0, 7.0), polygon(4.0, 8.0, 4.0, 9.0)],
            &[4, 6, 6, 12],
            &[1, 1, 1, 2],
        );
        assert_case(
            vec![polygon(1.0, 4.0, 1.0, 4.0), polygon(4.0, 7.0, 4.0, 7.0)],
            &[4, 4, 12],
            &[1, 1, 2],
        );
        let internally_touching = NurbsCurve::try_new(
            1,
            vec![
                point(2.0, 5.0, 0.0),
                point(5.0, 3.0, 0.0),
                point(7.0, 5.0, 0.0),
                point(5.0, 7.0, 0.0),
                point(2.0, 5.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        assert_case(
            vec![polygon(2.0, 8.0, 2.0, 8.0), internally_touching],
            &[4, 9, 9],
            &[1, 1, 2],
        );
        assert_case(
            vec![circle([2.5, 2.5], 1.0), circle([7.5, 7.5], 1.0)],
            &[1, 1, 6],
            &[1, 1, 3],
        );
        assert_case(
            vec![circle([3.5, 5.0], 1.5), circle([6.5, 5.0], 1.5)],
            &[1, 2, 7],
            &[1, 1, 2],
        );
        assert_case(
            vec![circle([5.0, 5.0], 3.0), circle([6.5, 5.0], 1.5)],
            &[1, 2, 5],
            &[1, 1, 2],
        );
        assert_case(
            vec![polygon(2.0, 5.0, 3.0, 7.0), circle([6.5, 5.0], 1.5)],
            &[2, 5, 11],
            &[1, 1, 2],
        );
        assert_case(
            vec![circle([6.0, 5.0], 2.0), polygon(2.0, 8.0, 2.0, 8.0)],
            &[1, 6, 9],
            &[1, 1, 2],
        );
        assert_case(
            vec![
                polygon(2.0, 5.0, 3.0, 7.0).reversed().unwrap(),
                circle([6.5, 5.0], 1.5).reversed().unwrap(),
            ],
            &[2, 5, 11],
            &[1, 1, 2],
        );
        assert_case(
            vec![
                polygon(2.0, 8.0, 2.0, 8.0).reversed().unwrap(),
                circle([6.0, 5.0], 2.0).reversed().unwrap(),
            ],
            &[1, 6, 9],
            &[1, 1, 2],
        );
        assert_case(
            vec![circle([5.0, 5.0], 2.0), line([0.0, 7.0], [10.0, 7.0])],
            &[2, 5, 7],
            &[1, 1, 1],
        );
        assert_case(
            vec![line([0.0, 7.0], [10.0, 7.0]), circle([5.0, 5.0], 2.0)],
            &[2, 5, 7],
            &[1, 1, 1],
        );
        assert_case(
            vec![
                circle([5.0, 5.0], 2.0).reversed().unwrap(),
                line([10.0, 7.0], [0.0, 7.0]),
            ],
            &[2, 5, 7],
            &[1, 1, 1],
        );
        assert_case(
            vec![circle([5.0, 5.0], 2.0), line([7.0, 0.0], [7.0, 10.0])],
            &[1, 5, 6],
            &[1, 1, 1],
        );
        let triangle = NurbsCurve::try_new(
            1,
            vec![
                point(3.0, 3.0, 0.0),
                point(7.0, 3.0, 0.0),
                point(5.0, 6.0, 0.0),
                point(3.0, 3.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        )
        .unwrap();
        assert_case(
            vec![triangle.clone(), line([0.0, 6.0], [10.0, 6.0])],
            &[3, 5, 8],
            &[1, 1, 1],
        );
        assert_case(
            vec![line([0.0, 6.0], [10.0, 6.0]), triangle],
            &[3, 5, 8],
            &[1, 1, 1],
        );
        let arch = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 2.0, 0.0),
                point(5.0, 10.0, 0.0),
                point(10.0, 2.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert_case(
            vec![arch.clone(), line([0.0, 6.0], [10.0, 6.0])],
            &[3, 3, 5, 5],
            &[1, 1, 1, 1],
        );
        assert_case(
            vec![line([0.0, 6.0], [10.0, 6.0]), arch.clone()],
            &[3, 3, 5, 5],
            &[1, 1, 1, 1],
        );
        assert_case(
            vec![arch.reversed().unwrap(), line([10.0, 6.0], [0.0, 6.0])],
            &[3, 3, 5, 5],
            &[1, 1, 1, 1],
        );
        let triple_nested = Brep::try_split_rectangular_surface_face_with_curves(
            surface,
            0.0..=10.0,
            0.0..=10.0,
            vec![
                polygon(2.0, 8.0, 2.0, 8.0),
                polygon(3.0, 7.0, 3.0, 7.0),
                polygon(4.0, 6.0, 4.0, 6.0),
            ],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut triple_edge_counts = triple_nested
            .iter()
            .map(|piece| piece.edges().len())
            .collect::<Vec<_>>();
        let mut triple_loop_counts = triple_nested
            .iter()
            .map(|piece| piece.faces()[0].loops().len())
            .collect::<Vec<_>>();
        triple_edge_counts.sort_unstable();
        triple_loop_counts.sort_unstable();
        assert_eq!(triple_edge_counts, vec![4, 8, 8, 8]);
        assert_eq!(triple_loop_counts, vec![1, 2, 2, 2]);
    }

    #[test]
    fn rectangular_surface_curve_arrangements_preserve_rational_cuts_and_corner_nodes() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let rational = |controls: [([Real; 2], Real); 3], domain: [Real; 2]| {
            NurbsCurve::try_new_rational(
                2,
                controls
                    .into_iter()
                    .map(|(coordinates, weight)| {
                        WeightedPoint3::try_new(point(coordinates[0], coordinates[1], 0.0), weight)
                            .unwrap()
                    })
                    .collect(),
                vec![
                    domain[0], domain[0], domain[0], domain[1], domain[1], domain[1],
                ],
            )
            .unwrap()
        };
        let curved = Brep::try_split_rectangular_surface_face_with_curves(
            surface.clone(),
            0.0..=10.0,
            0.0..=10.0,
            [
                rational(
                    [([0.0, 2.0], 1.0), ([5.0, 9.0], 0.75), ([10.0, 8.0], 1.0)],
                    [2.0, 6.0],
                ),
                rational(
                    [([0.0, 8.0], 1.0), ([5.0, 1.0], 1.25), ([10.0, 2.0], 1.0)],
                    [-3.0, 5.0],
                ),
            ],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(curved.len(), 4);
        let mut curved_area = 0.0;
        let mut curved_trim_count = 0;
        for piece in &curved {
            for trim in piece.faces()[0].loops()[0].trims() {
                if trim.iso() != SurfaceIso::NotIso {
                    continue;
                }
                curved_trim_count += 1;
                assert_eq!(trim.curve().degree(), 2);
                assert!(trim.curve().is_rational());
                let controls = trim.curve().control_points();
                assert_eq!(controls[0].weight(), 1.0);
                assert_eq!(controls[controls.len() - 1].weight(), 1.0);
            }
            curved_area += piece.area(Tolerance::DEFAULT).unwrap();
            piece.tessellate(4, Tolerance::DEFAULT).unwrap();
        }
        assert_eq!(curved_trim_count, 8);
        assert!(Tolerance::DEFAULT.approx_eq(curved_area, 100.0));

        let line = |end: [Real; 2]| {
            NurbsCurve::try_new(
                1,
                vec![point(0.0, 0.0, 0.0), point(end[0], end[1], 0.0)],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap()
        };
        let corner_fan = Brep::try_split_rectangular_surface_face_with_curves(
            surface,
            0.0..=10.0,
            0.0..=10.0,
            [line([10.0, 7.0]), line([6.0, 10.0])],
            false,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(corner_fan.len(), 3);
        let mut corner_area = 0.0;
        for piece in &corner_fan {
            assert_eq!(
                piece
                    .vertices()
                    .iter()
                    .filter(|vertex| vertex.point() == point(0.0, 0.0, 0.0))
                    .count(),
                1
            );
            corner_area += piece.area(Tolerance::DEFAULT).unwrap();
            piece.tessellate(3, Tolerance::DEFAULT).unwrap();
        }
        assert!(Tolerance::DEFAULT.approx_eq(corner_area, 100.0));
    }

    #[test]
    fn rectangular_surface_adjacent_side_splits_match_rhino_topology() {
        type SplitFunction = fn(
            NurbsSurface,
            RangeInclusive<Real>,
            RangeInclusive<Real>,
            [Real; 2],
            NurbsCurve,
            bool,
            Tolerance,
        ) -> Result<[Brep; 2], GeometryError>;
        struct Case {
            split: SplitFunction,
            side_parameters: [Real; 2],
            cut_endpoints: [Point3; 2],
            vertices: [Vec<Point3>; 2],
            edges: [Vec<[usize; 2]>; 2],
            trims: [Vec<(usize, bool, SurfaceIso)>; 2],
        }

        let sw = point(0.0, 0.0, 0.0);
        let se = point(10.0, 0.0, 0.0);
        let ne = point(10.0, 10.0, 0.0);
        let nw = point(0.0, 10.0, 0.0);
        let cases = [
            Case {
                split: Brep::try_split_rectangular_surface_face_south_east,
                side_parameters: [2.0, 7.0],
                cut_endpoints: [point(2.0, 0.0, 0.0), point(10.0, 7.0, 0.0)],
                vertices: [
                    vec![sw, ne, nw, point(2.0, 0.0, 0.0), point(10.0, 7.0, 0.0)],
                    vec![se, point(2.0, 0.0, 0.0), point(10.0, 7.0, 0.0)],
                ],
                edges: [
                    vec![[0, 3], [1, 2], [2, 0], [3, 4], [4, 1]],
                    vec![[0, 2], [1, 2], [1, 0]],
                ],
                trims: [
                    vec![
                        (3, false, SurfaceIso::NotIso),
                        (4, false, SurfaceIso::East),
                        (1, false, SurfaceIso::North),
                        (2, false, SurfaceIso::West),
                        (0, false, SurfaceIso::South),
                    ],
                    vec![
                        (2, false, SurfaceIso::South),
                        (0, false, SurfaceIso::East),
                        (1, true, SurfaceIso::NotIso),
                    ],
                ],
            },
            Case {
                split: Brep::try_split_rectangular_surface_face_east_north,
                side_parameters: [2.0, 7.0],
                cut_endpoints: [point(10.0, 2.0, 0.0), point(7.0, 10.0, 0.0)],
                vertices: [
                    vec![sw, se, nw, point(10.0, 2.0, 0.0), point(7.0, 10.0, 0.0)],
                    vec![ne, point(10.0, 2.0, 0.0), point(7.0, 10.0, 0.0)],
                ],
                edges: [
                    vec![[0, 1], [1, 3], [2, 0], [3, 4], [4, 2]],
                    vec![[0, 2], [1, 2], [1, 0]],
                ],
                trims: [
                    vec![
                        (3, false, SurfaceIso::NotIso),
                        (4, false, SurfaceIso::North),
                        (2, false, SurfaceIso::West),
                        (0, false, SurfaceIso::South),
                        (1, false, SurfaceIso::East),
                    ],
                    vec![
                        (2, false, SurfaceIso::East),
                        (0, false, SurfaceIso::North),
                        (1, true, SurfaceIso::NotIso),
                    ],
                ],
            },
            Case {
                split: Brep::try_split_rectangular_surface_face_north_west,
                side_parameters: [8.0, 3.0],
                cut_endpoints: [point(8.0, 10.0, 0.0), point(0.0, 3.0, 0.0)],
                vertices: [
                    vec![sw, se, ne, point(8.0, 10.0, 0.0), point(0.0, 3.0, 0.0)],
                    vec![nw, point(8.0, 10.0, 0.0), point(0.0, 3.0, 0.0)],
                ],
                edges: [
                    vec![[0, 1], [1, 2], [2, 3], [3, 4], [4, 0]],
                    vec![[0, 2], [1, 2], [1, 0]],
                ],
                trims: [
                    vec![
                        (3, false, SurfaceIso::NotIso),
                        (4, false, SurfaceIso::West),
                        (0, false, SurfaceIso::South),
                        (1, false, SurfaceIso::East),
                        (2, false, SurfaceIso::North),
                    ],
                    vec![
                        (2, false, SurfaceIso::North),
                        (0, false, SurfaceIso::West),
                        (1, true, SurfaceIso::NotIso),
                    ],
                ],
            },
            Case {
                split: Brep::try_split_rectangular_surface_face_west_south,
                side_parameters: [8.0, 3.0],
                cut_endpoints: [point(0.0, 8.0, 0.0), point(3.0, 0.0, 0.0)],
                vertices: [
                    vec![sw, point(0.0, 8.0, 0.0), point(3.0, 0.0, 0.0)],
                    vec![se, ne, nw, point(0.0, 8.0, 0.0), point(3.0, 0.0, 0.0)],
                ],
                edges: [
                    vec![[0, 2], [1, 2], [1, 0]],
                    vec![[0, 1], [1, 2], [2, 3], [3, 4], [4, 0]],
                ],
                trims: [
                    vec![
                        (2, false, SurfaceIso::West),
                        (0, false, SurfaceIso::South),
                        (1, true, SurfaceIso::NotIso),
                    ],
                    vec![
                        (3, false, SurfaceIso::NotIso),
                        (4, false, SurfaceIso::South),
                        (0, false, SurfaceIso::East),
                        (1, false, SurfaceIso::North),
                        (2, false, SurfaceIso::West),
                    ],
                ],
            },
        ];

        for case in cases {
            let surface = NurbsSurface::try_bilinear([sw, se, ne, nw])
                .unwrap()
                .try_reparameterized(0.0..=10.0, 0.0..=10.0)
                .unwrap();
            let length = case.cut_endpoints[0]
                .distance_to(case.cut_endpoints[1])
                .unwrap();
            let curve = NurbsCurve::try_new(
                1,
                case.cut_endpoints.to_vec(),
                vec![0.0, 0.0, length, length],
            )
            .unwrap();
            let pieces = (case.split)(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                case.side_parameters,
                curve,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let mut total_area = 0.0;
            for (index, piece) in pieces.iter().enumerate() {
                assert!(piece.faces()[0].is_reversed());
                assert_eq!(piece.faces()[0].surface(), &surface);
                assert_eq!(
                    piece
                        .vertices()
                        .iter()
                        .map(|vertex| vertex.point())
                        .collect::<Vec<_>>(),
                    case.vertices[index]
                );
                assert_eq!(
                    piece
                        .edges()
                        .iter()
                        .map(BrepEdge::vertices)
                        .collect::<Vec<_>>(),
                    case.edges[index]
                );
                assert_eq!(
                    piece.faces()[0].loops()[0]
                        .trims()
                        .iter()
                        .map(|trim| { (trim.edge().unwrap(), trim.is_reversed_3d(), trim.iso()) })
                        .collect::<Vec<_>>(),
                    case.trims[index]
                );
                let cut_edge = piece.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .find(|trim| trim.iso() == SurfaceIso::NotIso)
                    .and_then(BrepTrim::edge)
                    .unwrap();
                assert_eq!(piece.edges()[cut_edge].curve().domain(), 0.0..=length);
                total_area += piece.area(Tolerance::DEFAULT).unwrap();
                piece.tessellate(2, Tolerance::DEFAULT).unwrap();
            }
            assert!(Tolerance::DEFAULT.approx_eq(total_area, 100.0));
        }
    }

    #[test]
    fn rectangular_surface_corner_splits_match_rhino_topology() {
        use RectangularSurfaceCorner::{NorthEast, NorthWest, SouthEast, SouthWest};

        let cases = [
            (
                SouthWest,
                [10.0, 7.0],
                [
                    vec![[0, 1], [1, 2], [0, 2]],
                    vec![[1, 2], [2, 0], [0, 3], [3, 1]],
                ],
                [true, false],
            ),
            (
                SouthWest,
                [6.0, 10.0],
                [
                    vec![[1, 0], [0, 2], [2, 1]],
                    vec![[0, 1], [1, 2], [2, 3], [0, 3]],
                ],
                [false, true],
            ),
            (
                SouthEast,
                [4.0, 10.0],
                [
                    vec![[0, 1], [2, 0], [1, 3], [3, 2]],
                    vec![[0, 1], [1, 2], [0, 2]],
                ],
                [false, true],
            ),
            (
                SouthEast,
                [0.0, 6.0],
                [
                    vec![[0, 1], [1, 2], [2, 0]],
                    vec![[0, 1], [1, 2], [2, 3], [0, 3]],
                ],
                [false, true],
            ),
            (
                NorthEast,
                [0.0, 3.0],
                [
                    vec![[0, 1], [1, 2], [2, 3], [3, 0]],
                    vec![[0, 1], [1, 2], [0, 2]],
                ],
                [false, true],
            ),
            (
                NorthEast,
                [4.0, 0.0],
                [
                    vec![[0, 3], [1, 2], [2, 0], [1, 3]],
                    vec![[0, 1], [1, 2], [2, 0]],
                ],
                [true, false],
            ),
            (
                NorthWest,
                [6.0, 0.0],
                [
                    vec![[0, 2], [1, 0], [1, 2]],
                    vec![[0, 1], [1, 2], [2, 3], [3, 0]],
                ],
                [true, false],
            ),
            (
                NorthWest,
                [10.0, 4.0],
                [
                    vec![[0, 1], [1, 3], [2, 0], [2, 3]],
                    vec![[0, 1], [1, 2], [2, 0]],
                ],
                [true, false],
            ),
            (
                SouthWest,
                [10.0, 10.0],
                [vec![[0, 1], [1, 2], [0, 2]], vec![[1, 2], [2, 0], [0, 1]]],
                [true, false],
            ),
            (
                SouthEast,
                [0.0, 10.0],
                [vec![[0, 1], [1, 2], [0, 2]], vec![[0, 1], [2, 0], [1, 2]]],
                [true, false],
            ),
        ];

        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        for (corner, destination, expected_edges, expected_cut_reversals) in cases {
            let start = match corner {
                SouthWest => [0.0, 0.0],
                SouthEast => [10.0, 0.0],
                NorthEast => [10.0, 10.0],
                NorthWest => [0.0, 10.0],
            };
            let endpoints = [
                surface.evaluate(start[0], start[1]).unwrap(),
                surface.evaluate(destination[0], destination[1]).unwrap(),
            ];
            let length = endpoints[0].distance_to(endpoints[1]).unwrap();
            let curve =
                NurbsCurve::try_new(1, endpoints.to_vec(), vec![0.0, 0.0, length, length]).unwrap();
            let pieces = Brep::try_split_rectangular_surface_face_from_corner(
                surface.clone(),
                0.0..=10.0,
                0.0..=10.0,
                RectangularSurfaceCornerCut::new(
                    corner,
                    Point2::try_new(destination[0], destination[1]).unwrap(),
                ),
                curve,
                true,
                Tolerance::DEFAULT,
            )
            .unwrap();
            let mut total_area = 0.0;
            for ((piece, expected_edges), expected_cut_reversal) in pieces
                .iter()
                .zip(expected_edges)
                .zip(expected_cut_reversals)
            {
                assert!(piece.faces()[0].is_reversed());
                assert_eq!(piece.faces()[0].surface(), &surface);
                assert_eq!(
                    piece
                        .edges()
                        .iter()
                        .map(BrepEdge::vertices)
                        .collect::<Vec<_>>(),
                    expected_edges
                );
                let cut = piece.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .find(|trim| trim.iso() == SurfaceIso::NotIso)
                    .unwrap();
                assert_eq!(cut.is_reversed_3d(), expected_cut_reversal);
                assert_eq!(
                    piece.faces()[0]
                        .rectangular_trim_bounds(Tolerance::DEFAULT)
                        .unwrap(),
                    None
                );
                total_area += piece.area(Tolerance::DEFAULT).unwrap();
                piece.tessellate(2, Tolerance::DEFAULT).unwrap();
            }
            assert!(Tolerance::DEFAULT.approx_eq(total_area, 100.0));
        }

        let boundary_curve = NurbsCurve::try_new(
            1,
            vec![point(0.0, 0.0, 0.0), point(5.0, 0.0, 0.0)],
            vec![0.0, 0.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(matches!(
            Brep::try_split_rectangular_surface_face_from_corner(
                surface,
                0.0..=10.0,
                0.0..=10.0,
                RectangularSurfaceCornerCut::new(SouthWest, Point2::try_new(5.0, 0.0).unwrap(),),
                boundary_curve,
                false,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidBrepTopology { .. })
        ));
    }

    #[test]
    fn rectangular_surface_face_uses_exact_seam_and_singular_topology() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();

        let cylinder = Brep::try_surface_face(
            NurbsSurface::try_cylinder(frame, 2.0, 0.0, 3.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(cylinder.vertices().len(), 2);
        assert_eq!(cylinder.edges().len(), 3);
        assert_eq!(
            cylinder.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.trim_type(), trim.is_reversed_3d()))
                .collect::<Vec<_>>(),
            vec![
                (Some(0), BrepTrimType::Boundary, false),
                (Some(1), BrepTrimType::Seam, false),
                (Some(2), BrepTrimType::Boundary, false),
                (Some(1), BrepTrimType::Seam, true),
            ]
        );
        assert_eq!(cylinder.edges()[0].vertices(), [0, 0]);
        assert_eq!(cylinder.edges()[1].vertices(), [0, 1]);
        assert_eq!(cylinder.edges()[2].vertices(), [1, 1]);

        let sphere_surface = NurbsSurface::try_sphere(frame, 2.0).unwrap();
        let sphere = Brep::try_surface_face(sphere_surface.clone(), Tolerance::DEFAULT).unwrap();
        assert_eq!(sphere.vertices().len(), 2);
        assert_eq!(sphere.edges().len(), 1);
        assert_eq!(
            sphere.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.trim_type(), trim.is_reversed_3d()))
                .collect::<Vec<_>>(),
            vec![
                (None, BrepTrimType::Singular, false),
                (Some(0), BrepTrimType::Seam, false),
                (None, BrepTrimType::Singular, false),
                (Some(0), BrepTrimType::Seam, true),
            ]
        );
        let sphere_u = sphere_surface.domain_u();
        let sphere_v = sphere_surface.domain_v();
        let hemisphere = Brep::try_rectangular_surface_face(
            sphere_surface,
            *sphere_u.start()..=*sphere_u.end(),
            *sphere_v.start()..=0.0,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(
            hemisphere.area(Tolerance::DEFAULT).unwrap(),
            8.0 * std::f64::consts::PI
        ));

        let torus = Brep::try_surface_face(
            NurbsSurface::try_torus(frame, 4.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(torus.vertices().len(), 1);
        assert_eq!(torus.edges().len(), 2);
        assert!(
            torus.faces()[0].loops()[0]
                .trims()
                .iter()
                .all(|trim| trim.trim_type() == BrepTrimType::Seam)
        );
        assert_eq!(
            torus.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| (trim.edge(), trim.is_reversed_3d()))
                .collect::<Vec<_>>(),
            vec![
                (Some(0), false),
                (Some(1), false),
                (Some(0), true),
                (Some(1), true),
            ]
        );
    }

    #[test]
    fn exact_box_has_shared_closed_oriented_topology() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_box(
            frame,
            [[-1.0, 2.0], [-2.0, 3.0], [0.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert_eq!(brep.vertices().len(), 8);
        assert_eq!(brep.edges().len(), 12);
        assert_eq!(brep.faces().len(), 6);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert!((0..12).all(|edge| brep.edge_use_count(edge) == Some(2)));
        assert_eq!(brep.bounds().min(), point(-1.0, -2.0, 0.0));
        assert_eq!(brep.bounds().max(), point(2.0, 3.0, 4.0));
        let mesh = brep.tessellate(1, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.triangles().len(), 12);
        assert!(mesh.topology().is_solid());
        assert!(Tolerance::DEFAULT.approx_eq(mesh.signed_volume().unwrap(), 60.0));

        let center = point(0.5, 0.5, 2.0);
        for face in brep.faces() {
            assert!(!face.is_reversed());
            assert_eq!(face.loops().len(), 1);
            assert_eq!(face.loops()[0].loop_type(), BrepLoopType::Outer);
            assert_eq!(face.loops()[0].trims().len(), 4);
            let midpoint = face.surface().evaluate(0.5, 0.5).unwrap();
            let outward = center.vector_to(midpoint).unwrap();
            let normal = face
                .surface()
                .normal_at(0.5, 0.5, Tolerance::DEFAULT)
                .unwrap();
            assert!(outward.dot(normal.as_vector()).unwrap() > 0.0);
            assert!(
                !face
                    .surface()
                    .tessellate(1, Tolerance::DEFAULT)
                    .unwrap()
                    .triangles()
                    .is_empty()
            );
        }
    }

    #[test]
    fn face_boundaries_follow_join_order_and_exclude_seams() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let boundary = box_brep.face_boundary_curve_components(2).unwrap();
        assert_eq!(boundary.len(), 1);
        assert_eq!(
            boundary[0],
            vec![
                box_brep.edges()[8].curve().reversed().unwrap(),
                box_brep.edges()[0].curve().clone(),
                box_brep.edges()[9].curve().clone(),
                box_brep.edges()[4].curve().reversed().unwrap(),
            ]
        );
        for pair in boundary[0].windows(2) {
            assert_eq!(
                pair[0].evaluate(*pair[0].domain().end()).unwrap(),
                pair[1].evaluate(*pair[1].domain().start()).unwrap()
            );
        }
        assert_eq!(
            boundary[0]
                .last()
                .unwrap()
                .evaluate(*boundary[0].last().unwrap().domain().end())
                .unwrap(),
            boundary[0][0]
                .evaluate(*boundary[0][0].domain().start())
                .unwrap()
        );

        let outer = Polyline3::try_new(
            vec![
                point(10.0, 0.0, 0.0),
                point(18.0, 0.0, 0.0),
                point(18.0, 6.0, 0.0),
                point(10.0, 6.0, 0.0),
                point(10.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let holes = [
            Circle3::try_new(point(12.0, 3.0, 0.0), 1.0, normal, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap(),
            Circle3::try_new(point(16.0, 3.0, 0.0), 0.5, normal, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        ];
        let holed = Brep::try_planar_face_with_holes(&outer, &holes, Tolerance::DEFAULT).unwrap();
        let boundaries = holed.face_boundary_curve_components(0).unwrap();
        assert_eq!(boundaries.len(), 3);
        assert_eq!(boundaries[0], vec![holed.edges()[2].curve().clone()]);
        assert_eq!(boundaries[1], vec![holed.edges()[1].curve().clone()]);
        assert_eq!(boundaries[2], vec![holed.edges()[0].curve().clone()]);

        let cylinder = Brep::try_extruded_curve(
            &holes[0],
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 5.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let wall_boundaries = cylinder.face_boundary_curve_components(0).unwrap();
        assert_eq!(wall_boundaries.len(), 2);
        assert_eq!(
            wall_boundaries[0],
            vec![cylinder.edges()[1].curve().clone()]
        );
        assert_eq!(
            wall_boundaries[1],
            vec![cylinder.edges()[0].curve().clone()]
        );
        assert_eq!(
            cylinder.face_boundary_curve_components(3),
            Err(GeometryError::BrepFaceIndexOutOfRange {
                face: 3,
                face_count: 3,
            })
        );
    }

    #[test]
    fn exploding_faces_compacts_topology_and_reclassifies_only_mated_edges() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut box_brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        box_brep.vertices[0].tolerance = 7.0;
        let box_parts = box_brep.explode_faces(Tolerance::DEFAULT).unwrap();
        assert_eq!(box_parts.len(), 6);
        for (part, source_face) in box_parts.iter().zip(box_brep.faces()) {
            assert_eq!(part.vertices().len(), 4);
            assert_eq!(part.edges().len(), 4);
            assert_eq!(part.faces().len(), 1);
            assert!(!part.is_closed());
            assert_eq!(part.faces()[0].surface(), source_face.surface());
            assert_eq!(part.faces()[0].is_reversed(), source_face.is_reversed());
            assert!(
                part.vertices()
                    .iter()
                    .all(|vertex| vertex.tolerance() == 0.0)
            );
            assert!(
                part.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .all(|trim| trim.trim_type() == BrepTrimType::Boundary)
            );
        }

        let circle = Circle3::try_new(
            point(10.0, 0.0, 0.0),
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let cylinder = Brep::try_extruded_curve(
            &circle,
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 5.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cylinder_parts = cylinder.explode_faces(Tolerance::DEFAULT).unwrap();
        assert_eq!(cylinder_parts.len(), 3);
        assert_eq!(cylinder_parts[0].vertices().len(), 2);
        assert_eq!(cylinder_parts[0].edges().len(), 3);
        assert_eq!(cylinder_parts[1].vertices().len(), 1);
        assert_eq!(cylinder_parts[1].edges().len(), 1);
        assert_eq!(cylinder_parts[2].vertices().len(), 1);
        assert_eq!(cylinder_parts[2].edges().len(), 1);
        assert_eq!(
            cylinder_parts[0].faces()[0].loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::trim_type)
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Boundary,
                BrepTrimType::Seam,
                BrepTrimType::Boundary,
                BrepTrimType::Seam,
            ]
        );
        for cap in &cylinder_parts[1..] {
            assert_eq!(
                cap.faces()[0].loops()[0].trims()[0].trim_type(),
                BrepTrimType::Boundary
            );
        }
    }

    #[test]
    fn face_subsets_retain_requested_face_order_and_only_surviving_adjacency() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut box_brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        box_brep.vertices[0].tolerance = 7.0;

        let adjacent = box_brep
            .duplicate_faces(&[2, 0], Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(adjacent.faces().len(), 2);
        assert_eq!(adjacent.edges().len(), 7);
        assert_eq!(adjacent.vertices().len(), 6);
        assert_eq!(adjacent.faces()[0].surface(), box_brep.faces()[2].surface());
        assert_eq!(adjacent.faces()[1].surface(), box_brep.faces()[0].surface());
        assert_eq!(
            adjacent
                .faces()
                .iter()
                .flat_map(|face| face.loops()[0].trims())
                .filter(|trim| trim.trim_type() == BrepTrimType::Mated)
                .count(),
            2
        );
        assert!(
            adjacent
                .vertices()
                .iter()
                .all(|vertex| vertex.tolerance() == 0.0)
        );

        let remainder = box_brep
            .sub_brep(&[1, 3, 4, 5], Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(remainder.faces().len(), 4);
        assert_eq!(
            remainder.faces()[0].surface(),
            box_brep.faces()[1].surface()
        );
        assert!(
            remainder
                .vertices()
                .iter()
                .any(|vertex| vertex.point() == box_brep.vertices()[0].point()
                    && vertex.tolerance() == 7.0)
        );
        assert!(
            remainder
                .faces()
                .iter()
                .flat_map(|face| face.loops()[0].trims())
                .any(|trim| trim.trim_type() == BrepTrimType::Boundary)
        );

        assert_eq!(
            box_brep.duplicate_faces(&[], Tolerance::DEFAULT),
            Err(GeometryError::EmptyBrepFaceSubset)
        );
        assert_eq!(
            box_brep.duplicate_faces(&[6], Tolerance::DEFAULT),
            Err(GeometryError::BrepFaceIndexOutOfRange {
                face: 6,
                face_count: 6,
            })
        );
        assert_eq!(
            box_brep.duplicate_faces(&[1, 1], Tolerance::DEFAULT),
            Err(GeometryError::DuplicateBrepFaceIndex { face: 1 })
        );
    }

    #[test]
    fn wireframe_emits_topology_once_then_trimmed_interior_isocurves() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [0.0, 5.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (density, expected) in [(-1, 12), (0, 12), (1, 24), (2, 24)] {
            let wires = box_brep
                .wireframe_curves(density, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(wires.len(), expected);
            assert_eq!(
                &wires[..box_brep.edges().len()],
                &box_brep
                    .edges()
                    .iter()
                    .map(|edge| edge.curve().clone())
                    .collect::<Vec<_>>()
            );
        }

        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let outer = Circle3::try_new(point(0.0, 0.0, 0.0), 5.0, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let hole = Circle3::try_new(point(0.0, 0.0, 0.0), 2.0, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let ring = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
        assert_eq!(
            ring.wireframe_curves(-1, Tolerance::DEFAULT).unwrap().len(),
            2
        );
        let wires = ring.wireframe_curves(1, Tolerance::DEFAULT).unwrap();
        assert_eq!(wires.len(), 6);
        assert_eq!(
            wires
                .iter()
                .skip(2)
                .filter(|curve| Tolerance::DEFAULT
                    .approx_eq(curve.length(Tolerance::DEFAULT).unwrap(), 3.0))
                .count(),
            4
        );
    }

    #[test]
    fn oriented_box_vertices_follow_the_supplied_frame() {
        let origin = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, -1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [-1.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(brep.vertices()[0].point(), point(2.0, 2.0, 3.0));
        assert_eq!(brep.vertices()[7].point(), point(-3.0, 4.0, 0.0));
        assert!(brep.is_solid());
    }

    #[test]
    fn polygon_meshing_preserves_box_quads_and_watertight_topology() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_box(
            frame,
            [[0.0, 2.0], [0.0, 3.0], [0.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mesh = brep
            .polygon_mesh(0.5, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(mesh.vertices().len(), 24);
        assert_eq!(mesh.face_count(), 6);
        assert!(mesh.faces().iter().all(|face| face.is_quad()));
        assert!(mesh.topology().is_solid());

        assert_eq!(
            brep.polygon_mesh(-0.1, false, false, Tolerance::DEFAULT),
            Err(GeometryError::InvalidMeshDensity(-0.1))
        );
    }

    #[test]
    fn mesh_to_nurbs_matches_rhino_triangle_surfaces_and_parameter_domains() {
        let source = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let untrimmed = Brep::try_from_mesh(&source, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(untrimmed.vertices().len(), 3);
        assert_eq!(untrimmed.edges().len(), 3);
        assert!(
            untrimmed
                .edges()
                .iter()
                .all(|edge| edge.curve().domain() == (0.0..=1.0))
        );
        let face = &untrimmed.faces()[0];
        assert_eq!(face.surface().degree_u(), 1);
        assert_eq!(face.surface().degree_v(), 1);
        assert_eq!(face.surface().domain_u(), 0.0..=4.0);
        assert_eq!(face.surface().domain_v(), 0.0..=5.0);
        assert_eq!(face.loops()[0].trims().len(), 4);
        assert_eq!(
            face.loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::trim_type)
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Boundary,
                BrepTrimType::Boundary,
                BrepTrimType::Boundary,
                BrepTrimType::Singular,
            ]
        );
        assert_eq!(face.loops()[0].trims()[3].iso(), SurfaceIso::West);
        assert_eq!(
            face.surface().evaluate(0.0, 0.0).unwrap(),
            point(0.0, 0.0, 0.0)
        );
        assert_eq!(
            face.surface().evaluate(4.0, 0.0).unwrap(),
            point(4.0, 0.0, 0.0)
        );
        assert_eq!(
            face.surface().evaluate(4.0, 5.0).unwrap(),
            point(0.0, 3.0, 0.0)
        );
        assert_eq!(
            face.surface().evaluate(0.0, 5.0).unwrap(),
            point(0.0, 0.0, 0.0)
        );

        let trimmed = Brep::try_from_mesh(&source, true, Tolerance::DEFAULT).unwrap();
        let face = &trimmed.faces()[0];
        assert_eq!(face.loops()[0].trims().len(), 3);
        assert_eq!(face.loops()[0].trims()[2].iso(), SurfaceIso::NotIso);
        assert_eq!(
            face.surface().evaluate(0.0, 5.0).unwrap(),
            point(-4.0, 3.0, 0.0)
        );
        for brep in [&untrimmed, &trimmed] {
            assert!((brep.area(Tolerance::DEFAULT).unwrap() - 6.0).abs() < 1.0e-12);
            let mesh = brep.tessellate(1, Tolerance::DEFAULT).unwrap();
            assert_eq!(mesh.face_count(), 1);
            assert!(mesh.faces()[0].is_triangle());
        }

        let with_unused = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(9.0, 9.0, 9.0),
            ],
            vec![[2, 1, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let compacted = Brep::try_from_mesh(&with_unused, true, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            compacted
                .vertices()
                .iter()
                .map(|vertex| vertex.point())
                .collect::<Vec<_>>(),
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
            ]
        );
    }

    #[test]
    fn mesh_to_nurbs_preserves_warped_quads_and_closed_shared_topology() {
        let warped = TriangleMesh::try_new_faces(
            vec![
                point(10.0, 0.0, 0.0),
                point(14.0, 0.0, 0.0),
                point(14.0, 3.0, 2.0),
                point(10.0, 3.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let warped_brep = Brep::try_from_mesh(&warped, true, Tolerance::DEFAULT).unwrap();
        let surface = warped_brep.faces()[0].surface();
        assert_eq!(surface.domain_u(), 0.0..=20.0_f64.sqrt());
        assert_eq!(surface.domain_v(), 0.0..=13.0_f64.sqrt());
        assert_eq!(
            surface
                .evaluate(*surface.domain_u().end(), *surface.domain_v().end())
                .unwrap(),
            point(14.0, 3.0, 2.0)
        );

        let tetrahedron = TriangleMesh::try_new(
            vec![
                point(20.0, 0.0, 0.0),
                point(24.0, 0.0, 0.0),
                point(20.0, 4.0, 0.0),
                point(20.0, 0.0, 4.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for trim_triangles in [false, true] {
            let brep =
                Brep::try_from_mesh(&tetrahedron, trim_triangles, Tolerance::DEFAULT).unwrap();
            assert_eq!(brep.vertices().len(), 4);
            assert_eq!(brep.edges().len(), 6);
            assert_eq!(brep.faces().len(), 4);
            assert!(brep.is_solid());
            assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 64.0 / 6.0).abs() < 1.0e-10);
        }
    }

    #[test]
    fn exact_planar_face_retains_concave_and_rational_trim_boundaries() {
        let profile = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 2.0),
                point(3.0, 0.0, 2.0),
                point(3.0, 1.0, 2.0),
                point(1.0, 1.0, 2.0),
                point(1.0, 3.0, 2.0),
                point(0.0, 3.0, 2.0),
                point(0.0, 0.0, 2.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let face = Brep::try_planar_face(&profile, Tolerance::DEFAULT).unwrap();

        assert_eq!(face.vertices().len(), 1);
        assert_eq!(face.edges().len(), 1);
        assert_eq!(face.faces().len(), 1);
        assert_eq!(face.edge_use_count(0), Some(1));
        assert!(face.is_manifold());
        assert!(!face.is_closed());
        assert!(!face.is_solid());
        assert_eq!(face.edges()[0].curve(), &profile);
        let trim = &face.faces()[0].loops()[0].trims()[0];
        assert_eq!(trim.trim_type(), BrepTrimType::Boundary);
        assert_eq!(trim.iso(), SurfaceIso::NotIso);
        assert!((face.area(Tolerance::DEFAULT).unwrap() - 5.0).abs() < 1.0e-12);
        for samples_per_span in [1, 4] {
            let mesh = face
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            assert!(!mesh.topology().is_closed());
            assert!((mesh.area().unwrap() - 5.0).abs() < 1.0e-12);
        }

        let reversed =
            Brep::try_planar_face(&profile.reversed().unwrap(), Tolerance::DEFAULT).unwrap();
        assert!((reversed.area(Tolerance::DEFAULT).unwrap() - 5.0).abs() < 1.0e-12);
        assert!(
            (reversed
                .tessellate(1, Tolerance::DEFAULT)
                .unwrap()
                .area()
                .unwrap()
                - 5.0)
                .abs()
                < 1.0e-12
        );

        let circle = Circle3::try_new(
            point(8.0, -3.0, 2.0),
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let disk = Brep::try_planar_face(&circle, Tolerance::DEFAULT).unwrap();
        assert_eq!(disk.edges()[0].curve().degree(), 2);
        assert_eq!(disk.edges()[0].curve().knots(), circle.knots());
        let expected_disk_area = std::f64::consts::PI * 4.0;
        let exact_disk_area = disk.area(Tolerance::DEFAULT).unwrap();
        assert!((exact_disk_area - expected_disk_area).abs() / expected_disk_area < 2.0e-12);
        let disk_mesh = disk.tessellate(8, Tolerance::DEFAULT).unwrap();
        assert!((disk_mesh.area().unwrap() - expected_disk_area).abs() / expected_disk_area < 0.01);

        let open = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_planar_face(&open, Tolerance::DEFAULT),
            Err(GeometryError::InvalidPlanarFaceBoundary)
        );
        let nonplanar = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 1.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_planar_face(&nonplanar, Tolerance::DEFAULT),
            Err(GeometryError::InvalidPlanarFaceBoundary)
        );
    }

    #[test]
    fn exact_planar_face_retains_multiple_rational_holes_and_curve_directions() {
        let outer = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 3.0),
                point(12.0, 0.0, 3.0),
                point(12.0, 10.0, 3.0),
                point(0.0, 10.0, 3.0),
                point(0.0, 0.0, 3.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let holes = [
            Circle3::try_new(point(3.0, 5.0, 3.0), 2.0, normal, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap(),
            Circle3::try_new(point(9.0, 5.0, 3.0), 1.0, normal, Tolerance::DEFAULT)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        ];
        let expected_area = 120.0 - 5.0 * std::f64::consts::PI;

        for reverse_outer in [false, true] {
            for reverse_holes in [false, true] {
                let directed_outer = if reverse_outer {
                    outer.reversed().unwrap()
                } else {
                    outer.clone()
                };
                let directed_holes = holes
                    .iter()
                    .map(|hole| {
                        if reverse_holes {
                            hole.reversed().unwrap()
                        } else {
                            hole.clone()
                        }
                    })
                    .collect::<Vec<_>>();
                let brep = Brep::try_planar_face_with_holes(
                    &directed_outer,
                    &directed_holes,
                    Tolerance::DEFAULT,
                )
                .unwrap();

                assert_eq!(brep.vertices().len(), 3);
                assert_eq!(brep.edges().len(), 3);
                assert_eq!(brep.faces().len(), 1);
                assert_eq!(brep.edges()[0].curve(), &directed_outer);
                assert_eq!(brep.edges()[1].curve(), &directed_holes[0]);
                assert_eq!(brep.edges()[2].curve(), &directed_holes[1]);
                assert_eq!(brep.faces()[0].loops().len(), 3);
                assert_eq!(brep.faces()[0].loops()[0].loop_type(), BrepLoopType::Outer);
                assert!(
                    brep.faces()[0].loops()[1..]
                        .iter()
                        .all(|face_loop| face_loop.loop_type() == BrepLoopType::Inner)
                );
                assert!((brep.area(Tolerance::DEFAULT).unwrap() - expected_area).abs() < 2.0e-10);

                let mesh = brep.tessellate(16, Tolerance::DEFAULT).unwrap();
                assert_eq!(mesh.topology().boundary_edge_count(), 192);
                assert!((mesh.area().unwrap() - expected_area).abs() / expected_area < 5.0e-4);
            }
        }
    }

    #[test]
    fn planar_face_holes_must_be_closed_coplanar_disjoint_and_inside() {
        let rectangle = |min_x, min_y, max_x, max_y, z| {
            Polyline3::try_new(
                vec![
                    point(min_x, min_y, z),
                    point(max_x, min_y, z),
                    point(max_x, max_y, z),
                    point(min_x, max_y, z),
                    point(min_x, min_y, z),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap()
            .to_nurbs()
            .unwrap()
        };
        let outer = rectangle(0.0, 0.0, 10.0, 10.0, 0.0);
        let valid = rectangle(2.0, 2.0, 4.0, 4.0, 0.0);
        let open = LineSegment::try_new(
            point(2.0, 2.0, 0.0),
            point(4.0, 2.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let noncoplanar = rectangle(2.0, 2.0, 4.0, 4.0, 1.0);
        let outside = rectangle(8.0, 8.0, 12.0, 12.0, 0.0);
        let overlapping = rectangle(3.0, 3.0, 5.0, 5.0, 0.0);
        let nested = rectangle(2.5, 2.5, 3.5, 3.5, 0.0);

        for holes in [
            vec![open],
            vec![noncoplanar],
            vec![outside],
            vec![valid.clone(), overlapping],
            vec![valid, nested],
        ] {
            assert_eq!(
                Brep::try_planar_face_with_holes(&outer, &holes, Tolerance::DEFAULT),
                Err(GeometryError::InvalidPlanarFaceBoundary)
            );
        }
    }

    #[test]
    fn constrained_trim_tessellation_scales_beyond_the_ear_clip_budget() {
        let rectangle = |min_x, min_y, max_x, max_y| {
            Polyline3::try_new(
                vec![
                    point(min_x, min_y, 0.0),
                    point(max_x, min_y, 0.0),
                    point(max_x, max_y, 0.0),
                    point(min_x, max_y, 0.0),
                    point(min_x, min_y, 0.0),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap()
            .to_nurbs()
            .unwrap()
        };
        let outer = rectangle(0.0, 0.0, 100.0, 100.0);
        let holes = (0..16)
            .flat_map(|row| {
                (0..17).map(move |column| {
                    let min_x = 2.0 + 5.0 * column as Real;
                    let min_y = 2.0 + 5.0 * row as Real;
                    rectangle(min_x, min_y, min_x + 1.0, min_y + 1.0)
                })
            })
            .collect::<Vec<_>>();
        let brep = Brep::try_planar_face_with_holes(&outer, &holes, Tolerance::DEFAULT).unwrap();
        let samples_per_span = 16;
        let mesh = brep
            .tessellate(samples_per_span, Tolerance::DEFAULT)
            .unwrap();
        let expected_boundary_edges = (holes.len() + 1) * 4 * samples_per_span;

        assert!(expected_boundary_edges > MAX_EAR_CLIP_VERTICES);
        assert_eq!(
            mesh.topology().boundary_edge_count(),
            expected_boundary_edges
        );
        assert!((mesh.area().unwrap() - (10_000.0 - holes.len() as Real)).abs() < 1.0e-10);
    }

    #[test]
    fn nonplanar_trim_tessellation_refines_the_surface_and_preserves_a_hole() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 10.0),
            point(0.0, 10.0, 0.0),
        ])
        .unwrap()
        .try_reparameterized(0.0..=10.0, 0.0..=10.0)
        .unwrap();
        let parameter_loops = [
            (
                BrepLoopType::Outer,
                vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
                vec![
                    SurfaceIso::South,
                    SurfaceIso::East,
                    SurfaceIso::North,
                    SurfaceIso::West,
                ],
            ),
            (
                BrepLoopType::Inner,
                vec![[3.0, 3.0], [3.0, 7.0], [7.0, 7.0], [7.0, 3.0]],
                vec![
                    SurfaceIso::InteriorUConstant,
                    SurfaceIso::InteriorVConstant,
                    SurfaceIso::InteriorUConstant,
                    SurfaceIso::InteriorVConstant,
                ],
            ),
        ];
        let mut vertices = Vec::new();
        let mut edges = Vec::new();
        let mut loops = Vec::new();
        for (loop_type, parameters, isos) in parameter_loops {
            let vertex_offset = vertices.len();
            let parameter_points = parameters
                .into_iter()
                .map(|parameter| Point2::try_from(parameter).unwrap())
                .collect::<Vec<_>>();
            for parameter in &parameter_points {
                vertices.push(
                    BrepVertex::try_new(
                        surface.evaluate(parameter.x(), parameter.y()).unwrap(),
                        0.0,
                    )
                    .unwrap(),
                );
            }
            let mut trims = Vec::new();
            for index in 0..parameter_points.len() {
                let next = (index + 1) % parameter_points.len();
                let edge_index = edges.len();
                let edge_vertices = [vertex_offset + index, vertex_offset + next];
                let model_points = edge_vertices.map(|vertex| vertices[vertex].point());
                edges.push(
                    BrepEdge::try_new(
                        edge_vertices,
                        NurbsCurve::try_new(1, model_points.to_vec(), vec![0.0, 0.0, 1.0, 1.0])
                            .unwrap(),
                        0.0,
                    )
                    .unwrap(),
                );
                trims.push(
                    BrepTrim::try_new(
                        edge_vertices,
                        Some(edge_index),
                        false,
                        NurbsCurve2::try_line(parameter_points[index], parameter_points[next])
                            .unwrap(),
                        BrepTrimType::Boundary,
                        isos[index],
                        [0.0, 0.0],
                    )
                    .unwrap(),
                );
            }
            loops.push(BrepLoop::try_new(loop_type, trims).unwrap());
        }
        let brep = Brep::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(surface.clone(), false, loops).unwrap()],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let coarse = brep.tessellate(1, Tolerance::DEFAULT).unwrap();
        let dense = brep.tessellate(4, Tolerance::DEFAULT).unwrap();
        assert_eq!(coarse.topology().boundary_edge_count(), 8);
        assert_eq!(dense.topology().boundary_edge_count(), 32);
        assert!(dense.vertices().len() > coarse.vertices().len());
        let dense_parameters = dense
            .vertices()
            .iter()
            .map(|point| {
                surface
                    .closest_parameters(*point, Tolerance::DEFAULT)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for face in dense.faces() {
            let centroid = face.indices().iter().fold([0.0, 0.0], |mut sum, index| {
                let parameter = dense_parameters[*index as usize];
                sum[0] += parameter.0 / face.vertex_count() as Real;
                sum[1] += parameter.1 / face.vertex_count() as Real;
                sum
            });
            assert!(
                centroid[0] <= 3.0
                    || centroid[0] >= 7.0
                    || centroid[1] <= 3.0
                    || centroid[1] >= 7.0
            );
        }
    }

    #[test]
    fn exact_capped_cylinder_has_mated_rims_and_periodic_seams() {
        let origin = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            origin,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_cylinder(frame, 2.5, 3.0, -4.0, Tolerance::DEFAULT).unwrap();

        assert_eq!(brep.vertices().len(), 4);
        assert_eq!(brep.edges().len(), 5);
        assert_eq!(brep.faces().len(), 3);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert!((0..brep.edges().len()).all(|edge| brep.edge_use_count(edge) == Some(2)));
        assert_eq!(
            brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.trim_type())
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Mated,
                BrepTrimType::Seam,
                BrepTrimType::Mated,
                BrepTrimType::Seam,
            ]
        );
        for cap in &brep.faces()[1..] {
            assert_eq!(
                cap.loops()[0]
                    .trims()
                    .iter()
                    .map(|trim| trim.trim_type())
                    .collect::<Vec<_>>(),
                vec![
                    BrepTrimType::Singular,
                    BrepTrimType::Seam,
                    BrepTrimType::Mated,
                    BrepTrimType::Seam,
                ]
            );
        }
        assert!(!brep.faces()[1].is_reversed());
        assert!(brep.faces()[2].is_reversed());

        let mesh = brep.tessellate(8, Tolerance::DEFAULT).unwrap();
        assert!(
            mesh.topology().is_solid(),
            "cylinder display topology: {:?}",
            mesh.topology()
        );
        let expected_volume = std::f64::consts::PI * 2.5 * 2.5 * 7.0;
        let relative_error =
            (mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume;
        assert!(
            relative_error < 0.01,
            "relative volume error {relative_error}"
        );

        assert!(Brep::try_cylinder(frame, 0.0, 0.0, 1.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_cylinder(frame, 1.0, 2.0, 2.0, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn exact_tube_matches_rhino_extrusion_topology_and_parameter_domains() {
        let base = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            base,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let tube = Brep::try_tube(frame, [1.0, 3.0], 5.0, Tolerance::DEFAULT).unwrap();
        let outer_end = 6.0 * std::f64::consts::PI;
        let inner_end = 8.0 * std::f64::consts::PI;

        assert_eq!(tube.vertices().len(), 4);
        assert_eq!(tube.edges().len(), 6);
        assert_eq!(tube.faces().len(), 4);
        assert!(tube.is_manifold());
        assert!(tube.is_closed());
        assert!(tube.is_solid());
        assert!((0..6).all(|edge| tube.edge_use_count(edge) == Some(2)));
        assert_eq!(tube.vertices()[0].point(), point(1.0, 5.0, 3.0));
        assert_eq!(tube.vertices()[1].point(), point(1.0, 5.0, 8.0));
        assert_eq!(tube.vertices()[2].point(), point(1.0, 3.0, 3.0));
        assert_eq!(tube.vertices()[3].point(), point(1.0, 3.0, 8.0));
        assert_eq!(tube.faces()[0].surface().domain_u(), 0.0..=outer_end);
        assert_eq!(tube.faces()[1].surface().domain_u(), outer_end..=inner_end);
        assert_eq!(tube.faces()[0].surface().domain_v(), 0.0..=5.0);
        assert_eq!(tube.faces()[1].surface().domain_v(), 0.0..=5.0);
        assert_eq!(
            tube.faces()[0]
                .surface()
                .control_point(1, 0)
                .unwrap()
                .point(),
            point(-2.0, 5.0, 3.0)
        );
        assert_eq!(
            tube.faces()[1]
                .surface()
                .control_point(1, 0)
                .unwrap()
                .point(),
            point(2.0, 3.0, 3.0)
        );
        assert!(!tube.faces()[0].is_reversed());
        assert!(!tube.faces()[1].is_reversed());
        assert!(tube.faces()[2].is_reversed());
        assert!(!tube.faces()[3].is_reversed());
        assert_eq!(
            tube.faces()[2]
                .loops()
                .iter()
                .map(BrepLoop::loop_type)
                .collect::<Vec<_>>(),
            vec![BrepLoopType::Outer, BrepLoopType::Inner]
        );
        assert_eq!(
            tube.faces()[3]
                .loops()
                .iter()
                .map(BrepLoop::loop_type)
                .collect::<Vec<_>>(),
            vec![BrepLoopType::Outer, BrepLoopType::Inner]
        );

        let mesh = tube.tessellate(12, Tolerance::DEFAULT).unwrap();
        assert!(mesh.topology().is_solid());
        let expected_volume = std::f64::consts::PI * (3.0_f64.powi(2) - 1.0) * 5.0;
        let relative_error =
            (mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume;
        assert!(
            relative_error < 0.01,
            "relative volume error {relative_error}"
        );

        assert_eq!(
            tube,
            Brep::try_tube(frame, [3.0, 1.0], 5.0, Tolerance::DEFAULT).unwrap()
        );
        assert!(Brep::try_tube(frame, [1.0, 1.0], 5.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_tube(frame, [0.0, 1.0], 5.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_tube(frame, [1.0, 3.0], 0.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_tube(frame, [1.0, Real::INFINITY], 5.0, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn regular_pyramids_match_rhino_wall_topology_and_signed_orientation() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let solid = Brep::try_pyramid(frame, 4, 3.0, 5.0, true, Tolerance::DEFAULT).unwrap();
        assert_eq!(solid.vertices().len(), 5);
        assert_eq!(solid.edges().len(), 8);
        assert_eq!(solid.faces().len(), 5);
        assert!(solid.is_manifold());
        assert!(solid.is_closed());
        assert!(solid.is_solid());
        assert!((0..8).all(|edge| solid.edge_use_count(edge) == Some(2)));
        assert_eq!(solid.vertices()[0].point(), point(3.0, 0.0, 0.0));
        assert_eq!(solid.vertices()[2].point(), point(0.0, 0.0, 5.0));
        let wall = &solid.faces()[0];
        let half_side = 18.0_f64.sqrt() * 0.5;
        let face_height = 29.5_f64.sqrt();
        assert!(Tolerance::DEFAULT.approx_eq(*wall.surface().domain_u().start(), -half_side));
        assert!(Tolerance::DEFAULT.approx_eq(*wall.surface().domain_u().end(), half_side));
        assert!(
            Tolerance::DEFAULT.approx_eq(*wall.surface().domain_v().start(), -face_height / 3.0)
        );
        assert!(
            Tolerance::DEFAULT.approx_eq(*wall.surface().domain_v().end(), 2.0 * face_height / 3.0)
        );
        assert_eq!(wall.loops()[0].trims()[0].iso(), SurfaceIso::South);
        assert!(!wall.is_reversed());
        assert!(solid.faces()[4].is_reversed());
        assert!((solid.signed_volume(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1.0e-10);
        assert!(
            solid
                .tessellate(1, Tolerance::DEFAULT)
                .unwrap()
                .topology()
                .is_solid()
        );

        let open = Brep::try_pyramid(frame, 5, 2.0, 4.0, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(open.faces().len(), 5);
        assert!(!open.is_closed());
        assert!(!open.is_solid());
        assert!(
            open.faces()
                .iter()
                .all(|face| { face.loops()[0].trims()[0].trim_type() == BrepTrimType::Boundary })
        );

        let negative = Brep::try_pyramid(frame, 5, 2.0, -4.0, true, Tolerance::DEFAULT).unwrap();
        assert!(negative.faces()[..5].iter().all(BrepFace::is_reversed));
        assert!(!negative.faces()[5].is_reversed());
        assert!(negative.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);

        for (sides, radius, height) in [
            (2, 1.0, 1.0),
            (MAX_REGULAR_POLYGON_SIDES + 1, 1.0, 1.0),
            (4, 0.0, 1.0),
            (4, 1.0, 0.0),
            (4, Real::INFINITY, 1.0),
        ] {
            assert!(
                Brep::try_pyramid(frame, sides, radius, height, true, Tolerance::DEFAULT).is_err()
            );
        }
    }

    #[test]
    fn regular_truncated_pyramids_match_rhino_lofted_ring_topology() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let solid =
            Brep::try_truncated_pyramid(frame, 4, [3.0, 1.0], 5.0, true, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(solid.vertices().len(), 8);
        assert_eq!(solid.edges().len(), 12);
        assert_eq!(solid.faces().len(), 6);
        assert!(solid.is_manifold());
        assert!(solid.is_closed());
        assert!(solid.is_solid());
        assert_eq!(solid.vertices()[0].point(), point(1.0, 0.0, 5.0));
        assert_eq!(solid.vertices()[1].point(), point(3.0, 0.0, 0.0));
        assert_eq!(solid.edges()[0].vertices(), [0, 1]);
        assert_eq!(solid.edges()[1].vertices(), [1, 2]);
        assert_eq!(solid.edges()[2].vertices(), [2, 3]);
        assert_eq!(solid.faces()[0].surface().domain_u(), 0.0..=29.0_f64.sqrt());
        assert_eq!(
            solid.faces()[0].surface().domain_v(),
            -18.0_f64.sqrt()..=0.0
        );
        assert!(solid.faces()[4].is_reversed());
        assert!(!solid.faces()[5].is_reversed());
        assert!((solid.signed_volume(Tolerance::DEFAULT).unwrap() - 130.0 / 3.0).abs() < 1.0e-9);
        assert!(
            solid
                .tessellate(1, Tolerance::DEFAULT)
                .unwrap()
                .topology()
                .is_solid()
        );

        let open =
            Brep::try_truncated_pyramid(frame, 4, [3.0, 1.0], 5.0, false, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(open.faces().len(), 4);
        assert!(!open.is_closed());
        assert!(!open.is_solid());
        assert!(open.faces().iter().all(|face| {
            let trims = face.loops()[0].trims();
            trims[1].trim_type() == BrepTrimType::Boundary
                && trims[3].trim_type() == BrepTrimType::Boundary
        }));

        let prism =
            Brep::try_truncated_pyramid(frame, 3, [2.0, 2.0], 4.0, true, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(prism.faces()[0].surface().domain_u(), 0.0..=4.0);
        assert_eq!(prism.faces()[0].surface().domain_v(), 0.0..=12.0_f64.sqrt());
        assert!(prism.is_solid());

        let negative =
            Brep::try_truncated_pyramid(frame, 5, [2.0, 0.75], -4.0, true, Tolerance::DEFAULT)
                .unwrap();
        assert!(negative.faces()[..5].iter().all(BrepFace::is_reversed));
        assert!(!negative.faces()[5].is_reversed());
        assert!(negative.faces()[6].is_reversed());
        assert!(negative.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);

        assert!(
            Brep::try_truncated_pyramid(frame, 4, [2.0, 0.0], 4.0, true, Tolerance::DEFAULT,)
                .is_err()
        );
    }

    #[test]
    fn exact_capped_truncated_cone_matches_rhino_shared_rim_topology() {
        let base = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            base,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_truncated_cone(frame, [2.5, 1.5], 7.0, Tolerance::DEFAULT).unwrap();

        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 3);
        assert_eq!(brep.faces().len(), 3);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert!((0..3).all(|edge| brep.edge_use_count(edge) == Some(2)));
        assert_eq!(brep.vertices()[0].point(), point(1.0, 4.5, 3.0));
        assert_eq!(brep.vertices()[1].point(), point(1.0, 3.5, 10.0));
        assert_eq!(brep.edges()[1].curve().domain(), 0.0..=50.0_f64.sqrt());
        assert_eq!(
            brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(BrepTrim::trim_type)
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Mated,
                BrepTrimType::Seam,
                BrepTrimType::Mated,
                BrepTrimType::Seam,
            ]
        );
        assert!(
            brep.faces()[1..]
                .iter()
                .all(|face| face.loops()[0].trims().len() == 1)
        );
        assert!(brep.faces()[1..].iter().all(|face| {
            let trim = &face.loops()[0].trims()[0];
            trim.trim_type() == BrepTrimType::Mated && trim.is_reversed_3d()
        }));
        assert!(brep.faces().iter().all(|face| !face.is_reversed()));

        let mesh = brep.tessellate(12, Tolerance::DEFAULT).unwrap();
        assert!(mesh.topology().is_solid());
        let expected_volume =
            std::f64::consts::PI * 7.0 * (2.5_f64.powi(2) + 2.5 * 1.5 + 1.5_f64.powi(2)) / 3.0;
        let relative_error =
            (mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume;
        assert!(
            relative_error < 0.01,
            "relative volume error {relative_error}"
        );

        let cylinder =
            Brep::try_truncated_cone(frame, [2.0, 2.0], 4.0, Tolerance::DEFAULT).unwrap();
        assert!(cylinder.is_solid());
        assert_eq!(cylinder.vertices().len(), 2);
        assert!(Brep::try_truncated_cone(frame, [0.0, 1.0], 4.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_truncated_cone(frame, [2.0, 1.0], 0.0, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn exact_capped_cone_handles_both_signed_apex_directions() {
        let base = point(1.0, 2.0, 3.0);
        let frame = Frame3::try_from_directions(
            base,
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();

        for height in [7.0, -7.0] {
            let brep = Brep::try_cone(frame, 2.5, height, Tolerance::DEFAULT).unwrap();
            assert_eq!(brep.vertices().len(), 3);
            assert_eq!(brep.edges().len(), 3);
            assert_eq!(brep.faces().len(), 2);
            assert!(brep.is_manifold());
            assert!(brep.is_closed());
            assert!(brep.is_solid());
            assert!((0..3).all(|edge| brep.edge_use_count(edge) == Some(2)));
            assert!(!brep.faces()[0].is_reversed());
            assert_eq!(brep.faces()[1].is_reversed(), height < 0.0);

            let wall_trim_types = brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.trim_type())
                .collect::<Vec<_>>();
            assert_eq!(
                wall_trim_types,
                if height > 0.0 {
                    vec![
                        BrepTrimType::Mated,
                        BrepTrimType::Seam,
                        BrepTrimType::Singular,
                        BrepTrimType::Seam,
                    ]
                } else {
                    vec![
                        BrepTrimType::Singular,
                        BrepTrimType::Seam,
                        BrepTrimType::Mated,
                        BrepTrimType::Seam,
                    ]
                }
            );

            let mesh = brep.tessellate(8, Tolerance::DEFAULT).unwrap();
            assert!(mesh.topology().is_solid());
            let expected_volume = std::f64::consts::PI * 2.5 * 2.5 * height.abs() / 3.0;
            let relative_error =
                (mesh.signed_volume().unwrap() - expected_volume).abs() / expected_volume;
            assert!(
                relative_error < 0.01,
                "height {height} relative volume error {relative_error}"
            );
        }

        assert!(Brep::try_cone(frame, 0.0, 1.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_cone(frame, 1.0, 0.0, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_cone(frame, 1.0, Real::NAN, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn exact_paraboloid_matches_rhino_singular_seam_and_cap_topology() {
        let frame = Frame3::try_from_directions(
            point(1.0, 2.0, 3.0),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let radius: Real = 2.0;
        let height: Real = 4.0;
        let meridian_length = height.hypot(0.5 * radius)
            + 0.5 * radius * (height / (0.5 * radius)).asinh() / (height / (0.5 * radius));

        let open = Brep::try_paraboloid(frame, radius, height, false, Tolerance::DEFAULT).unwrap();
        assert_eq!(open.vertices().len(), 2);
        assert_eq!(open.edges().len(), 2);
        assert_eq!(open.faces().len(), 1);
        assert!(open.is_manifold());
        assert!(!open.is_closed());
        assert!(!open.is_solid());
        assert_eq!(open.vertices()[0].point(), point(1.0, 2.0, 3.0));
        assert_eq!(open.vertices()[1].point(), point(1.0, 4.0, 7.0));
        assert_eq!(open.edges()[0].vertices(), [0, 1]);
        assert_eq!(open.edges()[0].curve().domain(), 0.0..=meridian_length);
        assert_eq!(open.edges()[1].vertices(), [1, 1]);
        assert_eq!(
            open.edges()[1].curve().domain(),
            -std::f64::consts::TAU..=0.0
        );
        assert_eq!(open.edge_use_count(0), Some(2));
        assert_eq!(open.edge_use_count(1), Some(1));
        assert_eq!(
            open.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.trim_type())
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Singular,
                BrepTrimType::Seam,
                BrepTrimType::Boundary,
                BrepTrimType::Seam,
            ]
        );

        let solid = Brep::try_paraboloid(frame, radius, height, true, Tolerance::DEFAULT).unwrap();
        assert_eq!(solid.vertices().len(), 2);
        assert_eq!(solid.edges().len(), 2);
        assert_eq!(solid.faces().len(), 2);
        assert!(solid.is_manifold());
        assert!(solid.is_closed());
        assert!(solid.is_solid());
        assert_eq!(solid.edge_use_count(0), Some(2));
        assert_eq!(solid.edge_use_count(1), Some(2));
        assert!(solid.faces().iter().all(|face| !face.is_reversed()));
        assert_eq!(
            solid.faces()[0].loops()[0].trims()[2].trim_type(),
            BrepTrimType::Mated
        );
        let cap_trim = &solid.faces()[1].loops()[0].trims()[0];
        assert_eq!(cap_trim.trim_type(), BrepTrimType::Mated);
        assert!(cap_trim.is_reversed_3d());
        assert_eq!(solid.faces()[1].surface().domain_u(), -radius..=radius);
        assert_eq!(solid.faces()[1].surface().domain_v(), -radius..=radius);
        let expected_volume = 0.5 * std::f64::consts::PI * radius * radius * height;
        assert!(
            (solid.signed_volume(Tolerance::DEFAULT).unwrap() - expected_volume).abs() < 1.0e-10
        );

        assert!(Brep::try_paraboloid(frame, 0.0, height, false, Tolerance::DEFAULT).is_err());
        assert!(Brep::try_paraboloid(frame, radius, 0.0, true, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn capped_curve_extrusion_retains_exact_profile_and_planar_trim_volume() {
        let origin = point(1.0e12, -2.0e12, 3.0e12);
        let profile = Polyline3::try_new(
            vec![
                origin,
                point(origin.x() + 2.0, origin.y(), origin.z()),
                point(origin.x() + 2.0, origin.y() + 3.0, origin.z()),
                point(origin.x(), origin.y() + 3.0, origin.z()),
                origin,
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let zero = Vector3::try_new(0.0, 0.0, 0.0).unwrap();
        let path = Vector3::try_new(1.0, 2.0, 5.0).unwrap();
        let brep = Brep::try_extruded_curve(&profile, zero, path, Tolerance::DEFAULT).unwrap();

        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 3);
        assert_eq!(brep.faces().len(), 3);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert_eq!(brep.faces()[0].loops()[0].trims().len(), 4);
        assert_eq!(brep.faces()[1].loops()[0].trims().len(), 1);
        assert_eq!(brep.faces()[2].loops()[0].trims().len(), 1);
        assert_eq!(brep.edges()[0].curve().degree(), profile.degree());
        assert_eq!(brep.edges()[0].curve().knots(), profile.knots());
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1.0e-10);
        let display_mesh = brep.tessellate(4, Tolerance::DEFAULT).unwrap();
        assert!(display_mesh.topology().is_solid());
        assert!((display_mesh.signed_volume().unwrap() - 30.0).abs() < 1.0e-10);

        let reversed = profile.reversed().unwrap();
        let reversed_brep =
            Brep::try_extruded_curve(&reversed, zero, path, Tolerance::DEFAULT).unwrap();
        assert!(reversed_brep.is_solid());
        assert!((reversed_brep.signed_volume(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1.0e-10);

        let opposite = path.scaled(-1.0).unwrap();
        let opposite_brep =
            Brep::try_extruded_curve(&profile, zero, opposite, Tolerance::DEFAULT).unwrap();
        assert!(opposite_brep.is_solid());
        assert!((opposite_brep.signed_volume(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1.0e-10);

        let open = LineSegment::try_new(
            origin,
            point(origin.x() + 2.0, origin.y(), origin.z()),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_extruded_curve(&open, zero, path, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCappedExtrusionProfile)
        );
        assert_eq!(
            Brep::try_extruded_curve(
                &profile,
                zero,
                Vector3::try_new(5.0, 0.0, 0.0).unwrap(),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CoplanarCappedExtrusion)
        );
    }

    #[test]
    fn capped_curve_to_point_extrusion_has_exact_singular_solid_topology() {
        let origin = point(1.0e12, -2.0e12, 3.0e12);
        let profile = Polyline3::try_new(
            vec![
                origin,
                point(origin.x() + 2.0, origin.y(), origin.z()),
                point(origin.x() + 2.0, origin.y() + 3.0, origin.z()),
                point(origin.x(), origin.y() + 3.0, origin.z()),
                origin,
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let apex = point(origin.x() + 1.0, origin.y() + 2.0, origin.z() + 5.0);
        let brep = Brep::try_extruded_curve_to_point(&profile, apex, Tolerance::DEFAULT).unwrap();

        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 2);
        assert_eq!(brep.faces().len(), 2);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert!((0..2).all(|edge| brep.edge_use_count(edge) == Some(2)));
        assert_eq!(
            brep.faces()[0].loops()[0]
                .trims()
                .iter()
                .map(|trim| trim.trim_type())
                .collect::<Vec<_>>(),
            vec![
                BrepTrimType::Seam,
                BrepTrimType::Singular,
                BrepTrimType::Seam,
                BrepTrimType::Mated,
            ]
        );
        assert_eq!(
            brep.faces()[1].loops()[0].trims()[0].trim_type(),
            BrepTrimType::Mated
        );
        assert_eq!(brep.edges()[0].curve().degree(), profile.degree());
        assert_eq!(brep.edges()[0].curve().knots(), profile.knots());
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 10.0).abs() < 1.0e-10);
        for samples_per_span in [1, 4] {
            let mesh = brep
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            assert!(mesh.topology().is_solid());
            assert!((mesh.signed_volume().unwrap() - 10.0).abs() < 1.0e-10);
        }

        let reversed = profile.reversed().unwrap();
        let reversed_brep =
            Brep::try_extruded_curve_to_point(&reversed, apex, Tolerance::DEFAULT).unwrap();
        assert!(reversed_brep.is_solid());
        assert!((reversed_brep.signed_volume(Tolerance::DEFAULT).unwrap() - 10.0).abs() < 1.0e-10);

        let opposite_apex = point(origin.x() + 1.0, origin.y() + 2.0, origin.z() - 5.0);
        let opposite_brep =
            Brep::try_extruded_curve_to_point(&profile, opposite_apex, Tolerance::DEFAULT).unwrap();
        assert!(opposite_brep.is_solid());
        assert!((opposite_brep.signed_volume(Tolerance::DEFAULT).unwrap() - 10.0).abs() < 1.0e-10);

        let open = LineSegment::try_new(
            origin,
            point(origin.x() + 2.0, origin.y(), origin.z()),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_extruded_curve_to_point(&open, apex, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCappedExtrusionProfile)
        );
        assert_eq!(
            Brep::try_extruded_curve_to_point(
                &profile,
                point(origin.x() + 1.0, origin.y() + 2.0, origin.z()),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::CoplanarCappedExtrusion)
        );
    }

    #[test]
    fn capped_curve_along_curve_retains_exact_rational_wall_and_path_seam() {
        let profile = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let path = NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(10.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(11.0, 4.0, 2.0), 0.5).unwrap(),
                WeightedPoint3::try_new(point(12.0, 3.0, 5.0), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let brep =
            Brep::try_extruded_curve_along_curve(&profile, &path, Tolerance::DEFAULT).unwrap();

        assert_eq!(brep.vertices().len(), 2);
        assert_eq!(brep.edges().len(), 3);
        assert_eq!(brep.faces().len(), 3);
        assert!(brep.is_manifold());
        assert!(brep.is_closed());
        assert!(brep.is_solid());
        assert_eq!(brep.edges()[0].curve(), &profile);
        assert_eq!(brep.edges()[2].curve().degree(), path.degree());
        assert_eq!(brep.edges()[2].curve().knots(), path.knots());
        assert_eq!(brep.faces()[0].surface().degree_u(), profile.degree());
        assert_eq!(brep.faces()[0].surface().degree_v(), path.degree());
        assert_eq!(brep.faces()[0].surface().knots_v(), path.knots());
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 30.0).abs() < 1.0e-10);
        for samples_per_span in [1, 4] {
            let mesh = brep
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            assert!(mesh.topology().is_solid());
            assert!((mesh.signed_volume().unwrap() - 30.0).abs() < 1.0e-10);
        }

        let reversed_profile = profile.reversed().unwrap();
        let reversed_profile_brep =
            Brep::try_extruded_curve_along_curve(&reversed_profile, &path, Tolerance::DEFAULT)
                .unwrap();
        assert!(reversed_profile_brep.is_solid());
        assert!(
            (reversed_profile_brep
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                - 30.0)
                .abs()
                < 1.0e-10
        );

        let reversed_path = path.reversed().unwrap();
        let reversed_path_brep =
            Brep::try_extruded_curve_along_curve(&profile, &reversed_path, Tolerance::DEFAULT)
                .unwrap();
        assert!(reversed_path_brep.is_solid());
        assert!(
            (reversed_path_brep
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                - 30.0)
                .abs()
                < 1.0e-10
        );

        let open_profile = LineSegment::try_new(
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_extruded_curve_along_curve(&open_profile, &path, Tolerance::DEFAULT,),
            Err(GeometryError::InvalidCappedExtrusionProfile)
        );

        let closed_path = Circle3::try_new(
            point(0.0, 0.0, 0.0),
            1.0,
            UnitVector3::try_new(0.0, 1.0, 0.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_extruded_curve_along_curve(&profile, &closed_path, Tolerance::DEFAULT,),
            Err(GeometryError::InvalidCappedExtrusionPath)
        );

        let coplanar_path = LineSegment::try_new(
            point(10.0, 0.0, 0.0),
            point(12.0, 3.0, 0.0),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        assert_eq!(
            Brep::try_extruded_curve_along_curve(&profile, &coplanar_path, Tolerance::DEFAULT,),
            Err(GeometryError::CoplanarCappedExtrusion)
        );
    }

    #[test]
    fn capped_curve_sweep_uses_evaluated_unclamped_path_endpoints() {
        let profile = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let path = NurbsCurve::try_new(
            2,
            vec![
                point(10.0, 0.0, -1.0),
                point(11.0, 2.0, 1.0),
                point(12.0, 4.0, 5.0),
                point(14.0, 3.0, 8.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        )
        .unwrap();
        let path_start = path.evaluate(*path.domain().start()).unwrap();
        let path_end = path.evaluate(*path.domain().end()).unwrap();
        assert_ne!(path_start, path.control_points()[0].point());
        assert_ne!(path_end, path.control_points()[3].point());

        let brep =
            Brep::try_extruded_curve_along_curve(&profile, &path, Tolerance::DEFAULT).unwrap();
        assert!(brep.is_solid());
        let seam = brep.edges()[2].curve();
        assert!(
            seam.evaluate(*seam.domain().start())
                .unwrap()
                .is_near(brep.vertices()[0].point(), Tolerance::DEFAULT)
        );
        assert!(
            seam.evaluate(*seam.domain().end())
                .unwrap()
                .is_near(brep.vertices()[1].point(), Tolerance::DEFAULT)
        );
        let expected_volume = 6.0 * (path_end.z() - path_start.z()).abs();
        assert!(
            (brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected_volume).abs() < 1.0e-10
        );
    }

    #[test]
    fn capped_circle_to_point_retains_exact_rational_volume() {
        let center = point(8.0, -3.0, 2.0);
        let circle = Circle3::try_new(
            center,
            2.0,
            UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let brep =
            Brep::try_extruded_curve_to_point(&circle, point(10.0, -1.0, 8.0), Tolerance::DEFAULT)
                .unwrap();
        let expected = std::f64::consts::PI * 4.0 * 6.0 / 3.0;
        let relative_error =
            (brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() / expected;
        assert!(relative_error < 2.0e-12, "relative error {relative_error}");
        let mesh = brep.tessellate(8, Tolerance::DEFAULT).unwrap();
        assert!(mesh.topology().is_solid());
        let mesh_relative_error = (mesh.signed_volume().unwrap() - expected).abs() / expected;
        assert!(
            mesh_relative_error < 0.01,
            "mesh relative error {mesh_relative_error}"
        );
    }

    #[test]
    fn concave_planar_extrusion_tessellates_without_losing_boundary_edges() {
        let profile = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(1.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let brep = Brep::try_extruded_curve(
            &profile,
            Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 4.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - 20.0).abs() < 1.0e-12);
        for samples_per_span in [1, 4] {
            let mesh = brep
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            assert!(mesh.topology().is_solid());
            assert!((mesh.signed_volume().unwrap() - 20.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn exact_solid_volume_is_oriented_translation_stable_and_not_faceted() {
        let frame = Frame3::try_from_normal(
            point(1.0e12, -2.0e12, 3.0e12),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_brep = Brep::try_box(
            frame,
            [[-1.0, 2.0], [-2.0, 3.0], [-3.0, 4.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_volume = box_brep.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (box_volume - 105.0).abs() < 1.0e-11,
            "box volume {box_volume}"
        );
        assert!((box_brep.area(Tolerance::DEFAULT).unwrap() - 142.0).abs() < 1.0e-11);

        let cylinder = Brep::try_cylinder(frame, 2.5, -3.0, 4.0, Tolerance::DEFAULT).unwrap();
        let expected_cylinder = std::f64::consts::PI * 2.5 * 2.5 * 7.0;
        let cylinder_error =
            (cylinder.signed_volume(Tolerance::DEFAULT).unwrap() - expected_cylinder).abs()
                / expected_cylinder;
        assert!(cylinder_error < 2.0e-12, "relative error {cylinder_error}");
        let expected_cylinder_area = 2.0 * std::f64::consts::PI * 2.5 * (2.5 + 7.0);
        let cylinder_area_error =
            (cylinder.area(Tolerance::DEFAULT).unwrap() - expected_cylinder_area).abs()
                / expected_cylinder_area;
        assert!(
            cylinder_area_error < 2.0e-12,
            "relative error {cylinder_area_error}"
        );
        let mut open_wall = cylinder.faces[0].clone();
        open_wall.loops[0].trims[0].trim_type = BrepTrimType::Boundary;
        open_wall.loops[0].trims[2].trim_type = BrepTrimType::Boundary;
        let open = Brep::try_new(
            cylinder.vertices.clone(),
            cylinder.edges[..3].to_vec(),
            vec![open_wall],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            open.signed_volume(Tolerance::DEFAULT),
            Err(GeometryError::OpenBrepVolume)
        );

        let cone = Brep::try_cone(frame, 2.5, -7.0, Tolerance::DEFAULT).unwrap();
        let expected_cone = std::f64::consts::PI * 2.5 * 2.5 * 7.0 / 3.0;
        let cone_error =
            (cone.signed_volume(Tolerance::DEFAULT).unwrap() - expected_cone).abs() / expected_cone;
        assert!(cone_error < 2.0e-12, "relative error {cone_error}");
        let expected_cone_area = std::f64::consts::PI * 2.5 * (2.5 + 2.5_f64.hypot(7.0));
        let cone_area_error = (cone.area(Tolerance::DEFAULT).unwrap() - expected_cone_area).abs()
            / expected_cone_area;
        assert!(
            cone_area_error < 2.0e-12,
            "relative error {cone_area_error}"
        );

        let mut reversed_faces = box_brep.faces.clone();
        for face in &mut reversed_faces {
            face.reversed = !face.reversed;
        }
        let reversed = Brep::try_new(
            box_brep.vertices.clone(),
            box_brep.edges.clone(),
            reversed_faces,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!((reversed.signed_volume(Tolerance::DEFAULT).unwrap() + 105.0).abs() < 1.0e-11);
        assert!((reversed.area(Tolerance::DEFAULT).unwrap() - 142.0).abs() < 1.0e-11);
    }

    #[test]
    fn affine_box_transform_preserves_solid_orientation_and_rejects_projection() {
        let origin = point(0.0, 0.0, 0.0);
        let frame = Frame3::try_from_normal(
            origin,
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep = Brep::try_box(
            frame,
            [[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let reflection_scale =
            AffineTransform3::try_nonuniform_scale(origin, [-2.0, 3.0, 4.0]).unwrap();
        let transformed = brep
            .transformed(reflection_scale, Tolerance::DEFAULT)
            .unwrap();

        assert!(transformed.is_solid());
        assert_eq!(transformed.bounds().min(), point(-2.0, 0.0, 0.0));
        assert_eq!(transformed.bounds().max(), point(0.0, 3.0, 4.0));
        assert!((transformed.signed_volume(Tolerance::DEFAULT).unwrap() - 24.0).abs() < 1.0e-12);
        let center = transformed.bounds().center().unwrap();
        for face in transformed.faces() {
            assert!(face.is_reversed());
            let midpoint = face.surface().evaluate(0.5, 0.5).unwrap();
            let toward_face = center.vector_to(midpoint).unwrap();
            let natural_normal = face
                .surface()
                .normal_at(0.5, 0.5, Tolerance::DEFAULT)
                .unwrap();
            assert!(toward_face.dot(natural_normal.as_vector()).unwrap() < 0.0);
        }

        let projection = AffineTransform3::try_nonuniform_scale(origin, [1.0, 1.0, 0.0]).unwrap();
        assert!(brep.transformed(projection, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn box_and_component_constructors_reject_invalid_topology() {
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            Brep::try_box(
                frame,
                [[0.0, 0.0], [0.0, 1.0], [0.0, 1.0]],
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert!(BrepVertex::try_new(point(0.0, 0.0, 0.0), -1.0).is_err());
        let parameter_line = NurbsCurve2::try_line(
            Point2::try_new(0.0, 0.0).unwrap(),
            Point2::try_new(1.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(
            BrepTrim::try_new(
                [0, 1],
                None,
                false,
                parameter_line.clone(),
                BrepTrimType::Singular,
                SurfaceIso::South,
                [0.0, 0.0],
            )
            .is_err()
        );
        assert!(
            BrepTrim::try_new(
                [0, 0],
                None,
                true,
                parameter_line,
                BrepTrimType::Singular,
                SurfaceIso::South,
                [0.0, 0.0],
            )
            .is_err()
        );

        let valid = Brep::try_box(
            frame,
            [[0.0, 1.0], [0.0, 1.0], [0.0, 1.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let mut edges = valid.edges.clone();
        edges[0].vertices[0] = valid.vertices.len();
        assert!(
            Brep::try_new(
                valid.vertices.clone(),
                edges,
                valid.faces.clone(),
                Tolerance::DEFAULT
            )
            .is_err()
        );

        let mut faces = valid.faces.clone();
        faces[0].loops[0].trims[0].vertices[1] = 7;
        assert!(
            Brep::try_new(
                valid.vertices.clone(),
                valid.edges.clone(),
                faces,
                Tolerance::DEFAULT
            )
            .is_err()
        );

        let mut faces = valid.faces.clone();
        faces[0].loops[0].trims[0].trim_type = BrepTrimType::Boundary;
        assert!(
            Brep::try_new(
                valid.vertices.clone(),
                valid.edges.clone(),
                faces,
                Tolerance::DEFAULT
            )
            .is_err()
        );

        let mut faces = valid.faces.clone();
        faces[0].loops[0].trims.reverse();
        assert!(Brep::try_new(valid.vertices, valid.edges, faces, Tolerance::DEFAULT).is_err());
    }

    #[test]
    fn planar_trimmed_face_tessellation_preserves_an_inner_hole() {
        let brep = planar_polygon_brep(&[
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
            ],
            vec![
                point(3.0, 3.0, 0.0),
                point(3.0, 7.0, 0.0),
                point(7.0, 7.0, 0.0),
                point(7.0, 3.0, 0.0),
            ],
        ]);

        assert!(brep.is_manifold());
        assert!(!brep.is_closed());
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - 84.0).abs() < 1.0e-12);
        for samples_per_span in [1, 3] {
            let mesh = brep
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            assert!((mesh.area().unwrap() - 84.0).abs() < 1.0e-12);
            assert_eq!(mesh.topology().boundary_edge_count(), 8 * samples_per_span);
            assert_eq!(mesh.topology().orientation_conflict_edge_count(), 0);
        }
        let polygon_mesh = brep
            .polygon_mesh(0.5, false, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(polygon_mesh.faces().iter().all(|face| face.is_triangle()));
        assert!((polygon_mesh.area().unwrap() - 84.0).abs() < 1.0e-12);
        assert_eq!(polygon_mesh.topology().boundary_edge_count(), 8);
    }

    #[test]
    fn planar_trimmed_face_tessellation_handles_concavity_multiple_holes_and_translation() {
        let offset = 1.0e12;
        let at = |x: Real, y: Real| point(offset + x, -offset + y, 4.0e12);
        let brep = planar_polygon_brep(&[
            vec![
                at(0.0, 0.0),
                at(10.0, 0.0),
                at(10.0, 10.0),
                at(6.0, 10.0),
                at(6.0, 4.0),
                at(0.0, 4.0),
            ],
            vec![at(1.0, 1.0), at(1.0, 3.0), at(3.0, 3.0), at(3.0, 1.0)],
            vec![at(7.0, 5.0), at(7.0, 7.0), at(9.0, 7.0), at(9.0, 5.0)],
        ]);
        let expected_area = 56.0;

        assert!((brep.area(Tolerance::DEFAULT).unwrap() - expected_area).abs() < 1.0e-12);
        for samples_per_span in [1, 2, 5] {
            let mesh = brep
                .tessellate(samples_per_span, Tolerance::DEFAULT)
                .unwrap();
            let area = mesh.area().unwrap();
            assert!(
                (area - expected_area).abs() < 1.0e-6,
                "samples {samples_per_span}, area {area}"
            );
            assert_eq!(mesh.topology().boundary_edge_count(), 14 * samples_per_span);
            assert_eq!(mesh.topology().orientation_conflict_edge_count(), 0);
        }
    }

    #[test]
    fn trimmed_face_isocurves_are_split_exactly_around_polygon_holes() {
        let brep = planar_polygon_brep(&[
            vec![
                point(0.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, 10.0, 0.0),
                point(0.0, 10.0, 0.0),
            ],
            vec![
                point(4.0, 4.0, 0.0),
                point(4.0, 6.0, 0.0),
                point(6.0, 6.0, 0.0),
                point(6.0, 4.0, 0.0),
            ],
        ]);
        let face = &brep.faces()[0];

        let horizontal = face.isocurve_u_segments(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(horizontal.len(), 2);
        assert_eq!(horizontal[0].domain(), 0.0..=0.4);
        assert_eq!(horizontal[1].domain(), 0.6..=1.0);
        for (curve, expected) in horizontal.iter().zip([(0.0, 4.0), (6.0, 10.0)]) {
            assert_eq!(
                curve.evaluate(*curve.domain().start()).unwrap(),
                point(expected.0, 5.0, 0.0)
            );
            assert_eq!(
                curve.evaluate(*curve.domain().end()).unwrap(),
                point(expected.1, 5.0, 0.0)
            );
        }

        let vertical = face.isocurve_v_segments(0.5, Tolerance::DEFAULT).unwrap();
        assert_eq!(vertical.len(), 2);
        assert_eq!(vertical[0].domain(), 0.0..=0.4);
        assert_eq!(vertical[1].domain(), 0.6..=1.0);
        let boundary = face.isocurve_u_segments(0.0, Tolerance::DEFAULT).unwrap();
        assert_eq!(boundary.len(), 1);
        assert_eq!(boundary[0].domain(), 0.0..=1.0);

        assert!(
            face.contains_parameters(0.2, 0.5, Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            face.contains_parameters(0.4, 0.5, Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            !face
                .contains_parameters(0.5, 0.5, Tolerance::DEFAULT)
                .unwrap()
        );
    }

    #[test]
    fn trim_intersection_handles_rational_closed_circle_loops() {
        let center = point(0.0, 0.0, 0.0);
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let outer = Circle3::try_new(center, 5.0, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let inner = Circle3::try_new(center, 2.0, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let brep = Brep::try_planar_face_with_holes(&outer, &[inner], Tolerance::DEFAULT).unwrap();
        let face = &brep.faces()[0];
        let (u, v) = face
            .surface()
            .closest_parameters(center, Tolerance::DEFAULT)
            .unwrap();

        assert!(!face.contains_parameters(u, v, Tolerance::DEFAULT).unwrap());
        let segments = face.isocurve_u_segments(v, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            segments.len(),
            2,
            "center parameters ({u}, {v}), segment domains {:?}",
            segments.iter().map(NurbsCurve::domain).collect::<Vec<_>>()
        );
        let radii = segments
            .iter()
            .flat_map(|curve| {
                [
                    curve.evaluate(*curve.domain().start()).unwrap(),
                    curve.evaluate(*curve.domain().end()).unwrap(),
                ]
            })
            .map(|point| point.x().hypot(point.y()))
            .collect::<Vec<_>>();
        assert!(Tolerance::DEFAULT.approx_eq(radii[0], 5.0));
        assert!(Tolerance::DEFAULT.approx_eq(radii[1], 2.0));
        assert!(Tolerance::DEFAULT.approx_eq(radii[2], 2.0));
        assert!(Tolerance::DEFAULT.approx_eq(radii[3], 5.0));

        assert_eq!(
            brep.closest_face_parameters(center, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        let underlying = brep
            .closest_underlying_face_parameters(center, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(underlying.0, 0);
        assert!(
            !face
                .contains_parameters(underlying.1, underlying.2, Tolerance::DEFAULT)
                .unwrap()
        );
        assert!(
            face.surface()
                .evaluate(underlying.1, underlying.2)
                .unwrap()
                .is_near(center, Tolerance::DEFAULT)
        );
        let target = point(3.0, 0.0, 0.0);
        let closest = brep
            .closest_face_parameters(target, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(closest.0, 0);
        assert!(
            face.contains_parameters(closest.1, closest.2, Tolerance::DEFAULT)
                .unwrap()
        );
    }

    #[test]
    fn trim_intersection_clamps_periodic_parameter_curves() {
        let outer = NurbsCurve::try_control_point_curve_with_closure(
            2,
            vec![
                point(-5.0, 0.0, 0.0),
                point(-3.0, -3.0, 0.0),
                point(0.0, -5.0, 0.0),
                point(3.0, -3.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(3.0, 3.0, 0.0),
                point(0.0, 5.0, 0.0),
                point(-3.0, 3.0, 0.0),
            ],
            ControlPointCurveClosure::Smooth,
        )
        .unwrap();
        assert!(outer.is_periodic());
        let brep = Brep::try_planar_face(&outer, Tolerance::DEFAULT).unwrap();
        let trim_curve = brep.faces()[0].loops()[0].trims()[0].curve();
        assert!(trim_curve.knots()[0] < *trim_curve.domain().start());

        let face = &brep.faces()[0];
        let center = point(0.0, 0.0, 0.0);
        let (u, v) = face
            .surface()
            .closest_parameters(center, Tolerance::DEFAULT)
            .unwrap();
        assert!(face.contains_parameters(u, v, Tolerance::DEFAULT).unwrap());
        assert_eq!(
            face.isocurve_u_segments(v, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            face.isocurve_v_segments(u, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            1
        );
    }
}
