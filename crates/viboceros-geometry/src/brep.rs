use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, LineSegment, NurbsCurve, NurbsCurve2,
    NurbsSurface, Point2, Point3, Real, Tolerance, TriangleMesh, require_finite,
};

const LOOP_SAMPLES_PER_SPAN: usize = 4;

#[derive(Clone, Copy, Debug)]
struct BoundarySnapPoint {
    point: Point3,
    tolerance: Real,
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

    /// Tessellates faces whose sole outer loop is exactly the full rectangular
    /// surface domain. Faces with holes or general trims are rejected until a
    /// constrained parameter-space triangulator is available; they are never
    /// silently filled as untrimmed surfaces.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            if !face_covers_full_surface_domain(face, tolerance)? {
                return Err(GeometryError::UnsupportedBrepTrimTessellation { face: face_index });
            }
            let mesh = face.surface.tessellate(samples_per_span, tolerance)?;
            let mut face_vertices = mesh.vertices().to_vec();
            self.snap_face_boundary_vertices(
                face,
                &mut face_vertices,
                samples_per_span,
                tolerance,
            )?;
            let offset =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            let combined_vertex_count = vertices
                .len()
                .checked_add(face_vertices.len())
                .ok_or(GeometryError::TooManyMeshVertices)?;
            if combined_vertex_count > u32::MAX as usize {
                return Err(GeometryError::TooManyMeshVertices);
            }
            vertices.extend(face_vertices);
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
            if let Some(edge_index) = trim.edge {
                let edge = &self.edges[edge_index];
                let spans = edge.curve.spans().collect::<Vec<_>>();
                let domain = edge.curve.domain();
                let last_span =
                    spans
                        .len()
                        .checked_sub(1)
                        .ok_or(GeometryError::InvalidBrepTopology {
                            context: "a B-rep edge curve has no nonempty span",
                        })?;
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
                            let parameter = brep_span_parameter(
                                start,
                                end,
                                sample,
                                samples_per_span,
                                *domain.end(),
                            );
                            (edge.curve.evaluate(parameter)?, edge.tolerance)
                        };
                        candidates[side].push(BoundarySnapPoint {
                            point,
                            tolerance: tolerance.absolute().max(component_tolerance),
                        });
                    }
                }
            } else {
                let vertex = self.vertices[trim.vertices[0]];
                candidates[side].push(BoundarySnapPoint {
                    point: vertex.point,
                    tolerance: tolerance.absolute().max(vertex.tolerance),
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
    use crate::Vector3;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
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
    fn a_trimmed_face_with_a_hole_is_valid_but_never_tessellated_as_untrimmed() {
        let model_points = [
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
            point(3.0, 3.0, 0.0),
            point(3.0, 7.0, 0.0),
            point(7.0, 7.0, 0.0),
            point(7.0, 3.0, 0.0),
        ];
        let vertices = model_points
            .into_iter()
            .map(|point| BrepVertex::try_new(point, 0.0).unwrap())
            .collect::<Vec<_>>();
        let paths = [[0, 1, 2, 3], [4, 5, 6, 7]];
        let mut edges = Vec::new();
        let mut loops = Vec::new();
        for (loop_index, path) in paths.into_iter().enumerate() {
            let mut trims = Vec::new();
            for index in 0..4 {
                let from = path[index];
                let to = path[(index + 1) % 4];
                let edge_index = edges.len();
                edges.push(
                    BrepEdge::try_new(
                        [from, to],
                        LineSegment::try_new(
                            vertices[from].point(),
                            vertices[to].point(),
                            Tolerance::DEFAULT,
                        )
                        .unwrap()
                        .to_nurbs()
                        .unwrap(),
                        0.0,
                    )
                    .unwrap(),
                );
                let from_point = vertices[from].point();
                let to_point = vertices[to].point();
                trims.push(
                    BrepTrim::try_new(
                        [from, to],
                        Some(edge_index),
                        false,
                        NurbsCurve2::try_line(
                            Point2::try_new(from_point.x() / 10.0, from_point.y() / 10.0).unwrap(),
                            Point2::try_new(to_point.x() / 10.0, to_point.y() / 10.0).unwrap(),
                        )
                        .unwrap(),
                        BrepTrimType::Boundary,
                        if loop_index == 0 {
                            [
                                SurfaceIso::South,
                                SurfaceIso::East,
                                SurfaceIso::North,
                                SurfaceIso::West,
                            ][index]
                        } else {
                            SurfaceIso::NotIso
                        },
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
        }
        let surface = NurbsSurface::try_bilinear(model_points[..4].try_into().unwrap()).unwrap();
        let brep = Brep::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(surface, false, loops).unwrap()],
            Tolerance::DEFAULT,
        )
        .unwrap();

        assert!(brep.is_manifold());
        assert!(!brep.is_closed());
        assert!(matches!(
            brep.tessellate(1, Tolerance::DEFAULT),
            Err(GeometryError::UnsupportedBrepTrimTessellation { face: 0 })
        ));
    }
}
