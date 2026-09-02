use spade::{
    ConstrainedDelaunayTriangulation, HasPosition, Point2 as TriangulationPoint2, Triangulation,
};

use crate::nurbs::find_span_in_knots;
use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, LineSegment, NurbsCurve, NurbsCurve2,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrepTrimType {
    Boundary,
    Mated,
    Seam,
    Singular,
}

/// Classification of a trim lying on an underlying surface-domain side.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceIso {
    NotIso,
    South,
    East,
    North,
    West,
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
    /// nonempty knot-span rectangle. Planar trimmed faces use their exact
    /// oriented p-curve boundaries, including subtractive inner loops. The
    /// control geometry is recentered first so large translations do not
    /// degrade either calculation.
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
        let planar_faces = full_domain_faces
            .iter()
            .zip(&centered_surfaces)
            .enumerate()
            .map(|(face_index, (full_domain, surface))| {
                if *full_domain {
                    Ok(None)
                } else {
                    planar_surface_plane(surface, tolerance)?.map_or_else(
                        || Err(GeometryError::UnsupportedBrepTrimArea { face: face_index }),
                        |plane| Ok(Some(plane)),
                    )
                }
            })
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
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (face_index, (face, surface)) in self.faces.iter().zip(&centered_surfaces).enumerate() {
            if full_domain_faces[face_index] {
                for (u_start, u_end) in surface.spans_u() {
                    for (v_start, v_end) in surface.spans_v() {
                        let contribution = integrate_area_patch(
                            surface,
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
        BoundingBox3::from_points(
            self.vertices
                .iter()
                .map(|vertex| vertex.point)
                .chain(self.edges.iter().flat_map(|edge| {
                    edge.curve
                        .control_points()
                        .iter()
                        .map(|control| control.point())
                }))
                .chain(self.faces.iter().flat_map(|face| {
                    face.surface
                        .control_points()
                        .iter()
                        .map(|control| control.point())
                })),
        )
        .expect("a validated B-rep has finite control geometry")
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

    /// Tessellates full rectangular faces and trimmed planar faces.
    ///
    /// Planar trim boundaries are sampled per exact p-curve knot span and
    /// constrained-triangulated in parameter space while preserving every
    /// outer and inner boundary sample for watertight stitching. Nonplanar
    /// general trims remain explicit errors; they are never silently filled
    /// as untrimmed surfaces.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        if samples_per_span == 0 {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            let mesh = if face_covers_full_surface_domain(face, tolerance)? {
                let surface_mesh = face.surface.tessellate(samples_per_span, tolerance)?;
                let mut face_vertices = surface_mesh.vertices().to_vec();
                self.snap_face_boundary_vertices(
                    face,
                    &mut face_vertices,
                    samples_per_span,
                    tolerance,
                )?;
                TriangleMesh::try_new(face_vertices, surface_mesh.triangles().to_vec(), tolerance)?
            } else if planar_surface_plane(&face.surface, tolerance)?.is_some() {
                self.tessellate_planar_trimmed_face(face_index, face, samples_per_span, tolerance)?
            } else {
                return Err(GeometryError::UnsupportedBrepTrimTessellation { face: face_index });
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
            for triangle in mesh.triangles() {
                let mut triangle = [
                    triangle[0]
                        .checked_add(offset)
                        .ok_or(GeometryError::TooManyMeshVertices)?,
                    triangle[1]
                        .checked_add(offset)
                        .ok_or(GeometryError::TooManyMeshVertices)?,
                    triangle[2]
                        .checked_add(offset)
                        .ok_or(GeometryError::TooManyMeshVertices)?,
                ];
                if face.reversed {
                    triangle.swap(1, 2);
                }
                triangles.push(triangle);
            }
        }
        let mesh = TriangleMesh::try_new(vertices, triangles, tolerance)?;
        let topology = mesh.topology();
        if (self.is_closed() && !topology.is_closed()) || (self.is_solid() && !topology.is_solid())
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
            let triangles = triangulate_simple_trim_polygon(&mut parameters)?
                .ok_or(GeometryError::UnsupportedBrepTrimTessellation { face: face_index })?;
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
                SurfaceIso::NotIso => {
                    return invalid("a full-domain B-rep face has a non-isoparametric side");
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
        .map(|control| control.weight())
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
    let mut samples = Vec::new();
    let mut largest_cross = None;
    let mut largest_area = 0.0;
    for (u_start, u_end) in surface.spans_u() {
        let u = u_start * 0.5 + u_end * 0.5;
        for (v_start, v_end) in surface.spans_v() {
            let v = v_start * 0.5 + v_end * 0.5;
            let (point, derivative_u, derivative_v) = surface.evaluate_with_derivatives(u, v)?;
            let cross = derivative_u.cross(derivative_v)?;
            let area = cross.length()?;
            if area > 0.0 {
                samples.push(cross);
                if area > largest_area {
                    largest_area = area;
                    largest_cross = Some((point, cross));
                }
            }
        }
    }
    let Some((point, cross)) = largest_cross else {
        return Ok(None);
    };
    let normal = cross.normalized_nonzero()?;
    for sample in samples {
        let length = sample.length()?;
        if sample.dot(normal.as_vector())? <= tolerance.angular() * length {
            return Ok(None);
        }
    }
    for control in surface.control_points() {
        let distance = point
            .vector_to(control.point())?
            .dot(normal.as_vector())?
            .abs();
        if distance > tolerance.absolute() {
            return Ok(None);
        }
    }
    Ok(Some(PlanarSurfacePlane { point, normal }))
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
    let (coordinate, expected, allowed) = match trim.iso {
        SurfaceIso::NotIso => return Ok(()),
        SurfaceIso::South => (1, *domain_v.start(), tolerance[1]),
        SurfaceIso::East => (0, *domain_u.end(), tolerance[0]),
        SurfaceIso::North => (1, *domain_v.end(), tolerance[1]),
        SurfaceIso::West => (0, *domain_u.start(), tolerance[0]),
    };
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

fn face_covers_full_surface_domain(
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
            SurfaceIso::NotIso => return Ok(false),
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
    if loop_lengths.len() < 2
        || loop_lengths.iter().any(|length| *length < 3)
        || parameters.len() > MAX_CONSTRAINED_TRIM_VERTICES
        || loop_lengths
            .iter()
            .try_fold(0_usize, |total, length| total.checked_add(*length))
            != Some(parameters.len())
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

    let vertices = normalized
        .iter()
        .enumerate()
        .map(|(source_index, point)| TrimTriangulationVertex {
            position: TriangulationPoint2::new(point[0], point[1]),
            source_index,
        })
        .collect::<Vec<_>>();
    let mut triangulation =
        match ConstrainedDelaunayTriangulation::<TrimTriangulationVertex>::bulk_load(vertices) {
            Ok(triangulation) => triangulation,
            Err(_) => return Ok(None),
        };
    if triangulation.num_vertices() != parameters.len() {
        return Ok(None);
    }
    let mut handles = vec![None; parameters.len()];
    for vertex in triangulation.vertices() {
        handles[vertex.data().source_index] = Some(vertex.fix());
    }
    for range in &loop_ranges {
        for index in range.clone() {
            let next = if index + 1 == range.end {
                range.start
            } else {
                index + 1
            };
            let before = triangulation.num_constraints();
            let Some(from) = handles[index] else {
                return Ok(None);
            };
            let Some(to) = handles[next] else {
                return Ok(None);
            };
            if triangulation.try_add_constraint(from, to).is_empty()
                || triangulation.num_constraints() != before + 1
            {
                return Ok(None);
            }
        }
    }
    if triangulation.num_constraints() != parameters.len() {
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
        if cross.abs() <= epsilon
            && point[0] >= start[0].min(end[0]) - epsilon
            && point[0] <= start[0].max(end[0]) + epsilon
            && point[1] >= start[1].min(end[1]) - epsilon
            && point[1] <= start[1].max(end[1]) + epsilon
        {
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

fn normalized_trim_polygon(parameters: &[Point2]) -> Result<Option<Vec<[Real; 2]>>, GeometryError> {
    let Some(origin) = parameters.first() else {
        return Ok(None);
    };
    let relative = parameters
        .iter()
        .map(|point| [point.x() - origin.x(), point.y() - origin.y()])
        .collect::<Vec<_>>();
    if relative.iter().flatten().all(|value| value.is_finite()) {
        let scale = relative
            .iter()
            .flatten()
            .map(|value| value.abs())
            .fold(0.0, Real::max);
        return if scale > 0.0 {
            Ok(Some(
                relative
                    .into_iter()
                    .map(|point| point.map(|value| value / scale))
                    .collect(),
            ))
        } else {
            Ok(None)
        };
    }

    let global_scale = parameters
        .iter()
        .flat_map(|point| [point.x().abs(), point.y().abs()])
        .fold(0.0, Real::max);
    if global_scale == 0.0 {
        return Ok(None);
    }
    let scaled_origin = [origin.x() / global_scale, origin.y() / global_scale];
    let relative = parameters
        .iter()
        .map(|point| {
            [
                point.x() / global_scale - scaled_origin[0],
                point.y() / global_scale - scaled_origin[1],
            ]
        })
        .collect::<Vec<_>>();
    require_finite(
        relative.iter().flatten().copied(),
        "trim triangulation coordinates",
    )?;
    let scale = relative
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    Ok((scale > 0.0).then(|| {
        relative
            .into_iter()
            .map(|point| point.map(|value| value / scale))
            .collect()
    }))
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
