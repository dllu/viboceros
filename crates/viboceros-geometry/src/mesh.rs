use std::collections::{BTreeMap, BTreeSet, VecDeque};

use spade::{
    ConstrainedDelaunayTriangulation, HasPosition, Point2 as TriangulationPoint2, Triangulation,
};

use crate::vector::product_three;
use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, LineSegment, NurbsSurface, Point3,
    Polyline3, Real, Tolerance, UnitVector3, require_finite,
};

/// Resource ceiling for one generated mesh-plane grid.
pub const MAX_MESH_PLANE_FACES: usize = 1_000_000;

/// Resource ceiling for one generated mesh-box shell.
pub const MAX_MESH_BOX_FACES: usize = 1_000_000;

/// Resource ceiling for one generated mesh-cylinder shell.
pub const MAX_MESH_CYLINDER_FACES: usize = 1_000_000;

/// Resource ceiling for one generated mesh-cone shell.
pub const MAX_MESH_CONE_FACES: usize = 1_000_000;

/// Resource ceiling for one generated UV mesh-sphere shell.
pub const MAX_MESH_SPHERE_FACES: usize = 1_000_000;

/// Resource ceiling for one generated mesh-ellipsoid shell.
pub const MAX_MESH_ELLIPSOID_FACES: usize = 1_000_000;

/// RhinoCommon's maximum subdivision count for a quad mesh sphere.
pub const MAX_MESH_QUAD_SPHERE_SUBDIVISIONS: usize = 8;

/// RhinoCommon's maximum subdivision count for a triangular icosphere.
pub const MAX_MESH_ICO_SPHERE_SUBDIVISIONS: usize = 7;

/// Resource ceiling for one generated mesh-torus shell.
pub const MAX_MESH_TORUS_FACES: usize = 1_000_000;

/// Polygon style used for a generated radial mesh-primitive cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshCapFaceStyle {
    Triangles,
    Quadrilaterals,
}

/// Topology controls for an exact polygonal cylinder primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshCylinderOptions {
    pub vertical_count: usize,
    pub around_count: usize,
    pub cap_bottom: bool,
    pub cap_top: bool,
    pub circumscribe: bool,
    pub cap_style: MeshCapFaceStyle,
}

/// Topology controls for an exact polygonal cone primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshConeOptions {
    pub vertical_count: usize,
    pub around_count: usize,
    pub solid: bool,
    pub cap_style: MeshCapFaceStyle,
}

/// Topology controls for an exact UV mesh-sphere primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshUvSphereOptions {
    pub vertical_count: usize,
    pub around_count: usize,
}

/// Topology controls for an exact polygonal ellipsoid primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshEllipsoidOptions {
    pub vertical_count: usize,
    pub around_count: usize,
    pub cap_style: MeshCapFaceStyle,
}

/// Refinement control for an evenly distributed mesh-sphere primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshSubdivisionSphereOptions {
    pub subdivisions: usize,
}

/// Topology controls for an exact polygonal torus primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshTorusOptions {
    pub vertical_count: usize,
    pub around_count: usize,
}

/// Exact location-welded edge topology for a polygon mesh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshTopology {
    topological_vertex_count: usize,
    edge_count: usize,
    boundary_edge_count: usize,
    non_manifold_edge_count: usize,
    orientation_conflict_edge_count: usize,
    closed: bool,
}

/// Selects topology edges for mesh-curve extraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshEdgeFilter {
    /// Edges used by exactly one polygon face.
    Naked,
    /// Naked edges and coincident seams whose faces use distinct raw vertices.
    Unwelded,
    /// Edges whose greatest incident-face normal angle lies strictly inside
    /// this radian interval.
    FaceAngle {
        greater_than_radians: Real,
        less_than_radians: Real,
    },
}

impl MeshTopology {
    pub const fn topological_vertex_count(self) -> usize {
        self.topological_vertex_count
    }

    pub const fn edge_count(self) -> usize {
        self.edge_count
    }

    pub const fn boundary_edge_count(self) -> usize {
        self.boundary_edge_count
    }

    pub const fn non_manifold_edge_count(self) -> usize {
        self.non_manifold_edge_count
    }

    pub const fn orientation_conflict_edge_count(self) -> usize {
        self.orientation_conflict_edge_count
    }

    pub const fn is_closed(self) -> bool {
        self.closed
    }

    pub const fn is_manifold(self) -> bool {
        self.non_manifold_edge_count == 0
    }

    pub const fn is_oriented(self) -> bool {
        self.non_manifold_edge_count == 0 && self.orientation_conflict_edge_count == 0
    }

    pub const fn is_solid(self) -> bool {
        self.is_closed() && self.is_manifold() && self.is_oriented()
    }
}

#[derive(Clone, Debug, Default)]
struct EdgeIncidence {
    count: usize,
    forward_count: usize,
    first_use: Option<EdgeUse>,
    second_use: Option<EdgeUse>,
    additional_uses: Vec<EdgeUse>,
}

impl EdgeIncidence {
    fn add_use(&mut self, edge_use: EdgeUse) {
        match self.count {
            0 => self.first_use = Some(edge_use),
            1 => self.second_use = Some(edge_use),
            _ => self.additional_uses.push(edge_use),
        }
        self.count += 1;
        self.forward_count += usize::from(edge_use.forward);
    }

    fn uses(&self) -> impl Iterator<Item = EdgeUse> + '_ {
        self.first_use
            .into_iter()
            .chain(self.second_use)
            .chain(self.additional_uses.iter().copied())
    }
}

#[derive(Clone, Copy, Debug)]
struct EdgeUse {
    face: usize,
    side: usize,
    forward: bool,
    /// Raw mesh vertex indices ordered like the canonical topology edge.
    raw_vertices: [u32; 2],
}

#[derive(Debug)]
struct MeshTopologyData {
    topological_vertex_count: usize,
    topological_points: Vec<Point3>,
    topological_vertices: Vec<usize>,
    edges: BTreeMap<(usize, usize), EdgeIncidence>,
}

#[derive(Clone, Copy, Debug)]
struct MeshHoleTriangulationVertex {
    position: TriangulationPoint2<Real>,
    source_index: usize,
}

impl HasPosition for MeshHoleTriangulationVertex {
    type Scalar = Real;

    fn position(&self) -> TriangulationPoint2<Self::Scalar> {
        self.position
    }
}

/// The two parts produced by extracting faces from a mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFaceExtraction {
    remainder: Option<TriangleMesh>,
    extracted: TriangleMesh,
}

/// The joined mesh and independent triangular patch produced by filling one
/// naked mesh boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshHoleFill {
    filled: TriangleMesh,
    patch: TriangleMesh,
}

impl MeshHoleFill {
    pub const fn filled(&self) -> &TriangleMesh {
        &self.filled
    }

    pub const fn patch(&self) -> &TriangleMesh {
        &self.patch
    }

    pub fn into_parts(self) -> (TriangleMesh, TriangleMesh) {
        (self.filled, self.patch)
    }
}

impl MeshFaceExtraction {
    pub fn remainder(&self) -> Option<&TriangleMesh> {
        self.remainder.as_ref()
    }

    pub const fn extracted(&self) -> &TriangleMesh {
        &self.extracted
    }

    pub fn into_parts(self) -> (Option<TriangleMesh>, TriangleMesh) {
        (self.remainder, self.extracted)
    }
}

/// One validated triangle or quadrilateral in a polygon mesh.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MeshFace {
    Triangle([u32; 3]),
    Quad([u32; 4]),
}

impl MeshFace {
    #[inline]
    pub const fn vertex_count(self) -> usize {
        match self {
            Self::Triangle(_) => 3,
            Self::Quad(_) => 4,
        }
    }

    #[inline]
    pub const fn is_triangle(self) -> bool {
        matches!(self, Self::Triangle(_))
    }

    #[inline]
    pub const fn is_quad(self) -> bool {
        matches!(self, Self::Quad(_))
    }

    #[inline]
    pub fn indices(&self) -> &[u32] {
        match self {
            Self::Triangle(indices) => indices,
            Self::Quad(indices) => indices,
        }
    }

    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Triangle([a, b, c]) => Self::Triangle([a, c, b]),
            Self::Quad([a, b, c, d]) => Self::Quad([a, d, c, b]),
        }
    }

    pub(crate) fn remapped(self, mut map: impl FnMut(u32) -> u32) -> Self {
        match self {
            Self::Triangle(indices) => Self::Triangle(indices.map(&mut map)),
            Self::Quad(indices) => Self::Quad(indices.map(&mut map)),
        }
    }
}

/// An indexed, oriented polygon mesh with validated finite vertices and
/// non-degenerate triangle and quadrilateral faces.
///
/// `TriangleMesh` retains its original public name for API compatibility, but
/// quadrilateral faces remain first-class so topology and 3DM interchange do
/// not invent diagonal edges. [`Self::triangles`] provides a deterministic
/// `0-2` triangulation for algorithms and formats that require triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleMesh {
    vertices: Vec<Point3>,
    faces: Vec<MeshFace>,
    triangles: Vec<[u32; 3]>,
}

impl TriangleMesh {
    pub fn try_new(
        vertices: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_faces(
            vertices,
            triangles.into_iter().map(MeshFace::Triangle).collect(),
            tolerance,
        )
    }

    pub fn try_new_faces(
        vertices: Vec<Point3>,
        faces: Vec<MeshFace>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if faces.is_empty() {
            return Err(GeometryError::EmptyMesh);
        }
        if vertices
            .len()
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let mut triangles = Vec::new();
        triangles
            .try_reserve(faces.len().saturating_mul(2))
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for (face_index, face) in faces.iter().copied().enumerate() {
            match face {
                MeshFace::Triangle(triangle) => {
                    validate_triangle(&vertices, triangle, face_index, false, tolerance)?;
                    triangles.push(triangle);
                }
                MeshFace::Quad([a, b, c, d]) => {
                    validate_triangle(&vertices, [a, b, c], face_index, true, tolerance)?;
                    validate_triangle(&vertices, [a, c, d], face_index, true, tolerance)?;
                    if vertices[b as usize] == vertices[d as usize] {
                        return Err(GeometryError::DegenerateQuad { face: face_index });
                    }
                    triangles.extend([[a, b, c], [a, c, d]]);
                }
            }
        }

        Ok(Self {
            vertices,
            faces,
            triangles,
        })
    }

    /// Constructs an ordered quadrilateral grid over increasing plane intervals.
    ///
    /// Vertices run in x-fastest row-major order and faces run in the same
    /// y-then-x order as RhinoCommon's `Mesh.CreateFromPlane`.
    pub fn try_plane_grid(
        frame: Frame3,
        x_interval: [Real; 2],
        y_interval: [Real; 2],
        x_count: usize,
        y_count: usize,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            x_interval.into_iter().chain(y_interval),
            "mesh-plane interval",
        )?;
        if x_count == 0 || y_count == 0 {
            return Err(GeometryError::InvalidMeshPlaneFaceCount { x_count, y_count });
        }
        if x_interval[0] >= x_interval[1] || y_interval[0] >= y_interval[1] {
            return Err(GeometryError::InvalidMeshPlaneInterval);
        }

        let x_vertex_count = x_count
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let y_vertex_count = y_count
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let vertex_count = x_vertex_count
            .checked_mul(y_vertex_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertex_count
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let face_count = x_count
            .checked_mul(y_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_PLANE_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        let origin = frame.origin();
        let x_axis = frame.x_axis().as_vector();
        let y_axis = frame.y_axis().as_vector();
        let x_step = (x_interval[1] - x_interval[0]) / x_count as Real;
        let y_step = (y_interval[1] - y_interval[0]) / y_count as Real;
        require_finite([x_step, y_step], "mesh-plane interval span")?;
        for y_index in 0..=y_count {
            let y = mesh_grid_sample(y_interval, y_step, y_count, y_index);
            let y_offset = y_axis.scaled(y)?;
            for x_index in 0..=x_count {
                let x = mesh_grid_sample(x_interval, x_step, x_count, x_index);
                vertices.push(origin.translated(x_axis.scaled(x)?)?.translated(y_offset)?);
            }
        }

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for y_index in 0..y_count {
            for x_index in 0..x_count {
                let lower_left = y_index * x_vertex_count + x_index;
                let upper_left = lower_left + x_vertex_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(lower_left).map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(lower_left + 1)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_left + 1)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_left).map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's six independently stored quadrilateral box grids.
    ///
    /// The bottom, top, front, right, back, and left grids are appended in
    /// that order. Their raw vertices remain separate while exact-location
    /// topology forms one closed, outward-oriented shell.
    pub fn try_box_grid(
        frame: Frame3,
        intervals: [[Real; 2]; 3],
        x_count: usize,
        y_count: usize,
        z_count: usize,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let [x_interval, y_interval, z_interval] = intervals;
        require_finite(
            x_interval.into_iter().chain(y_interval).chain(z_interval),
            "mesh-box interval",
        )?;
        if x_count == 0 || y_count == 0 || z_count == 0 {
            return Err(GeometryError::InvalidMeshBoxFaceCount {
                x_count,
                y_count,
                z_count,
            });
        }
        if x_interval[0] >= x_interval[1]
            || y_interval[0] >= y_interval[1]
            || z_interval[0] >= z_interval[1]
        {
            return Err(GeometryError::InvalidMeshBoxInterval);
        }

        let xy_faces = x_count
            .checked_mul(y_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let xz_faces = x_count
            .checked_mul(z_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let yz_faces = y_count
            .checked_mul(z_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let face_count = xy_faces
            .checked_add(xz_faces)
            .and_then(|count| count.checked_add(yz_faces))
            .and_then(|count| count.checked_mul(2))
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_BOX_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }

        let x_vertices = x_count
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let y_vertices = y_count
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let z_vertices = z_count
            .checked_add(1)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let xy_vertices = x_vertices
            .checked_mul(y_vertices)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let xz_vertices = x_vertices
            .checked_mul(z_vertices)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let yz_vertices = y_vertices
            .checked_mul(z_vertices)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let vertex_count = xy_vertices
            .checked_add(xz_vertices)
            .and_then(|count| count.checked_add(yz_vertices))
            .and_then(|count| count.checked_mul(2))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertex_count
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let x_step = (x_interval[1] - x_interval[0]) / x_count as Real;
        let y_step = (y_interval[1] - y_interval[0]) / y_count as Real;
        let z_step = (z_interval[1] - z_interval[0]) / z_count as Real;
        require_finite([x_step, y_step, z_step], "mesh-box interval span")?;

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;

        append_mesh_grid_side(&mut vertices, &mut faces, x_count, y_count, |x, y| {
            mesh_frame_point(
                frame,
                mesh_grid_sample(x_interval, x_step, x_count, x),
                mesh_grid_sample(y_interval, y_step, y_count, y_count - y),
                z_interval[0],
            )
        })?;
        append_mesh_grid_side(&mut vertices, &mut faces, x_count, y_count, |x, y| {
            mesh_frame_point(
                frame,
                mesh_grid_sample(x_interval, x_step, x_count, x),
                mesh_grid_sample(y_interval, y_step, y_count, y),
                z_interval[1],
            )
        })?;
        append_mesh_grid_side(&mut vertices, &mut faces, x_count, z_count, |x, z| {
            mesh_frame_point(
                frame,
                mesh_grid_sample(x_interval, x_step, x_count, x),
                y_interval[0],
                mesh_grid_sample(z_interval, z_step, z_count, z),
            )
        })?;
        append_mesh_grid_side(&mut vertices, &mut faces, y_count, z_count, |y, z| {
            mesh_frame_point(
                frame,
                x_interval[1],
                mesh_grid_sample(y_interval, y_step, y_count, y),
                mesh_grid_sample(z_interval, z_step, z_count, z),
            )
        })?;
        append_mesh_grid_side(&mut vertices, &mut faces, x_count, z_count, |x, z| {
            mesh_frame_point(
                frame,
                mesh_grid_sample(x_interval, x_step, x_count, x_count - x),
                y_interval[1],
                mesh_grid_sample(z_interval, z_step, z_count, z),
            )
        })?;
        append_mesh_grid_side(&mut vertices, &mut faces, y_count, z_count, |y, z| {
            mesh_frame_point(
                frame,
                x_interval[0],
                mesh_grid_sample(y_interval, y_step, y_count, y_count - y),
                mesh_grid_sample(z_interval, z_step, z_count, z),
            )
        })?;
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's ordered polygonal cylinder wall and optional caps.
    ///
    /// Wall vertices are stored as height-major rings without a duplicated
    /// angular seam. Each cap has its own raw vertices, matching
    /// `Mesh.CreateFromCylinder`; exact-location topology still joins the
    /// requested caps to the wall.
    pub fn try_cylinder_grid(
        frame: Frame3,
        radius: Real,
        heights: [Real; 2],
        options: MeshCylinderOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(
            std::iter::once(radius).chain(heights),
            "mesh-cylinder dimensions",
        )?;
        if options.vertical_count == 0 || options.around_count < 3 {
            return Err(GeometryError::InvalidMeshCylinderFaceCount {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            });
        }
        if radius <= 0.0 || heights[0] >= heights[1] {
            return Err(GeometryError::InvalidMeshCylinderDimensions);
        }

        let wall_face_count = options
            .vertical_count
            .checked_mul(options.around_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let cap_count = usize::from(options.cap_bottom) + usize::from(options.cap_top);
        let cap_face_count = mesh_radial_cap_face_count(options.around_count, options.cap_style);
        let face_count = cap_face_count
            .checked_mul(cap_count)
            .and_then(|caps| wall_face_count.checked_add(caps))
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_CYLINDER_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }

        let wall_vertex_count = options
            .vertical_count
            .checked_add(1)
            .and_then(|rings| rings.checked_mul(options.around_count))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let one_cap_vertex_count =
            if options.cap_style == MeshCapFaceStyle::Quadrilaterals && options.around_count == 4 {
                options.around_count
            } else {
                options
                    .around_count
                    .checked_add(1)
                    .ok_or(GeometryError::TooManyMeshVertices)?
            };
        let vertex_count = one_cap_vertex_count
            .checked_mul(cap_count)
            .and_then(|caps| wall_vertex_count.checked_add(caps))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertex_count
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let angle_step = std::f64::consts::TAU / options.around_count as Real;
        let half_step = 0.5 * angle_step;
        let polygon_radius = if options.circumscribe {
            radius / half_step.cos()
        } else {
            radius
        };
        let start_angle = if options.circumscribe { half_step } else { 0.0 };
        let height_step = (heights[1] - heights[0]) / options.vertical_count as Real;
        require_finite(
            [angle_step, polygon_radius, height_step],
            "mesh-cylinder sampling",
        )?;

        let mut radial_coordinates = Vec::new();
        radial_coordinates
            .try_reserve_exact(options.around_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for around_index in 0..options.around_count {
            let angle = (around_index as Real).mul_add(angle_step, start_angle);
            let (sine, cosine) = angle.sin_cos();
            radial_coordinates.push([polygon_radius * cosine, polygon_radius * sine]);
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for vertical_index in 0..=options.vertical_count {
            let height =
                mesh_grid_sample(heights, height_step, options.vertical_count, vertical_index);
            for [x, y] in &radial_coordinates {
                vertices.push(mesh_frame_point(frame, *x, *y, height)?);
            }
        }

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for vertical_index in 0..options.vertical_count {
            let lower_offset = vertical_index * options.around_count;
            let upper_offset = lower_offset + options.around_count;
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(lower_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(lower_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        if options.cap_bottom {
            append_mesh_radial_cap(
                &mut vertices,
                &mut faces,
                frame,
                &radial_coordinates,
                heights[0],
                options.cap_style,
            )?;
        }
        if options.cap_top {
            append_mesh_radial_cap(
                &mut vertices,
                &mut faces,
                frame,
                &radial_coordinates,
                heights[1],
                options.cap_style,
            )?;
        }
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's ordered polygonal cone wall and optional base cap.
    ///
    /// The frame origin is the apex and `height_to_base` is the signed base
    /// offset on frame Z, matching `Mesh.CreateFromCone`. The apex is stored
    /// once, followed by progressively larger height-major rings. A requested
    /// cap has independent raw vertices that exact-location topology joins to
    /// the final wall ring.
    pub fn try_cone_grid(
        apex_frame: Frame3,
        radius: Real,
        height_to_base: Real,
        options: MeshConeOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius, height_to_base], "mesh-cone dimensions")?;
        if options.vertical_count == 0 || options.around_count < 3 {
            return Err(GeometryError::InvalidMeshConeFaceCount {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            });
        }
        if radius <= 0.0 || height_to_base == 0.0 {
            return Err(GeometryError::InvalidMeshConeDimensions);
        }

        let wall_face_count = options
            .vertical_count
            .checked_mul(options.around_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let cap_face_count = if options.solid {
            mesh_radial_cap_face_count(options.around_count, options.cap_style)
        } else {
            0
        };
        let face_count = wall_face_count
            .checked_add(cap_face_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_CONE_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }

        let wall_vertex_count = options
            .vertical_count
            .checked_mul(options.around_count)
            .and_then(|rings| rings.checked_add(1))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let cap_vertex_count = if !options.solid {
            0
        } else if options.cap_style == MeshCapFaceStyle::Quadrilaterals && options.around_count == 4
        {
            options.around_count
        } else {
            options
                .around_count
                .checked_add(1)
                .ok_or(GeometryError::TooManyMeshVertices)?
        };
        let vertex_count = wall_vertex_count
            .checked_add(cap_vertex_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if vertex_count
            .checked_sub(1)
            .is_some_and(|last_index| u32::try_from(last_index).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let angle_step = std::f64::consts::TAU / options.around_count as Real;
        let radius_step = radius / options.vertical_count as Real;
        let height_step = height_to_base / options.vertical_count as Real;
        require_finite([angle_step, radius_step, height_step], "mesh-cone sampling")?;

        let mut unit_radial_coordinates = Vec::new();
        unit_radial_coordinates
            .try_reserve_exact(options.around_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        let mut base_radial_coordinates = Vec::new();
        base_radial_coordinates
            .try_reserve_exact(options.around_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for around_index in 0..options.around_count {
            let angle = (around_index as Real).mul_add(angle_step, 0.0);
            let (sine, cosine) = angle.sin_cos();
            unit_radial_coordinates.push([cosine, sine]);
            base_radial_coordinates.push([radius * cosine, radius * sine]);
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.push(apex_frame.origin());
        for vertical_index in 1..=options.vertical_count {
            let ring_radius = mesh_grid_sample(
                [0.0, radius],
                radius_step,
                options.vertical_count,
                vertical_index,
            );
            let height = mesh_grid_sample(
                [0.0, height_to_base],
                height_step,
                options.vertical_count,
                vertical_index,
            );
            for [cosine, sine] in &unit_radial_coordinates {
                vertices.push(mesh_frame_point(
                    apex_frame,
                    ring_radius * cosine,
                    ring_radius * sine,
                    height,
                )?);
            }
        }

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for around_index in 0..options.around_count {
            let next = (around_index + 1) % options.around_count;
            faces.push(MeshFace::Triangle([
                0,
                u32::try_from(1 + next).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(1 + around_index).map_err(|_| GeometryError::TooManyMeshVertices)?,
            ]));
        }
        for vertical_index in 0..options.vertical_count.saturating_sub(1) {
            let inner_offset = 1 + vertical_index * options.around_count;
            let outer_offset = inner_offset + options.around_count;
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(inner_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(inner_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(outer_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(outer_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        if options.solid {
            append_mesh_radial_cap(
                &mut vertices,
                &mut faces,
                apex_frame,
                &base_radial_coordinates,
                height_to_base,
                options.cap_style,
            )?;
        }
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's ordered UV mesh sphere with shared pole vertices.
    ///
    /// Vertices run from the south pole through latitude-major rings to the
    /// north pole, with no duplicated longitude seam. The first and last
    /// latitude bands are triangle fans and all interior bands are quads,
    /// matching `Mesh.CreateFromSphere`.
    pub fn try_uv_sphere_grid(
        frame: Frame3,
        radius: Real,
        options: MeshUvSphereOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([radius], "mesh-sphere radius")?;
        if options.vertical_count < 2 || options.around_count < 3 {
            return Err(GeometryError::InvalidMeshSphereFaceCount {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            });
        }
        if radius <= 0.0 {
            return Err(GeometryError::InvalidMeshSphereRadius);
        }

        let face_count = options
            .vertical_count
            .checked_mul(options.around_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_SPHERE_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }
        let vertex_count = options
            .vertical_count
            .checked_sub(1)
            .and_then(|rings| rings.checked_mul(options.around_count))
            .and_then(|rings| rings.checked_add(2))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if u32::try_from(vertex_count - 1).is_err() {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let longitude_step = std::f64::consts::TAU / options.around_count as Real;
        let latitude_step = std::f64::consts::PI / options.vertical_count as Real;
        require_finite([longitude_step, latitude_step], "mesh-sphere sampling")?;
        let mut longitude_coordinates = Vec::new();
        longitude_coordinates
            .try_reserve_exact(options.around_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for around_index in 0..options.around_count {
            let angle = (around_index as Real).mul_add(longitude_step, 0.0);
            let (sine, cosine) = angle.sin_cos();
            longitude_coordinates.push([cosine, sine]);
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.push(mesh_frame_point(frame, 0.0, 0.0, -radius)?);
        for vertical_index in 1..options.vertical_count {
            let latitude =
                (vertical_index as Real).mul_add(latitude_step, -std::f64::consts::FRAC_PI_2);
            let (latitude_sine, latitude_cosine) = latitude.sin_cos();
            let ring_radius = radius * latitude_cosine;
            let height = radius * latitude_sine;
            for [longitude_cosine, longitude_sine] in &longitude_coordinates {
                vertices.push(mesh_frame_point(
                    frame,
                    ring_radius * longitude_cosine,
                    ring_radius * longitude_sine,
                    height,
                )?);
            }
        }
        vertices.push(mesh_frame_point(frame, 0.0, 0.0, radius)?);

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for around_index in 0..options.around_count {
            let next = (around_index + 1) % options.around_count;
            faces.push(MeshFace::Triangle([
                0,
                u32::try_from(1 + next).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(1 + around_index).map_err(|_| GeometryError::TooManyMeshVertices)?,
            ]));
        }
        for ring_index in 0..options.vertical_count - 2 {
            let lower_offset = 1 + ring_index * options.around_count;
            let upper_offset = lower_offset + options.around_count;
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(lower_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(lower_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        let north =
            u32::try_from(vertex_count - 1).map_err(|_| GeometryError::TooManyMeshVertices)?;
        let last_ring_offset = 1 + (options.vertical_count - 2) * options.around_count;
        for around_index in 0..options.around_count {
            let next = (around_index + 1) % options.around_count;
            faces.push(MeshFace::Triangle([
                u32::try_from(last_ring_offset + around_index)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(last_ring_offset + next)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?,
                north,
            ]));
        }
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's ordered mesh ellipsoid from its three semi-axes.
    ///
    /// The supplied frame's X axis is the first construction axis and joins
    /// the two poles. Rings begin on positive frame Y and advance toward
    /// positive frame Z. Samples are uniform in the exact rational NURBS
    /// ellipsoid's parameter domains, rather than uniform in geometric angle.
    /// Even around counts may pair the pole fans into Rhino's degenerate quad
    /// caps; odd counts always use triangles.
    pub fn try_ellipsoid_grid(
        frame: Frame3,
        radii: [Real; 3],
        options: MeshEllipsoidOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite(radii, "mesh-ellipsoid radii")?;
        if options.vertical_count < 2 || options.around_count < 3 {
            return Err(GeometryError::InvalidMeshEllipsoidFaceCount {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            });
        }
        if radii.into_iter().any(|radius| radius <= 0.0) {
            return Err(GeometryError::InvalidMeshEllipsoidRadii);
        }

        let triangular_caps = options.cap_style == MeshCapFaceStyle::Triangles
            || !options.around_count.is_multiple_of(2);
        let cap_face_count = if triangular_caps {
            options
                .around_count
                .checked_mul(2)
                .ok_or(GeometryError::TooManyMeshFaces)?
        } else {
            options.around_count
        };
        let interior_face_count = options
            .vertical_count
            .checked_sub(2)
            .and_then(|bands| bands.checked_mul(options.around_count))
            .ok_or(GeometryError::TooManyMeshFaces)?;
        let face_count = interior_face_count
            .checked_add(cap_face_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_ELLIPSOID_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }
        let vertex_count = options
            .vertical_count
            .checked_sub(1)
            .and_then(|rings| rings.checked_mul(options.around_count))
            .and_then(|rings| rings.checked_add(2))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if u32::try_from(vertex_count - 1).is_err() {
            return Err(GeometryError::TooManyMeshVertices);
        }

        // A standard NURBS sphere is polar on frame Z and begins its U seam
        // on frame X. Cyclically permuting the ellipsoid frame therefore puts
        // the requested first axis at the poles without changing handedness.
        let surface_frame = Frame3::try_from_directions(
            frame.origin(),
            frame.y_axis().as_vector(),
            frame.z_axis().as_vector(),
            tolerance,
        )?;
        let surface = NurbsSurface::try_ellipsoid(surface_frame, [radii[1], radii[2], radii[0]])?;

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices
            .push(surface.evaluate(surface.parameter_at_u(0.0)?, surface.parameter_at_v(0.0)?)?);
        for vertical_index in 1..options.vertical_count {
            let v =
                surface.parameter_at_v(vertical_index as Real / options.vertical_count as Real)?;
            for around_index in 0..options.around_count {
                let u =
                    surface.parameter_at_u(around_index as Real / options.around_count as Real)?;
                vertices.push(surface.evaluate(u, v)?);
            }
        }
        vertices
            .push(surface.evaluate(surface.parameter_at_u(0.0)?, surface.parameter_at_v(1.0)?)?);

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        if triangular_caps {
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Triangle([
                    0,
                    u32::try_from(1 + next).map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(1 + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        } else {
            for face_index in 0..options.around_count / 2 {
                let first = 2 * face_index;
                faces.push(MeshFace::Quad([
                    0,
                    u32::try_from(1 + (first + 2) % options.around_count)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(1 + first + 1).map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(1 + first).map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        for ring_index in 0..options.vertical_count - 2 {
            let lower_offset = 1 + ring_index * options.around_count;
            let upper_offset = lower_offset + options.around_count;
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(lower_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(lower_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(upper_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        let north =
            u32::try_from(vertex_count - 1).map_err(|_| GeometryError::TooManyMeshVertices)?;
        let last_ring_offset = 1 + (options.vertical_count - 2) * options.around_count;
        if triangular_caps {
            for around_index in 0..options.around_count {
                let next = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Triangle([
                    u32::try_from(last_ring_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(last_ring_offset + next)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    north,
                ]));
            }
        } else {
            for face_index in 0..options.around_count / 2 {
                let first = 2 * face_index;
                faces.push(MeshFace::Quad([
                    u32::try_from(last_ring_offset + first)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(last_ring_offset + first + 1)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(last_ring_offset + (first + 2) % options.around_count)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    north,
                ]));
            }
        }
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's evenly distributed quad mesh sphere.
    ///
    /// Subdivision zero is a cube projected onto the sphere. Each refinement
    /// applies one Catmull-Clark step to the cube control mesh, storing face
    /// points, edge points, and updated old vertices in that order. The final
    /// control vertices are projected radially onto the sphere. This preserves
    /// `Mesh.CreateQuadSphere` geometry and indexing.
    pub fn try_quad_sphere(
        frame: Frame3,
        radius: Real,
        options: MeshSubdivisionSphereOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        validate_subdivision_sphere_options(
            radius,
            options.subdivisions,
            MAX_MESH_QUAD_SPHERE_SUBDIVISIONS,
            6,
        )?;

        let cube_coordinate = radius / 3.0_f64.sqrt();
        require_finite([cube_coordinate], "quad mesh-sphere sampling")?;
        let mut coordinates = vec![
            [-cube_coordinate, -cube_coordinate, -cube_coordinate],
            [cube_coordinate, -cube_coordinate, -cube_coordinate],
            [cube_coordinate, cube_coordinate, -cube_coordinate],
            [-cube_coordinate, cube_coordinate, -cube_coordinate],
            [-cube_coordinate, -cube_coordinate, cube_coordinate],
            [cube_coordinate, -cube_coordinate, cube_coordinate],
            [cube_coordinate, cube_coordinate, cube_coordinate],
            [-cube_coordinate, cube_coordinate, cube_coordinate],
        ];
        let mut quads = vec![
            [3, 2, 1, 0],
            [2, 6, 5, 1],
            [5, 6, 7, 4],
            [0, 4, 7, 3],
            [3, 7, 6, 2],
            [1, 5, 4, 0],
        ];
        // The directions record Rhino's persistent topology-edge orientation.
        // Edge points are stored in this order. At later levels, each edge's
        // two children precede the new face-center spokes.
        let mut ordered_edges = vec![
            (1, 0),
            (0, 3),
            (4, 0),
            (1, 2),
            (1, 5),
            (2, 3),
            (6, 2),
            (3, 7),
            (5, 4),
            (4, 7),
            (5, 6),
            (7, 6),
        ];

        for subdivision in 0..options.subdivisions {
            let next_vertex_count = quads
                .len()
                .checked_add(ordered_edges.len())
                .and_then(|count| count.checked_add(coordinates.len()))
                .ok_or(GeometryError::TooManyMeshVertices)?;
            let mut next_coordinates = Vec::new();
            next_coordinates
                .try_reserve_exact(next_vertex_count)
                .map_err(|_| GeometryError::TooManyMeshVertices)?;

            let mut face_points = Vec::new();
            face_points
                .try_reserve_exact(quads.len())
                .map_err(|_| GeometryError::TooManyMeshVertices)?;
            for quad in &quads {
                let mut center = [0.0; 3];
                for vertex in quad {
                    let coordinate = coordinates[*vertex as usize];
                    for axis in 0..3 {
                        center[axis] += coordinate[axis] * 0.25;
                    }
                }
                face_points.push(center);
            }
            next_coordinates.extend_from_slice(&face_points);

            let mut edge_order_by_index = BTreeMap::new();
            for (edge_index, &(first, second)) in ordered_edges.iter().enumerate() {
                let previous =
                    edge_order_by_index.insert(mesh_index_edge(first, second), edge_index);
                debug_assert!(previous.is_none());
            }
            let mut edge_faces = vec![[usize::MAX; 2]; ordered_edges.len()];
            let mut edge_face_counts = vec![0_u8; ordered_edges.len()];
            for (face_index, quad) in quads.iter().enumerate() {
                for index in 0..4 {
                    let edge = mesh_index_edge(quad[index], quad[(index + 1) % 4]);
                    let edge_index = edge_order_by_index[&edge];
                    let incident_count = usize::from(edge_face_counts[edge_index]);
                    debug_assert!(incident_count < 2);
                    edge_faces[edge_index][incident_count] = face_index;
                    edge_face_counts[edge_index] += 1;
                }
            }

            let face_count = quads.len();
            let mut edge_vertices = BTreeMap::new();
            for (edge_index, &(first, second)) in ordered_edges.iter().enumerate() {
                debug_assert_eq!(edge_face_counts[edge_index], 2);
                let [first_face, second_face] = edge_faces[edge_index];
                let first_coordinate = coordinates[first as usize];
                let second_coordinate = coordinates[second as usize];
                let edge_point = std::array::from_fn(|axis| {
                    0.25 * (first_coordinate[axis]
                        + second_coordinate[axis]
                        + face_points[first_face][axis]
                        + face_points[second_face][axis])
                });
                let vertex = u32::try_from(face_count + edge_index)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                next_coordinates.push(edge_point);
                edge_vertices.insert(mesh_index_edge(first, second), vertex);
            }

            let old_vertex_offset = next_coordinates.len();
            let mut face_point_sums = vec![[0.0; 3]; coordinates.len()];
            let mut incident_face_counts = vec![0_usize; coordinates.len()];
            for (face_index, quad) in quads.iter().enumerate() {
                for &vertex in quad {
                    for axis in 0..3 {
                        face_point_sums[vertex as usize][axis] += face_points[face_index][axis];
                    }
                    incident_face_counts[vertex as usize] += 1;
                }
            }
            let mut edge_midpoint_sums = vec![[0.0; 3]; coordinates.len()];
            let mut incident_edge_counts = vec![0_usize; coordinates.len()];
            for &(first, second) in &ordered_edges {
                let first_coordinate = coordinates[first as usize];
                let second_coordinate = coordinates[second as usize];
                for vertex in [first, second] {
                    for axis in 0..3 {
                        edge_midpoint_sums[vertex as usize][axis] +=
                            0.5 * (first_coordinate[axis] + second_coordinate[axis]);
                    }
                    incident_edge_counts[vertex as usize] += 1;
                }
            }
            for (vertex_index, coordinate) in coordinates.iter().enumerate() {
                let valence = incident_face_counts[vertex_index];
                debug_assert!(valence >= 3);
                debug_assert_eq!(incident_edge_counts[vertex_index], valence);
                let valence_real = valence as Real;
                let updated = std::array::from_fn(|axis| {
                    let average_face_point = face_point_sums[vertex_index][axis] / valence_real;
                    let average_edge_midpoint =
                        edge_midpoint_sums[vertex_index][axis] / valence_real;
                    (average_face_point
                        + 2.0 * average_edge_midpoint
                        + (valence_real - 3.0) * coordinate[axis])
                        / valence_real
                });
                next_coordinates.push(updated);
            }

            let next_face_count = quads
                .len()
                .checked_mul(4)
                .ok_or(GeometryError::TooManyMeshFaces)?;
            let mut next_quads = Vec::new();
            next_quads
                .try_reserve_exact(next_face_count)
                .map_err(|_| GeometryError::TooManyMeshFaces)?;
            for (face_index, quad) in quads.iter().enumerate() {
                let face_center =
                    u32::try_from(face_index).map_err(|_| GeometryError::TooManyMeshVertices)?;
                for index in 0..4 {
                    let previous = quad[(index + 3) % 4];
                    let current = quad[index];
                    let next = quad[(index + 1) % 4];
                    next_quads.push([
                        edge_vertices[&mesh_index_edge(previous, current)],
                        u32::try_from(old_vertex_offset + current as usize)
                            .map_err(|_| GeometryError::TooManyMeshVertices)?,
                        edge_vertices[&mesh_index_edge(current, next)],
                        face_center,
                    ]);
                }
            }
            if subdivision + 1 < options.subdivisions {
                let next_edge_count = ordered_edges
                    .len()
                    .checked_mul(2)
                    .and_then(|count| quads.len().checked_mul(4)?.checked_add(count))
                    .ok_or(GeometryError::TooManyMeshVertices)?;
                let mut next_ordered_edges = Vec::new();
                next_ordered_edges
                    .try_reserve_exact(next_edge_count)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                for &(first, second) in &ordered_edges {
                    let edge_vertex = edge_vertices[&mesh_index_edge(first, second)];
                    let shifted_first = u32::try_from(old_vertex_offset + first as usize)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    let shifted_second = u32::try_from(old_vertex_offset + second as usize)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    next_ordered_edges.push((shifted_first, edge_vertex));
                    next_ordered_edges.push((edge_vertex, shifted_second));
                }
                for (face_index, quad) in quads.iter().enumerate() {
                    let face_vertex = u32::try_from(face_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    for index in 0..4 {
                        let previous = quad[(index + 3) % 4];
                        let current = quad[index];
                        let edge_vertex = edge_vertices[&mesh_index_edge(previous, current)];
                        next_ordered_edges.push((face_vertex, edge_vertex));
                    }
                }
                debug_assert_eq!(next_ordered_edges.len(), next_edge_count);
                ordered_edges = next_ordered_edges;
            }
            coordinates = next_coordinates;
            quads = next_quads;
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(coordinates.len())
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for coordinate in coordinates {
            let [x, y, z] = project_sphere_coordinate(coordinate, radius)?;
            vertices.push(mesh_frame_point(frame, x, y, z)?);
        }
        let faces = quads.into_iter().map(MeshFace::Quad).collect();
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's evenly distributed triangular icosphere.
    ///
    /// Subdivision zero is a regular icosahedron projected onto the sphere.
    /// Every refinement appends projected edge midpoints in first-face-use
    /// order and replaces each triangle with three corner faces followed by
    /// its central face, matching `Mesh.CreateIcoSphere`.
    pub fn try_ico_sphere(
        frame: Frame3,
        radius: Real,
        options: MeshSubdivisionSphereOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        validate_subdivision_sphere_options(
            radius,
            options.subdivisions,
            MAX_MESH_ICO_SPHERE_SUBDIVISIONS,
            20,
        )?;

        let golden_ratio = 0.5 * (1.0 + 5.0_f64.sqrt());
        let short = radius / (1.0 + golden_ratio * golden_ratio).sqrt();
        let long = golden_ratio * short;
        require_finite([short, long], "icosphere sampling")?;
        let mut coordinates = vec![
            [-short, long, 0.0],
            [short, long, 0.0],
            [-short, -long, 0.0],
            [short, -long, 0.0],
            [0.0, -short, long],
            [0.0, short, long],
            [0.0, -short, -long],
            [0.0, short, -long],
            [long, 0.0, -short],
            [long, 0.0, short],
            [-long, 0.0, -short],
            [-long, 0.0, short],
        ];
        let mut triangles = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];

        for _ in 0..options.subdivisions {
            let next_face_count = triangles
                .len()
                .checked_mul(4)
                .ok_or(GeometryError::TooManyMeshFaces)?;
            let mut next_triangles = Vec::new();
            next_triangles
                .try_reserve_exact(next_face_count)
                .map_err(|_| GeometryError::TooManyMeshFaces)?;
            let mut edge_vertices = BTreeMap::new();
            for [a, b, c] in triangles {
                let ab =
                    append_icosphere_midpoint(&mut coordinates, &mut edge_vertices, a, b, radius)?;
                let bc =
                    append_icosphere_midpoint(&mut coordinates, &mut edge_vertices, b, c, radius)?;
                let ca =
                    append_icosphere_midpoint(&mut coordinates, &mut edge_vertices, c, a, radius)?;
                next_triangles.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            triangles = next_triangles;
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(coordinates.len())
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for [x, y, z] in coordinates {
            vertices.push(mesh_frame_point(frame, x, y, z)?);
        }
        let faces = triangles.into_iter().map(MeshFace::Triangle).collect();
        Self::try_new_faces(vertices, faces, tolerance)
    }

    /// Constructs Rhino's ordered polygonal ring torus.
    ///
    /// Vertices are stored in minor-circle-major rows with no duplicated seam
    /// in either periodic direction. Every cell is a seam-wrapped quad,
    /// matching `Mesh.CreateFromTorus`.
    pub fn try_torus_grid(
        frame: Frame3,
        major_radius: Real,
        minor_radius: Real,
        options: MeshTorusOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([major_radius, minor_radius], "mesh-torus radii")?;
        if options.vertical_count < 3 || options.around_count < 3 {
            return Err(GeometryError::InvalidMeshTorusFaceCount {
                vertical_count: options.vertical_count,
                around_count: options.around_count,
            });
        }
        if minor_radius <= 0.0 || major_radius <= minor_radius {
            return Err(GeometryError::InvalidMeshTorusRadii);
        }

        let face_count = options
            .vertical_count
            .checked_mul(options.around_count)
            .ok_or(GeometryError::TooManyMeshFaces)?;
        if face_count > MAX_MESH_TORUS_FACES {
            return Err(GeometryError::TooManyMeshFaces);
        }
        let vertex_count = face_count;
        if u32::try_from(vertex_count - 1).is_err() {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let major_angle_step = std::f64::consts::TAU / options.around_count as Real;
        let minor_angle_step = std::f64::consts::TAU / options.vertical_count as Real;
        require_finite([major_angle_step, minor_angle_step], "mesh-torus sampling")?;

        let mut major_coordinates = Vec::new();
        major_coordinates
            .try_reserve_exact(options.around_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for around_index in 0..options.around_count {
            let angle = (around_index as Real).mul_add(major_angle_step, 0.0);
            let (sine, cosine) = angle.sin_cos();
            major_coordinates.push([cosine, sine]);
        }

        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        for vertical_index in 0..options.vertical_count {
            let minor_angle = (vertical_index as Real).mul_add(minor_angle_step, 0.0);
            let (minor_sine, minor_cosine) = minor_angle.sin_cos();
            let radial = minor_radius.mul_add(minor_cosine, major_radius);
            let height = minor_radius * minor_sine;
            for [major_cosine, major_sine] in &major_coordinates {
                vertices.push(mesh_frame_point(
                    frame,
                    radial * major_cosine,
                    radial * major_sine,
                    height,
                )?);
            }
        }

        let mut faces = Vec::new();
        faces
            .try_reserve_exact(face_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for vertical_index in 0..options.vertical_count {
            let next_vertical = (vertical_index + 1) % options.vertical_count;
            let current_offset = vertical_index * options.around_count;
            let next_offset = next_vertical * options.around_count;
            for around_index in 0..options.around_count {
                let next_around = (around_index + 1) % options.around_count;
                faces.push(MeshFace::Quad([
                    u32::try_from(current_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(current_offset + next_around)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(next_offset + next_around)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                    u32::try_from(next_offset + around_index)
                        .map_err(|_| GeometryError::TooManyMeshVertices)?,
                ]));
            }
        }
        debug_assert_eq!(vertices.len(), vertex_count);
        debug_assert_eq!(faces.len(), face_count);
        Self::try_new_faces(vertices, faces, tolerance)
    }

    fn from_validated_parts(vertices: Vec<Point3>, faces: Vec<MeshFace>) -> Self {
        let mut triangles = Vec::with_capacity(faces.len().saturating_mul(2));
        for face in &faces {
            match *face {
                MeshFace::Triangle(triangle) => triangles.push(triangle),
                MeshFace::Quad([a, b, c, d]) => {
                    triangles.extend([[a, b, c], [a, c, d]]);
                }
            }
        }
        Self {
            vertices,
            faces,
            triangles,
        }
    }

    #[inline]
    pub fn vertices(&self) -> &[Point3] {
        &self.vertices
    }

    #[inline]
    pub fn faces(&self) -> &[MeshFace] {
        &self.faces
    }

    #[inline]
    pub const fn face_count(&self) -> usize {
        self.faces.len()
    }

    #[inline]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Reverses every face winding without changing mesh vertex or face order.
    pub fn reversed(&self) -> Self {
        Self::from_validated_parts(
            self.vertices.clone(),
            self.faces.iter().copied().map(MeshFace::reversed).collect(),
        )
    }

    /// Splits every quadrilateral along its shortest three-dimensional
    /// diagonal, choosing A-C on an exact tie.
    ///
    /// Each first triangle replaces its source quad in place. Second
    /// triangles are appended in source-quad order, matching OpenNURBS and
    /// Rhino's `ConvertQuadsToTriangles` face ordering. Vertices are retained
    /// verbatim, including unused vertices.
    pub fn triangulate_quads(&self, tolerance: Tolerance) -> Result<(Self, usize), GeometryError> {
        let quad_count = self.faces.iter().filter(|face| face.is_quad()).count();
        if quad_count == 0 {
            return Ok((self.clone(), 0));
        }
        let mut faces = self.faces.clone();
        faces
            .try_reserve(quad_count)
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for face_index in 0..self.faces.len() {
            let MeshFace::Quad([a, b, c, d]) = self.faces[face_index] else {
                continue;
            };
            let diagonal_ac = self.vertices[a as usize].distance_to(self.vertices[c as usize])?;
            let diagonal_bd = self.vertices[b as usize].distance_to(self.vertices[d as usize])?;
            if diagonal_ac <= diagonal_bd {
                faces[face_index] = MeshFace::Triangle([a, b, c]);
                faces.push(MeshFace::Triangle([a, c, d]));
            } else {
                faces[face_index] = MeshFace::Triangle([a, b, d]);
                faces.push(MeshFace::Triangle([b, c, d]));
            }
        }
        Ok((
            Self::try_new_faces(self.vertices.clone(), faces, tolerance)?,
            quad_count,
        ))
    }

    /// Replaces one welded interior triangle edge with the opposite diagonal.
    ///
    /// The topology edge index uses the deterministic order exposed by
    /// [`Self::wireframe_lines`]. A swap is available only when exactly two
    /// consistently oriented triangle faces share both raw edge vertices.
    /// Vertices and face slots are retained verbatim, matching Rhino's
    /// `MeshTopologyEdgeList::SwapEdge` ordering. `None` indicates that the
    /// selected edge is not swappable or that the replacement would violate
    /// this mesh type's non-degenerate-face invariant.
    pub fn swap_topology_edge(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some(incidence) = data.edges.values().nth(edge_index) else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        if incidence.count != 2 {
            return Ok(None);
        }
        let first = incidence
            .first_use
            .expect("an edge used twice records its first face");
        let second = incidence
            .second_use
            .expect("an edge used twice records its second face");
        if first.forward == second.forward || first.raw_vertices != second.raw_vertices {
            return Ok(None);
        }
        let (forward, backward) = if first.forward {
            (first, second)
        } else {
            (second, first)
        };
        let MeshFace::Triangle(forward_face) = self.faces[forward.face] else {
            return Ok(None);
        };
        let MeshFace::Triangle(backward_face) = self.faces[backward.face] else {
            return Ok(None);
        };
        let forward_opposite = forward_face[(forward.side + 2) % 3];
        let backward_opposite = backward_face[(backward.side + 2) % 3];
        let [edge_start, edge_end] = forward.raw_vertices;
        let backward_replacement = [edge_start, backward_opposite, forward_opposite];
        let forward_replacement = [edge_end, forward_opposite, backward_opposite];
        for (face_index, triangle) in [
            (backward.face, backward_replacement),
            (forward.face, forward_replacement),
        ] {
            match validate_triangle(&self.vertices, triangle, face_index, false, tolerance) {
                Ok(()) => {}
                Err(GeometryError::DegenerateTriangle { .. }) => return Ok(None),
                Err(error) => return Err(error),
            }
        }

        let mut faces = self.faces.clone();
        faces[backward.face] = MeshFace::Triangle(backward_replacement);
        faces[forward.face] = MeshFace::Triangle(forward_replacement);
        Ok(Some(Self::from_validated_parts(
            self.vertices.clone(),
            faces,
        )))
    }

    /// Replaces one exact-location topology edge with vertices at its center.
    ///
    /// Every raw vertex belonging to either topology endpoint moves to the
    /// midpoint. Raw endpoint pairs used by the selected edge are merged, so
    /// welded and partially welded uses stay joined while independent seam
    /// components remain distinct. Collapsed triangle faces disappear;
    /// collapsed quad sides become triangles. Surviving faces retain source
    /// order and referenced vertices compact in source order, matching
    /// RhinoCommon's `MeshTopologyEdgeList::CollapseEdge` behavior. `None`
    /// means that the collapse removes every face.
    pub fn collapse_topology_edge(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some((&(first_topology_vertex, second_topology_vertex), incidence)) =
            data.edges.iter().nth(edge_index)
        else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        let first = data.topological_points[first_topology_vertex];
        let second = data.topological_points[second_topology_vertex];
        let midpoint = Point3::try_new(
            first.x() * 0.5 + second.x() * 0.5,
            first.y() * 0.5 + second.y() * 0.5,
            first.z() * 0.5 + second.z() * 0.5,
        )?;

        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for edge_use in incidence.uses() {
            union_indices_keep_earlier(
                &mut parents,
                edge_use.raw_vertices[0] as usize,
                edge_use.raw_vertices[1] as usize,
            );
        }
        let mut moved_vertices = self.vertices.clone();
        for (raw_vertex, &topology_vertex) in data.topological_vertices.iter().enumerate() {
            if topology_vertex == first_topology_vertex || topology_vertex == second_topology_vertex
            {
                moved_vertices[raw_vertex] = midpoint;
            }
        }

        let mut faces = Vec::with_capacity(self.faces.len());
        for face in self.faces.iter().copied() {
            let remapped = face.remapped(|raw| {
                u32::try_from(index_root(&mut parents, raw as usize))
                    .expect("a mesh raw vertex index already fits in u32")
            });
            match remapped {
                MeshFace::Triangle([a, b, c]) => {
                    if a != b && b != c && c != a {
                        faces.push(MeshFace::Triangle([a, b, c]));
                    }
                }
                MeshFace::Quad(indices) => {
                    let collapsed_sides = (0..4)
                        .filter(|&side| indices[side] == indices[(side + 1) % 4])
                        .collect::<Vec<_>>();
                    if collapsed_sides.is_empty() {
                        let unique = indices.into_iter().collect::<BTreeSet<_>>();
                        if unique.len() == 4 {
                            faces.push(MeshFace::Quad(indices));
                        }
                    } else if let [side] = collapsed_sides.as_slice() {
                        let start = (side + 2) % 4;
                        let triangle = [
                            indices[start],
                            indices[(start + 1) % 4],
                            indices[(start + 2) % 4],
                        ];
                        if triangle[0] != triangle[1]
                            && triangle[1] != triangle[2]
                            && triangle[2] != triangle[0]
                        {
                            faces.push(MeshFace::Triangle(triangle));
                        }
                    }
                }
            }
        }
        if faces.is_empty() {
            return Ok(None);
        }

        let mut used = vec![false; self.vertices.len()];
        for face in &faces {
            for &raw in face.indices() {
                used[raw as usize] = true;
            }
        }
        let retained_vertex_count = used.iter().filter(|&&retain| retain).count();
        let mut raw_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_vertex_count);
        for (raw, (&point, retain)) in moved_vertices.iter().zip(used).enumerate() {
            if !retain {
                continue;
            }
            raw_remap[raw] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }
        let faces = faces
            .into_iter()
            .map(|face| face.remapped(|raw| raw_remap[raw as usize]))
            .collect();
        Ok(Some(Self::try_new_faces(vertices, faces, tolerance)?))
    }

    /// Divides one exact-location topology edge at a normalized parameter.
    ///
    /// The parameter follows the direction returned by [`Self::wireframe_lines`]
    /// and values outside `[0, 1]` are rejected with `None`, matching
    /// RhinoCommon's `MeshTopologyEdgeList::SplitEdge`. Unaffected faces come
    /// first in source order and replacement triangles append in incident-face
    /// order. A welded edge shares one appended split vertex. Splitting an
    /// unwelded edge fully separates every replacement triangle, preserving
    /// Rhino's raw-vertex and seam behavior. Exact endpoint splits retain
    /// source triangles and append their coincident replacement.
    pub fn split_topology_edge(
        &self,
        edge_index: usize,
        parameter: Real,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some((&(first_topology_vertex, second_topology_vertex), incidence)) =
            data.edges.iter().nth(edge_index)
        else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        if !parameter.is_finite() || !(0.0..=1.0).contains(&parameter) {
            return Ok(None);
        }

        let first = data.topological_points[first_topology_vertex];
        let second = data.topological_points[second_topology_vertex];
        let edge = LineSegment::try_new(first, second, tolerance)?;
        let split_point = if parameter < 0.5 {
            edge.point_at(parameter)?
        } else {
            edge.reversed().point_at(1.0 - parameter)?
        };
        let split_at_endpoint = split_point == first || split_point == second;
        let edge_uses = incidence.uses().collect::<Vec<_>>();
        let first_raw_edge = edge_uses
            .first()
            .expect("a topology edge records at least one face use")
            .raw_vertices;
        let welded = edge_uses
            .iter()
            .all(|edge_use| edge_use.raw_vertices == first_raw_edge);
        let mut affected_faces = vec![false; self.faces.len()];
        let mut generated = Vec::<([Option<u32>; 3], bool)>::new();
        for edge_use in &edge_uses {
            affected_faces[edge_use.face] = true;
            let [from, to] = edge_use.raw_vertices;
            match self.faces[edge_use.face] {
                MeshFace::Triangle(indices) => {
                    let opposite = indices[(edge_use.side + 2) % 3];
                    generated.extend([
                        ([Some(opposite), Some(from), None], edge_use.forward),
                        ([Some(opposite), None, Some(to)], edge_use.forward),
                    ]);
                }
                MeshFace::Quad(indices) => {
                    let after_edge = indices[(edge_use.side + 2) % 4];
                    let before_edge = indices[(edge_use.side + 3) % 4];
                    let (from_opposite, to_opposite) = if edge_use.forward {
                        (before_edge, after_edge)
                    } else {
                        (after_edge, before_edge)
                    };
                    generated.extend([
                        (
                            [Some(from_opposite), None, Some(to_opposite)],
                            edge_use.forward,
                        ),
                        ([Some(from_opposite), Some(from), None], edge_use.forward),
                        ([Some(to_opposite), None, Some(to)], edge_use.forward),
                    ]);
                }
            }
        }
        generated.retain(|(vertices, _)| {
            let [a, b, c] =
                vertices.map(|raw| raw.map_or(split_point, |raw| self.vertices[raw as usize]));
            a != b && b != c && c != a
        });

        let retained_faces = self
            .faces
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(face_index, face)| {
                (!affected_faces[face_index]
                    || (split_at_endpoint && matches!(face, MeshFace::Triangle(_))))
                .then_some(face)
            })
            .collect::<Vec<_>>();
        let mut used = vec![false; self.vertices.len()];
        for face in &retained_faces {
            for &raw in face.indices() {
                used[raw as usize] = true;
            }
        }
        if welded {
            for (vertices, _) in &generated {
                for raw in vertices.iter().flatten() {
                    used[*raw as usize] = true;
                }
            }
        }

        let retained_vertex_count = used.iter().filter(|&&retain| retain).count();
        let mut raw_remap = vec![0_u32; self.vertices.len()];
        let mut vertices =
            Vec::with_capacity(retained_vertex_count.saturating_add(usize::from(welded)));
        for (raw, (&point, retain)) in self.vertices.iter().zip(used).enumerate() {
            if !retain {
                continue;
            }
            raw_remap[raw] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }
        let mut faces = retained_faces
            .into_iter()
            .map(|face| face.remapped(|raw| raw_remap[raw as usize]))
            .collect::<Vec<_>>();

        if welded {
            let split_vertex =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(split_point);
            faces.extend(generated.into_iter().map(|(canonical, forward)| {
                let mut triangle =
                    canonical.map(|raw| raw.map_or(split_vertex, |raw| raw_remap[raw as usize]));
                if !forward {
                    triangle.swap(1, 2);
                }
                MeshFace::Triangle(triangle)
            }));
        } else {
            for (canonical, forward) in generated {
                let mut triangle = [0_u32; 3];
                for (target, raw) in triangle.iter_mut().zip(canonical) {
                    *target = u32::try_from(vertices.len())
                        .map_err(|_| GeometryError::TooManyMeshVertices)?;
                    vertices.push(raw.map_or(split_point, |raw| self.vertices[raw as usize]));
                }
                if !forward {
                    triangle.swap(1, 2);
                }
                faces.push(MeshFace::Triangle(triangle));
            }
        }
        Ok(Some(Self::try_new_faces(vertices, faces, tolerance)?))
    }

    /// Fills the closed naked boundary containing one topology edge.
    ///
    /// The selected index follows [`Self::wireframe_lines`]. `None` means the
    /// edge is not naked, its boundary branches or does not close, or its
    /// projected polygon cannot be constrained-triangulated. Boundary points
    /// are copied into a separate raw-vertex patch, including Rhino's unused
    /// closing duplicate in the joined result. Exact-location topology still
    /// joins the patch to the source. Generated winding is made opposite the
    /// face at the selected edge so a consistently oriented input remains
    /// consistently oriented after filling.
    pub fn fill_topology_hole(
        &self,
        edge_index: usize,
        tolerance: Tolerance,
    ) -> Result<Option<MeshHoleFill>, GeometryError> {
        let data = self.topology_data();
        let edge_count = data.edges.len();
        let Some((&selected_edge, selected_incidence)) = data.edges.iter().nth(edge_index) else {
            return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_index,
                edge_count,
            });
        };
        if selected_incidence.count != 1 {
            return Ok(None);
        }

        let naked_edges = data
            .edges
            .iter()
            .filter_map(|(&edge, incidence)| (incidence.count == 1).then_some(edge))
            .collect::<Vec<_>>();
        let Some(selected_naked_edge) = naked_edges
            .iter()
            .position(|&candidate| candidate == selected_edge)
        else {
            return Ok(None);
        };
        let mut adjacency = vec![Vec::new(); data.topological_vertex_count];
        for (naked_edge_index, &(first, second)) in naked_edges.iter().enumerate() {
            adjacency[first].push(naked_edge_index);
            adjacency[second].push(naked_edge_index);
        }

        let mut boundary = Vec::new();
        let start = selected_edge.0;
        let mut current = start;
        let mut current_edge = selected_naked_edge;
        let mut used = vec![false; naked_edges.len()];
        loop {
            if used[current_edge] {
                return Ok(None);
            }
            used[current_edge] = true;
            boundary.push(current);
            let (first, second) = naked_edges[current_edge];
            let next = if current == first {
                second
            } else if current == second {
                first
            } else {
                return Ok(None);
            };
            if next == start {
                break;
            }
            if adjacency[next].len() != 2 {
                return Ok(None);
            }
            let Some(next_edge) = adjacency[next]
                .iter()
                .copied()
                .find(|&candidate| !used[candidate])
            else {
                return Ok(None);
            };
            current = next;
            current_edge = next_edge;
        }
        if boundary.len() < 3
            || adjacency[start].len() != 2
            || boundary.iter().copied().collect::<BTreeSet<_>>().len() != boundary.len()
        {
            return Ok(None);
        }

        let boundary_points = boundary
            .iter()
            .map(|&vertex| data.topological_points[vertex])
            .collect::<Vec<_>>();
        let Some(projected) = project_mesh_hole_boundary(&boundary_points)? else {
            return Ok(None);
        };
        let Some(mut patch_triangles) = triangulate_projected_mesh_hole(&projected)? else {
            return Ok(None);
        };
        let boundary_area = projected_polygon_doubled_area(&projected);
        let selected_forward = selected_incidence
            .first_use
            .expect("a naked edge records one face use")
            .forward;
        let desired_positive = (boundary_area > 0.0) != selected_forward;
        for triangle in &mut patch_triangles {
            let points = triangle.map(|vertex| projected[vertex as usize]);
            let positive = mesh_hole_cross(points[0], points[1], points[2]) > 0.0;
            if positive != desired_positive {
                triangle.swap(1, 2);
            }
        }

        let patch_faces = patch_triangles
            .iter()
            .copied()
            .map(MeshFace::Triangle)
            .collect::<Vec<_>>();
        let patch = Self::try_new_faces(boundary_points.clone(), patch_faces, tolerance)?;

        let added_vertex_count = boundary_points.len().saturating_add(1);
        let total_vertex_count = self
            .vertices
            .len()
            .checked_add(added_vertex_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if total_vertex_count
            .checked_sub(1)
            .is_some_and(|last| u32::try_from(last).is_err())
        {
            return Err(GeometryError::TooManyMeshVertices);
        }
        let offset = u32::try_from(self.vertices.len())
            .expect("the checked appended mesh vertex range starts within u32");
        let mut vertices = self.vertices.clone();
        vertices
            .try_reserve(added_vertex_count)
            .map_err(|_| GeometryError::TooManyMeshVertices)?;
        vertices.extend(boundary_points.iter().copied());
        vertices.push(boundary_points[0]);
        let mut faces = self.faces.clone();
        faces
            .try_reserve(patch_triangles.len())
            .map_err(|_| GeometryError::TooManyMeshFaces)?;
        for triangle in patch_triangles {
            let mapped = triangle.map(|vertex| {
                offset
                    .checked_add(vertex)
                    .expect("the reserved mesh vertex range fits in u32")
            });
            faces.push(MeshFace::Triangle(mapped));
        }
        let filled = Self::try_new_faces(vertices, faces, tolerance)?;
        Ok(Some(MeshHoleFill { filled, patch }))
    }

    /// Fills every unambiguous closed naked boundary with triangles.
    ///
    /// Boundaries are processed in deterministic topology-edge order and the
    /// search restarts after each patch because topology indices change. This
    /// mirrors Rhino's all-holes operation, including treating outer naked
    /// borders as holes, while omitting the singular operation's unused
    /// closing duplicate after each boundary. The returned count is the
    /// number of filled boundary loops.
    pub fn fill_holes(&self, tolerance: Tolerance) -> Result<(Self, usize), GeometryError> {
        let mut filled = self.clone();
        let mut filled_hole_count = 0_usize;
        loop {
            let edge_count = filled.topology().edge_count();
            let mut next = None;
            for edge_index in 0..edge_count {
                if let Some(fill) = filled.fill_topology_hole(edge_index, tolerance)? {
                    next = Some(fill.filled);
                    break;
                }
            }
            let Some(mut next) = next else {
                break;
            };
            let closing_duplicate = next
                .vertices
                .pop()
                .expect("a filled boundary appends an unused closing vertex");
            debug_assert!(
                !next
                    .faces
                    .iter()
                    .flat_map(MeshFace::indices)
                    .any(|&vertex| vertex as usize == next.vertices.len()),
                "the closing boundary duplicate is unused"
            );
            debug_assert!(
                next.vertices.contains(&closing_duplicate),
                "the removed vertex duplicates the boundary start"
            );
            filled = Self::from_validated_parts(next.vertices, next.faces);
            filled_hole_count = filled_hole_count
                .checked_add(1)
                .ok_or(GeometryError::TooManyMeshFaces)?;
        }
        Ok((filled, filled_hole_count))
    }

    pub fn triangle_points(&self, index: usize) -> Option<[Point3; 3]> {
        let triangle = *self.triangles.get(index)?;
        Some([
            self.vertices[triangle[0] as usize],
            self.vertices[triangle[1] as usize],
            self.vertices[triangle[2] as usize],
        ])
    }

    /// Returns the point on a logical triangle or quadrilateral face nearest
    /// to `target`. Quadrilaterals use the mesh's deterministic A-C display
    /// diagonal, with exact ties resolved toward the A-B-C half.
    pub fn closest_point_on_face(
        &self,
        index: usize,
        target: Point3,
    ) -> Result<Point3, GeometryError> {
        let face =
            self.faces
                .get(index)
                .copied()
                .ok_or(GeometryError::MeshFaceIndexOutOfRange {
                    face: index,
                    face_count: self.faces.len(),
                })?;
        match face {
            MeshFace::Triangle([a, b, c]) => closest_point_on_triangle(
                target,
                self.vertices[a as usize],
                self.vertices[b as usize],
                self.vertices[c as usize],
            ),
            MeshFace::Quad([a, b, c, d]) => {
                let first = closest_point_on_triangle(
                    target,
                    self.vertices[a as usize],
                    self.vertices[b as usize],
                    self.vertices[c as usize],
                )?;
                let second = closest_point_on_triangle(
                    target,
                    self.vertices[a as usize],
                    self.vertices[c as usize],
                    self.vertices[d as usize],
                )?;
                if second.distance_to(target)? < first.distance_to(target)? {
                    Ok(second)
                } else {
                    Ok(first)
                }
            }
        }
    }

    /// Builds OpenNURBS-compatible edge topology after welding vertices at
    /// exactly equal 3D locations. This recognizes indexed meshes and
    /// triangle-soup imports consistently without applying model tolerance.
    pub fn topology(&self) -> MeshTopology {
        let data = self.topology_data();
        let edges = &data.edges;
        let boundary_edge_count = edges
            .values()
            .filter(|incidence| incidence.count == 1)
            .count();
        let non_manifold_edge_count = edges
            .values()
            .filter(|incidence| incidence.count > 2)
            .count();
        let orientation_conflict_edge_count = edges
            .values()
            .filter(|incidence| {
                incidence.count == 2
                    && (incidence.forward_count == 0 || incidence.forward_count == 2)
            })
            .count();
        MeshTopology {
            topological_vertex_count: data.topological_vertex_count,
            edge_count: edges.len(),
            boundary_edge_count,
            non_manifold_edge_count,
            orientation_conflict_edge_count,
            closed: self.vertices.len() >= 4 && self.faces.len() >= 4 && boundary_edge_count == 0,
        }
    }

    /// Returns exact-location topology vertices in the same deterministic
    /// order used by topology vertex selectors.
    pub fn topology_vertex_points(&self) -> Vec<Point3> {
        self.topology_data().topological_points
    }

    /// Returns every exact-location-welded topology edge exactly once.
    ///
    /// This is the curve set Rhino displays and extracts for a triangle mesh:
    /// shared face edges are not duplicated, while naked and non-manifold
    /// edges remain represented.
    pub fn wireframe_lines(&self, tolerance: Tolerance) -> Result<Vec<LineSegment>, GeometryError> {
        let data = self.topology_data();
        data.edges
            .keys()
            .map(|&(first, second)| {
                LineSegment::try_new(
                    data.topological_points[first],
                    data.topological_points[second],
                    tolerance,
                )
            })
            .collect()
    }

    /// Returns selected exact-location topology edges as canonical lines.
    ///
    /// [`MeshEdgeFilter::Unwelded`] follows Rhino/OpenNURBS terminology: a
    /// naked edge is included, and an interior seam is included only when all
    /// incident faces use distinct raw vertex indices at both endpoints.
    pub fn filtered_edge_lines(
        &self,
        filter: MeshEdgeFilter,
        tolerance: Tolerance,
    ) -> Result<Vec<LineSegment>, GeometryError> {
        validate_mesh_edge_filter(filter)?;
        let data = self.topology_data();
        let face_normals = matches!(filter, MeshEdgeFilter::FaceAngle { .. })
            .then(|| self.polygon_face_normals())
            .transpose()?;
        data.edges
            .iter()
            .filter(|(_, incidence)| {
                mesh_edge_matches_filter(incidence, filter, face_normals.as_deref())
            })
            .map(|(&(first, second), _)| {
                LineSegment::try_new(
                    data.topological_points[first],
                    data.topological_points[second],
                    tolerance,
                )
            })
            .collect()
    }

    /// Joins selected topology edges into deterministic, edge-exact trails.
    ///
    /// Each selected edge appears once. An Euler trail is used when a
    /// connected network has zero or two odd vertices; more highly branched
    /// networks are decomposed into the minimum number of open trails by
    /// temporarily pairing their odd vertices.
    pub fn filtered_edge_polylines(
        &self,
        filter: MeshEdgeFilter,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        validate_mesh_edge_filter(filter)?;
        let data = self.topology_data();
        let face_normals = matches!(filter, MeshEdgeFilter::FaceAngle { .. })
            .then(|| self.polygon_face_normals())
            .transpose()?;
        let edges = data
            .edges
            .iter()
            .filter_map(|(&(first, second), incidence)| {
                mesh_edge_matches_filter(incidence, filter, face_normals.as_deref())
                    .then_some([first, second])
            })
            .collect::<Vec<_>>();
        topology_edge_polylines(&data.topological_points, &edges, tolerance)
    }

    /// Returns the logical mesh edges induced by a face break angle.
    ///
    /// Naked and unwelded topology edges always form boundaries. A welded
    /// interior edge forms a boundary when at least two of its incident face
    /// normals meet at or above `break_angle_radians`; angles strictly below
    /// the threshold are treated as one smooth face region. At each topology
    /// vertex, boundary segments join only when the locally adjacent smooth
    /// face regions agree. This keeps a crease separate from an intersecting
    /// crease while allowing a tessellated open border to become one polyline.
    pub fn logical_edge_polylines(
        &self,
        break_angle_radians: Real,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        validate_mesh_break_angle(break_angle_radians)?;
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let edge_records = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let boundary_edges = edge_records
            .iter()
            .map(|(_, incidence)| {
                mesh_edge_is_logical_boundary(incidence, &face_normals, break_angle_radians)
            })
            .collect::<Vec<_>>();
        if !boundary_edges.iter().any(|&boundary| boundary) {
            return Ok(Vec::new());
        }

        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edge_records.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
        let mut endpoint_signatures = vec![[Vec::new(), Vec::new()]; edge_records.len()];
        for (vertex, incident) in incident_edges.iter().enumerate() {
            let mut faces = incident
                .iter()
                .flat_map(|&edge| edge_records[edge].1.uses().map(|edge_use| edge_use.face))
                .collect::<Vec<_>>();
            faces.sort_unstable();
            faces.dedup();
            let mut parents = (0..faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; faces.len()];
            for &edge in incident {
                if boundary_edges[edge] {
                    continue;
                }
                let mut uses = edge_records[edge].1.uses();
                let Some(first) = uses.next() else {
                    continue;
                };
                let first = faces
                    .binary_search(&first.face)
                    .expect("an incident edge face is present at its vertex");
                for edge_use in uses {
                    let face = faces
                        .binary_search(&edge_use.face)
                        .expect("an incident edge face is present at its vertex");
                    union_faces(&mut parents, &mut ranks, first, face);
                }
            }

            for &edge in incident {
                if !boundary_edges[edge] {
                    continue;
                }
                let mut signature = edge_records[edge]
                    .1
                    .uses()
                    .map(|edge_use| {
                        let face = faces
                            .binary_search(&edge_use.face)
                            .expect("an incident edge face is present at its vertex");
                        face_root(&mut parents, face)
                    })
                    .collect::<Vec<_>>();
                if edge_records[edge].1.count == 1 {
                    // The exterior is a distinct local region after every real
                    // incident face. Its stable sentinel lets adjacent naked
                    // edges join without colliding with a face-region root.
                    signature.push(faces.len());
                }
                signature.sort_unstable();
                let endpoint = usize::from(edge_records[edge].0[1] == vertex);
                endpoint_signatures[edge][endpoint] = signature;
            }
        }

        let mut logical_vertex_indices = BTreeMap::<(usize, Vec<usize>), usize>::new();
        let mut logical_points = Vec::new();
        let mut logical_edges = Vec::new();
        for (edge, ((vertices, _), boundary)) in edge_records.iter().zip(boundary_edges).enumerate()
        {
            if !boundary {
                continue;
            }
            let mut logical_vertices = [0_usize; 2];
            for endpoint in 0..2 {
                let key = (
                    vertices[endpoint],
                    endpoint_signatures[edge][endpoint].clone(),
                );
                logical_vertices[endpoint] = if let Some(&index) = logical_vertex_indices.get(&key)
                {
                    index
                } else {
                    let index = logical_points.len();
                    logical_points.push(data.topological_points[vertices[endpoint]]);
                    logical_vertex_indices.insert(key, index);
                    index
                };
            }
            logical_edges.push(logical_vertices);
        }
        topology_edge_polylines(&logical_points, &logical_edges, tolerance)
    }

    /// Returns each exact-location-welded naked border as a polyline.
    ///
    /// Manifold boundaries are returned as closed, face-oriented loops.
    /// Non-manifold boundary graphs are split deterministically into maximal
    /// trails at branch vertices, with every edge represented exactly once.
    pub fn boundary_polylines(
        &self,
        tolerance: Tolerance,
    ) -> Result<Vec<Polyline3>, GeometryError> {
        let data = self.topology_data();
        let boundary_edges = data
            .edges
            .iter()
            .filter(|(_, incidence)| incidence.count == 1)
            .map(|(&(first, second), incidence)| {
                if incidence
                    .first_use
                    .expect("a boundary edge records its face use")
                    .forward
                {
                    [first, second]
                } else {
                    [second, first]
                }
            })
            .collect::<Vec<_>>();
        if boundary_edges.is_empty() {
            return Ok(Vec::new());
        }

        let mut adjacency = vec![Vec::new(); data.topological_vertex_count];
        for (edge_index, [first, second]) in boundary_edges.iter().copied().enumerate() {
            adjacency[first].push(edge_index);
            adjacency[second].push(edge_index);
        }
        let mut used = vec![false; boundary_edges.len()];
        let mut paths = Vec::new();
        for vertex in 0..adjacency.len() {
            if adjacency[vertex].len() == 2 {
                continue;
            }
            for edge in adjacency[vertex].iter().copied() {
                if !used[edge] {
                    paths.push(trace_boundary_path(
                        vertex,
                        edge,
                        &boundary_edges,
                        &adjacency,
                        &mut used,
                    ));
                }
            }
        }
        for edge in 0..boundary_edges.len() {
            if !used[edge] {
                paths.push(trace_boundary_path(
                    boundary_edges[edge][0],
                    edge,
                    &boundary_edges,
                    &adjacency,
                    &mut used,
                ));
            }
        }

        paths
            .into_iter()
            .map(|path| {
                Polyline3::try_new(
                    path.into_iter()
                        .map(|vertex| data.topological_points[vertex])
                        .collect(),
                    tolerance,
                )
            })
            .collect()
    }

    /// Reorients each manifold-connected face component consistently while
    /// retaining face and vertex order. Exact coincident locations define
    /// adjacency, as they do for [`Self::topology`]. Non-manifold edges do not
    /// impose an ambiguous orientation constraint.
    pub fn unified_face_orientations(&self) -> Result<(Self, usize), GeometryError> {
        let data = self.topology_data();
        let mut neighbors = vec![Vec::<(usize, bool)>::new(); self.faces.len()];
        for incidence in data.edges.values() {
            if incidence.count != 2 {
                continue;
            }
            let first = incidence
                .first_use
                .expect("an edge used twice records its first face");
            let second = incidence
                .second_use
                .expect("an edge used twice records its second face");
            // Equal traversal directions require exactly one adjacent face to
            // flip; opposite directions require both to retain equal parity.
            let opposite_parity = first.forward == second.forward;
            neighbors[first.face].push((second.face, opposite_parity));
            neighbors[second.face].push((first.face, opposite_parity));
        }

        let mut flipped = vec![None; self.faces.len()];
        let mut pending = VecDeque::new();
        for root in 0..self.faces.len() {
            if flipped[root].is_some() {
                continue;
            }
            flipped[root] = Some(false);
            pending.push_back(root);
            while let Some(face) = pending.pop_front() {
                let face_flipped = flipped[face].expect("queued faces have assigned parity");
                for &(neighbor, opposite_parity) in &neighbors[face] {
                    let expected = face_flipped ^ opposite_parity;
                    match flipped[neighbor] {
                        Some(actual) if actual != expected => {
                            return Err(GeometryError::NonOrientableMesh);
                        }
                        Some(_) => {}
                        None => {
                            flipped[neighbor] = Some(expected);
                            pending.push_back(neighbor);
                        }
                    }
                }
            }
        }

        let mut faces = self.faces.clone();
        let mut flipped_face_count = 0;
        for (face, flip) in faces.iter_mut().zip(flipped) {
            if flip.expect("every face component receives an orientation") {
                *face = face.reversed();
                flipped_face_count += 1;
            }
        }
        Ok((
            Self::from_validated_parts(self.vertices.clone(), faces),
            flipped_face_count,
        ))
    }

    /// Splits the mesh into exact-location edge-connected components. A lone
    /// shared vertex does not connect faces. Each result retains source face
    /// order and compacts referenced raw vertices in first-use order.
    pub fn disjoint_pieces(&self) -> Vec<Self> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            for edge_use in uses {
                union_faces(&mut parents, &mut ranks, first.face, edge_use.face);
            }
        }

        let mut component_by_root = BTreeMap::new();
        let mut component_faces = Vec::<Vec<usize>>::new();
        for face in 0..self.faces.len() {
            let root = face_root(&mut parents, face);
            let component_count = component_faces.len();
            let component = *component_by_root.entry(root).or_insert_with(|| {
                component_faces.push(Vec::new());
                component_count
            });
            component_faces[component].push(face);
        }

        component_faces
            .into_iter()
            .map(|faces| self.piece_from_faces(&faces))
            .collect()
    }

    /// Splits the mesh into the parts Rhino's `Explode` command sees across
    /// unwelded edges.
    ///
    /// Exact coincident positions establish topological edges. Such an edge
    /// remains welded when its incident faces reuse a raw vertex index at
    /// either endpoint; it is unwelded only when every incident use has a
    /// distinct raw index at both endpoints. A lone shared vertex never joins
    /// parts. Results retain source face order and preserve logical quads.
    pub fn explode_pieces(&self) -> Vec<Self> {
        let data = self.topology_data();
        let mut parents = (0..self.faces.len()).collect::<Vec<_>>();
        let mut ranks = vec![0_u8; self.faces.len()];
        for incidence in data.edges.values() {
            let uses = incidence.uses().collect::<Vec<_>>();
            if uses.len() == 1 || edge_uses_are_unwelded(&uses) {
                continue;
            }
            for edge_use in &uses[1..] {
                union_faces(&mut parents, &mut ranks, uses[0].face, edge_use.face);
            }
        }

        let mut component_by_root = BTreeMap::new();
        let mut component_faces = Vec::<Vec<usize>>::new();
        for face in 0..self.faces.len() {
            let root = face_root(&mut parents, face);
            let component_count = component_faces.len();
            let component = *component_by_root.entry(root).or_insert_with(|| {
                component_faces.push(Vec::new());
                component_count
            });
            component_faces[component].push(face);
        }

        component_faces
            .into_iter()
            .map(|faces| self.piece_from_faces(&faces))
            .collect()
    }

    /// Removes the requested source faces into a separate mesh.
    ///
    /// The extracted face order follows `face_indices`, while the remainder
    /// retains source face order. Both meshes discard unused vertices and keep
    /// their retained vertices in source order, matching RhinoCommon's
    /// `MeshFaceList.ExtractFaces` behavior.
    pub fn extract_faces(
        &self,
        face_indices: &[usize],
    ) -> Result<MeshFaceExtraction, GeometryError> {
        let extracted_mask = self.face_subset_mask(face_indices)?;
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !extracted_mask[face])
            .collect::<Vec<_>>();
        Ok(MeshFaceExtraction {
            remainder: (!remainder_faces.is_empty())
                .then(|| self.subset_preserving_vertex_order(&remainder_faces)),
            extracted: self.subset_preserving_vertex_order(face_indices),
        })
    }

    /// Deletes a non-empty, unique source-face subset and compacts the
    /// remainder. Surviving faces and vertices retain source order. `None`
    /// represents deleting every face, because an empty mesh is not valid.
    pub fn delete_faces(&self, face_indices: &[usize]) -> Result<Option<Self>, GeometryError> {
        let deleted_mask = self.face_subset_mask(face_indices)?;
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !deleted_mask[face])
            .collect::<Vec<_>>();
        Ok((!remainder_faces.is_empty())
            .then(|| self.subset_preserving_vertex_order(&remainder_faces)))
    }

    fn face_subset_mask(&self, face_indices: &[usize]) -> Result<Vec<bool>, GeometryError> {
        if face_indices.is_empty() {
            return Err(GeometryError::EmptyMeshFaceSubset);
        }
        let mut selected = vec![false; self.faces.len()];
        for &face in face_indices {
            if face >= self.faces.len() {
                return Err(GeometryError::MeshFaceIndexOutOfRange {
                    face,
                    face_count: self.faces.len(),
                });
            }
            if std::mem::replace(&mut selected[face], true) {
                return Err(GeometryError::DuplicateMeshFaceIndex { face });
            }
        }
        Ok(selected)
    }

    /// Removes faces around exact-location edges used by at least
    /// `minimum_face_count` faces. With `hanging_faces_only`, a qualifying
    /// face must also touch an edge used by exactly one face.
    pub fn extract_non_manifold_faces(
        &self,
        minimum_face_count: usize,
        hanging_faces_only: bool,
    ) -> Result<Option<MeshFaceExtraction>, GeometryError> {
        if minimum_face_count < 3 {
            return Err(GeometryError::InvalidNonManifoldMinimumFaceCount(
                minimum_face_count,
            ));
        }
        let data = self.topology_data();
        let mut touches_qualifying_edge = vec![false; self.faces.len()];
        let mut touches_boundary_edge = vec![false; self.faces.len()];
        for incidence in data.edges.values() {
            if incidence.count >= minimum_face_count {
                for edge_use in incidence.uses() {
                    touches_qualifying_edge[edge_use.face] = true;
                }
            }
            if incidence.count == 1 {
                let edge_use = incidence
                    .first_use
                    .expect("a boundary edge records its incident face");
                touches_boundary_edge[edge_use.face] = true;
            }
        }
        let extracted_mask = touches_qualifying_edge
            .into_iter()
            .zip(touches_boundary_edge)
            .map(|(qualifying, boundary)| qualifying && (!hanging_faces_only || boundary))
            .collect::<Vec<_>>();
        let extracted_faces = extracted_mask
            .iter()
            .enumerate()
            .filter_map(|(face, &extracted)| extracted.then_some(face))
            .collect::<Vec<_>>();
        if extracted_faces.is_empty() {
            return Ok(None);
        }
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !extracted_mask[face])
            .collect::<Vec<_>>();
        let extracted = self.subset_preserving_vertex_order(&extracted_faces);
        let remainder = (!remainder_faces.is_empty())
            .then(|| self.subset_preserving_vertex_order(&remainder_faces));
        Ok(Some(MeshFaceExtraction {
            remainder,
            extracted,
        }))
    }

    /// Extracts all but one face from every exact-location duplicate class.
    /// Vertex indices, cyclic order, and winding do not affect equality. The
    /// first source face in each class is retained so the result is stable.
    pub fn extract_duplicate_faces(&self) -> Option<MeshFaceExtraction> {
        let mut representative_locations = BTreeSet::new();
        let mut extracted_mask = vec![false; self.faces.len()];
        for (face_index, face) in self.faces.iter().enumerate() {
            let mut locations = face
                .indices()
                .iter()
                .map(|&vertex| {
                    self.vertices[vertex as usize]
                        .to_array()
                        .map(canonical_coordinate_bits)
                })
                .collect::<Vec<_>>();
            locations.sort_unstable();
            if !representative_locations.insert(locations) {
                extracted_mask[face_index] = true;
            }
        }
        let extracted_faces = extracted_mask
            .iter()
            .enumerate()
            .filter_map(|(face, &extracted)| extracted.then_some(face))
            .collect::<Vec<_>>();
        if extracted_faces.is_empty() {
            return None;
        }
        let remainder_faces = (0..self.faces.len())
            .filter(|&face| !extracted_mask[face])
            .collect::<Vec<_>>();
        Some(MeshFaceExtraction {
            remainder: Some(self.subset_preserving_vertex_order(&remainder_faces)),
            extracted: self.subset_preserving_vertex_order(&extracted_faces),
        })
    }

    /// Merges vertices at exactly equal locations, ignoring derived normals
    /// and attributes that this mesh representation does not store. Matching
    /// OpenNURBS behavior, a changed mesh sorts unique vertices by descending
    /// `(x, y, z)` while a mesh with no duplicates remains byte-for-byte equal.
    pub fn combined_identical_vertices(&self) -> (Self, usize) {
        let mut unique_by_location = BTreeMap::new();
        for &vertex in &self.vertices {
            let key = vertex.to_array().map(canonical_coordinate_bits);
            unique_by_location.entry(key).or_insert(vertex);
        }
        let removed = self.vertices.len() - unique_by_location.len();
        if removed == 0 {
            return (self.clone(), 0);
        }

        let mut vertices = unique_by_location.into_values().collect::<Vec<_>>();
        vertices.sort_by(compare_points_descending);
        let index_by_location = vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| {
                (
                    vertex.to_array().map(canonical_coordinate_bits),
                    u32::try_from(index)
                        .expect("a combined mesh cannot have more vertices than its source"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| {
                face.remapped(|source| {
                    let key = self.vertices[source as usize]
                        .to_array()
                        .map(canonical_coordinate_bits);
                    index_by_location[&key]
                })
            })
            .collect();
        (Self::from_validated_parts(vertices, faces), removed)
    }

    /// Welds coincident edge endpoints whose incident face normals fall
    /// within the supplied angular tolerance.
    ///
    /// Only vertices paired along an exact-location topology edge can merge;
    /// a coincident vertex-only contact remains distinct. The later raw vertex
    /// is retained as Rhino/OpenNURBS does. Non-manifold edges consider their
    /// first two face uses only. All unreferenced vertices are then compacted
    /// while the remaining source order is preserved.
    pub fn welded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshWeldAngle);
        }
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let minimum_dot = angle_tolerance_radians.cos();
        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        for incidence in data.edges.values() {
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            let Some(second) = uses.next() else {
                continue;
            };
            let dot = face_normals[first.face]
                .as_vector()
                .dot(face_normals[second.face].as_vector())?
                .clamp(-1.0, 1.0);
            if dot < minimum_dot {
                continue;
            }
            for endpoint in 0..2 {
                union_indices_keep_later(
                    &mut parents,
                    first.raw_vertices[endpoint] as usize,
                    second.raw_vertices[endpoint] as usize,
                );
            }
        }

        Ok(self.compacted_with_vertex_parents(&mut parents))
    }

    /// Welds coincident raw endpoint sets along selected exact-location
    /// topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Only raw vertices used by faces incident to a selected edge are merged;
    /// other coincident fan components remain separate. The earliest source
    /// raw vertex survives, and a non-empty valid selection compacts unused
    /// vertices. The returned count is the number of selected edges that had
    /// at least one divided endpoint set.
    pub fn welded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if edge_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let edges = data.edges.values().collect::<Vec<_>>();
        let mut selected_edges = vec![false; edges.len()];
        for &edge in edge_indices {
            let Some(selected) = selected_edges.get_mut(edge) else {
                return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                    edge,
                    edge_count: edges.len(),
                });
            };
            *selected = true;
        }

        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        let mut welded_edge_count = 0;
        for (incidence, selected) in edges.into_iter().zip(selected_edges) {
            if !selected {
                continue;
            }
            let uses = incidence.uses().collect::<Vec<_>>();
            let mut divided_endpoint = false;
            for endpoint in 0..2 {
                let raw_vertices = uses
                    .iter()
                    .map(|edge_use| edge_use.raw_vertices[endpoint] as usize)
                    .collect::<BTreeSet<_>>();
                divided_endpoint |= raw_vertices.len() > 1;
                if let Some(&first) = raw_vertices.first() {
                    for &raw in raw_vertices.iter().skip(1) {
                        union_indices_keep_earlier(&mut parents, first, raw);
                    }
                }
            }
            welded_edge_count += usize::from(divided_endpoint);
        }
        let (welded, _) = self.compacted_with_vertex_parents(&mut parents);
        Ok((welded, welded_edge_count))
    }

    /// Welds joined mesh seams incident to selected exact-location topology
    /// vertices.
    ///
    /// Indices use the same deterministic order as
    /// [`Self::topology_vertex_points`]. Selecting either endpoint of a seam
    /// welds both endpoint pairs on every incident edge, matching Rhino's
    /// vertex tool. On a non-manifold edge only the first two face uses are
    /// joined. The later raw vertex survives, and a non-empty valid selection
    /// compacts unused vertices. The returned count is the number of incident
    /// edges that required welding.
    pub fn welded_topology_vertices(
        &self,
        vertex_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if vertex_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let mut selected_vertices = vec![false; data.topological_vertex_count];
        for &vertex in vertex_indices {
            let Some(selected) = selected_vertices.get_mut(vertex) else {
                return Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                    vertex,
                    vertex_count: data.topological_vertex_count,
                });
            };
            *selected = true;
        }

        let mut parents = (0..self.vertices.len()).collect::<Vec<_>>();
        let mut welded_edge_count = 0;
        for (&(first_vertex, second_vertex), incidence) in &data.edges {
            if !selected_vertices[first_vertex] && !selected_vertices[second_vertex] {
                continue;
            }
            let mut uses = incidence.uses();
            let Some(first) = uses.next() else {
                continue;
            };
            let Some(second) = uses.next() else {
                continue;
            };
            let divided = (0..2)
                .any(|endpoint| first.raw_vertices[endpoint] != second.raw_vertices[endpoint]);
            if !divided {
                continue;
            }
            for endpoint in 0..2 {
                union_indices_keep_later(
                    &mut parents,
                    first.raw_vertices[endpoint] as usize,
                    second.raw_vertices[endpoint] as usize,
                );
            }
            welded_edge_count += 1;
        }
        let (welded, _) = self.compacted_with_vertex_parents(&mut parents);
        Ok((welded, welded_edge_count))
    }

    /// Separates coincident edge endpoints where the incident-face normal
    /// angle is greater than or equal to the supplied tolerance.
    ///
    /// Face regions that remain connected through smoother, already-welded
    /// edges continue to share a raw vertex. Qualifying topology vertices are
    /// rebuilt in OpenNURBS radial order, and unused vertices are compacted as
    /// Rhino does even when no edge qualifies. The returned count is the
    /// number of qualifying topology edges.
    pub fn unwelded_vertices(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<(Self, usize), GeometryError> {
        if !(angle_tolerance_radians.is_finite()
            && (0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians))
        {
            return Err(GeometryError::InvalidMeshUnweldAngle);
        }
        let data = self.topology_data();
        let face_normals = self.polygon_face_normals()?;
        let maximum_dot = angle_tolerance_radians.cos();
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut qualifying_edges = vec![false; edges.len()];
        for (edge_index, (_, incidence)) in edges.iter().enumerate() {
            let uses = incidence.uses().collect::<Vec<_>>();
            'pairs: for left in 0..uses.len().saturating_sub(1) {
                for right in left + 1..uses.len() {
                    let dot = face_normals[uses[left].face]
                        .as_vector()
                        .dot(face_normals[uses[right].face].as_vector())?
                        .clamp(-1.0, 1.0);
                    if dot <= maximum_dot {
                        qualifying_edges[edge_index] = true;
                        break 'pairs;
                    }
                }
            }
        }
        let qualifying_edge_count = qualifying_edges.iter().filter(|&&edge| edge).count();
        if qualifying_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        for (edge, qualifies) in edges.iter().zip(&qualifying_edges) {
            if *qualifies {
                affected_topological_vertices[edge.0[0]] = true;
                affected_topological_vertices[edge.0[1]] = true;
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
        let mut edge_groups = vec![Vec::new(); data.topological_vertex_count];
        for vertex in 0..data.topological_vertex_count {
            if affected_topological_vertices[vertex] {
                edge_groups[vertex] = radially_sorted_vertex_edges(
                    vertex,
                    &incident_edges[vertex],
                    &edges,
                    &face_edges,
                );
            }
        }

        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                incident_faces[data.topological_vertices[raw as usize]].push(face);
            }
        }
        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_topological_vertices[topological_vertex] {
                continue;
            }
            let vertex_faces = &incident_faces[topological_vertex];
            let face_to_local = vertex_faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for &edge in &incident_edges[topological_vertex] {
                let (edge_vertices, incidence) = edges[edge];
                let endpoint = if edge_vertices[0] == topological_vertex {
                    Some(0)
                } else if edge_vertices[1] == topological_vertex {
                    Some(1)
                } else {
                    None
                };
                let Some(endpoint) = endpoint else {
                    continue;
                };
                let uses = incidence.uses().collect::<Vec<_>>();
                for left in 0..uses.len().saturating_sub(1) {
                    for right in left + 1..uses.len() {
                        if uses[left].raw_vertices[endpoint] != uses[right].raw_vertices[endpoint] {
                            continue;
                        }
                        let dot = face_normals[uses[left].face]
                            .as_vector()
                            .dot(face_normals[uses[right].face].as_vector())?
                            .clamp(-1.0, 1.0);
                        if dot > maximum_dot {
                            union_faces(
                                &mut parents,
                                &mut ranks,
                                face_to_local[&uses[left].face],
                                face_to_local[&uses[right].face],
                            );
                        }
                    }
                }
            }

            let component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            for component in component_order {
                let mut faces = Vec::new();
                for &face in vertex_faces {
                    let local = face_to_local[&face];
                    if face_root(&mut parents, local) == component {
                        faces.push(face);
                    }
                }
                face_components[topological_vertex].push(faces);
            }
        }
        let vertex_order = (0..data.topological_vertex_count).collect::<Vec<_>>();
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            qualifying_edge_count,
        ))
    }

    /// Separates the supplied exact-location topology edges.
    ///
    /// Indices use the same deterministic order as [`Self::wireframe_lines`].
    /// Naked and already-unwelded edges require no new seams, but a non-empty
    /// valid selection still compacts unused vertices as Rhino does. The
    /// returned count is the number of distinct edges that required a new
    /// separation.
    pub fn unwelded_topology_edges(
        &self,
        edge_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if edge_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut selected_edges = vec![false; edges.len()];
        for &edge in edge_indices {
            let Some(selected) = selected_edges.get_mut(edge) else {
                return Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                    edge,
                    edge_count: edges.len(),
                });
            };
            *selected = true;
        }
        let active_edges = edges
            .iter()
            .zip(selected_edges)
            .map(|((_, incidence), selected)| {
                selected
                    && incidence.count > 1
                    && !edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
            })
            .collect::<Vec<_>>();
        let active_edge_count = active_edges.iter().filter(|&&active| active).count();
        if active_edge_count == 0 {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut affected_topological_vertices = vec![false; data.topological_vertex_count];
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
            if active_edges[edge] {
                affected_topological_vertices[vertices[0]] = true;
                affected_topological_vertices[vertices[1]] = true;
            }
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut edge_groups = vec![Vec::new(); data.topological_vertex_count];
        for vertex in 0..data.topological_vertex_count {
            if affected_topological_vertices[vertex] {
                edge_groups[vertex] = radially_sorted_vertex_edges(
                    vertex,
                    &incident_edges[vertex],
                    &edges,
                    &face_edges,
                );
            }
        }
        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                incident_faces[data.topological_vertices[raw as usize]].push(face);
            }
        }

        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_topological_vertices[topological_vertex] {
                continue;
            }
            let vertex_faces = &incident_faces[topological_vertex];
            let face_to_local = vertex_faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let mut separated_edges = active_edges.clone();
            for group in &edge_groups[topological_vertex] {
                let selected = group
                    .iter()
                    .enumerate()
                    .filter_map(|(position, &edge)| active_edges[edge].then_some(position))
                    .collect::<Vec<_>>();
                let closed_manifold = group.iter().all(|&edge| edges[edge].1.count == 2);
                if closed_manifold && selected.len() == 1 {
                    separated_edges[group[(selected[0] + 1) % group.len()]] = true;
                }
            }

            let mut parents = (0..vertex_faces.len()).collect::<Vec<_>>();
            let mut ranks = vec![0_u8; vertex_faces.len()];
            for &edge in &incident_edges[topological_vertex] {
                if separated_edges[edge] {
                    continue;
                }
                let (edge_vertices, incidence) = edges[edge];
                let endpoint = usize::from(edge_vertices[1] == topological_vertex);
                debug_assert_eq!(edge_vertices[endpoint], topological_vertex);
                let uses = incidence.uses().collect::<Vec<_>>();
                for left in 0..uses.len().saturating_sub(1) {
                    for right in left + 1..uses.len() {
                        if uses[left].raw_vertices[endpoint] == uses[right].raw_vertices[endpoint] {
                            union_faces(
                                &mut parents,
                                &mut ranks,
                                face_to_local[&uses[left].face],
                                face_to_local[&uses[right].face],
                            );
                        }
                    }
                }
            }

            let mut component_order = ordered_vertex_face_components(
                &edge_groups[topological_vertex],
                &edges,
                &face_to_local,
                vertex_faces,
                &mut parents,
            );
            component_order.reverse();
            for component in component_order {
                let mut faces = Vec::new();
                for &face in vertex_faces {
                    if face_root(&mut parents, face_to_local[&face]) == component {
                        faces.push(face);
                    }
                }
                face_components[topological_vertex].push(faces);
            }
        }

        let vertex_order =
            topology_edge_vertex_order(data.topological_vertex_count, &edges, &active_edges);
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            active_edge_count,
        ))
    }

    /// Gives every face incident to the supplied exact-location topology
    /// vertices its own raw mesh vertex.
    ///
    /// Indices use the same deterministic order as
    /// [`Self::topology_vertex_points`]. A non-empty valid selection compacts
    /// unused vertices, and selected vertices with multiple incident faces are
    /// rebuilt in OpenNURBS radial order even when they were already unwelded.
    /// The returned count is the number of selected topology vertices that
    /// required at least one new face-local raw vertex.
    pub fn unwelded_topology_vertices(
        &self,
        vertex_indices: &[usize],
    ) -> Result<(Self, usize), GeometryError> {
        if vertex_indices.is_empty() {
            return Ok((self.clone(), 0));
        }
        let data = self.topology_data();
        let mut selected_vertices = vec![false; data.topological_vertex_count];
        for &vertex in vertex_indices {
            let Some(selected) = selected_vertices.get_mut(vertex) else {
                return Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                    vertex,
                    vertex_count: data.topological_vertex_count,
                });
            };
            *selected = true;
        }

        let mut incident_faces = vec![Vec::new(); data.topological_vertex_count];
        for (face, polygon) in self.faces.iter().enumerate() {
            for &raw in polygon.indices() {
                let topological_vertex = data.topological_vertices[raw as usize];
                if incident_faces[topological_vertex].last().copied() != Some(face) {
                    incident_faces[topological_vertex].push(face);
                }
            }
        }
        let affected_vertices = selected_vertices
            .iter()
            .zip(&incident_faces)
            .map(|(&selected, faces)| selected && faces.len() > 1)
            .collect::<Vec<_>>();
        if !affected_vertices.iter().any(|&affected| affected) {
            return Ok((self.culled_unused_vertices().0, 0));
        }

        let mut newly_separated_vertex_count = 0;
        for (topological_vertex, faces) in incident_faces.iter().enumerate() {
            if !affected_vertices[topological_vertex] {
                continue;
            }
            let raw_vertices = faces
                .iter()
                .map(|&face| {
                    self.faces[face]
                        .indices()
                        .iter()
                        .copied()
                        .find(|&raw| data.topological_vertices[raw as usize] == topological_vertex)
                        .expect("an incident face contains its topology vertex")
                })
                .collect::<BTreeSet<_>>();
            if raw_vertices.len() < faces.len() {
                newly_separated_vertex_count += 1;
            }
        }

        let edges = data
            .edges
            .iter()
            .map(|(&(first, second), incidence)| ([first, second], incidence))
            .collect::<Vec<_>>();
        let mut incident_edges = vec![Vec::new(); data.topological_vertex_count];
        for (edge, (vertices, _)) in edges.iter().enumerate() {
            incident_edges[vertices[0]].push(edge);
            incident_edges[vertices[1]].push(edge);
        }
        let face_edges = topology_face_edge_indices(self, &data);
        let mut face_components = vec![Vec::new(); data.topological_vertex_count];
        for topological_vertex in 0..data.topological_vertex_count {
            if !affected_vertices[topological_vertex] {
                continue;
            }
            let faces = &incident_faces[topological_vertex];
            let face_to_local = faces
                .iter()
                .enumerate()
                .map(|(local, &face)| (face, local))
                .collect::<BTreeMap<_, _>>();
            let edge_groups = radially_sorted_vertex_edges(
                topological_vertex,
                &incident_edges[topological_vertex],
                &edges,
                &face_edges,
            );
            let mut parents = (0..faces.len()).collect::<Vec<_>>();
            for component in ordered_vertex_face_components(
                &edge_groups,
                &edges,
                &face_to_local,
                faces,
                &mut parents,
            ) {
                face_components[topological_vertex].push(vec![faces[component]]);
            }
        }

        let vertex_order = (0..data.topological_vertex_count).collect::<Vec<_>>();
        Ok((
            self.rebuilt_from_face_components(&data, &face_components, &vertex_order)?,
            newly_separated_vertex_count,
        ))
    }

    fn rebuilt_from_face_components(
        &self,
        data: &MeshTopologyData,
        face_components: &[Vec<Vec<usize>>],
        topological_vertex_order: &[usize],
    ) -> Result<Self, GeometryError> {
        let affected_topological_vertices = face_components
            .iter()
            .map(|components| !components.is_empty())
            .collect::<Vec<_>>();
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] = true;
            }
        }
        let mut raw_remap = vec![u32::MAX; self.vertices.len()];
        let mut vertices = Vec::new();
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used || affected_topological_vertices[data.topological_vertices[source]] {
                continue;
            }
            raw_remap[source] =
                u32::try_from(vertices.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
            vertices.push(point);
        }

        let mut face_replacements = vec![BTreeMap::<u32, u32>::new(); self.faces.len()];
        for (face_index, face) in self.faces.iter().enumerate() {
            for &raw_vertex in face.indices() {
                let source = raw_vertex as usize;
                if !affected_topological_vertices[data.topological_vertices[source]] {
                    face_replacements[face_index].insert(raw_vertex, raw_remap[source]);
                }
            }
        }
        for &topological_vertex in topological_vertex_order {
            let components = &face_components[topological_vertex];
            for component in components {
                let target = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                vertices.push(data.topological_points[topological_vertex]);
                for &face in component {
                    let raw_vertex = self.faces[face]
                        .indices()
                        .iter()
                        .copied()
                        .find(|&raw| data.topological_vertices[raw as usize] == topological_vertex)
                        .expect("an incident face contains its topology vertex");
                    face_replacements[face].insert(raw_vertex, target);
                }
            }
        }

        let faces = self
            .faces
            .iter()
            .copied()
            .enumerate()
            .map(|(face, polygon)| {
                polygon.remapped(|raw| {
                    *face_replacements[face]
                        .get(&raw)
                        .expect("every unwelded face vertex has a replacement")
                })
            })
            .collect();
        Ok(Self::from_validated_parts(vertices, faces))
    }

    fn compacted_with_vertex_parents(&self, parents: &mut [usize]) -> (Self, usize) {
        let mut retained = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                let representative = index_root(parents, vertex as usize);
                retained[representative] = true;
            }
        }
        let retained_count = retained.iter().filter(|&&keep| keep).count();
        let removed = self.vertices.len() - retained_count;
        if removed == 0 {
            return (self.clone(), 0);
        }

        let mut representative_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_count);
        for (source, (&point, keep)) in self.vertices.iter().zip(retained).enumerate() {
            if !keep {
                continue;
            }
            representative_remap[source] = u32::try_from(vertices.len())
                .expect("a compacted mesh cannot have more vertices than its source");
            vertices.push(point);
        }
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| {
                face.remapped(|vertex| {
                    let representative = index_root(parents, vertex as usize);
                    representative_remap[representative]
                })
            })
            .collect();
        (Self::from_validated_parts(vertices, faces), removed)
    }

    /// Removes vertices that are not referenced by any face. Referenced
    /// vertices retain their relative source order, and coincident referenced
    /// vertices remain distinct. Face order and winding are unchanged.
    pub fn culled_unused_vertices(&self) -> (Self, usize) {
        let mut used = vec![false; self.vertices.len()];
        for face in &self.faces {
            for &vertex in face.indices() {
                used[vertex as usize] = true;
            }
        }

        let retained_vertex_count = used.iter().filter(|&&is_used| is_used).count();
        let removed_vertex_count = self.vertices.len() - retained_vertex_count;
        if removed_vertex_count == 0 {
            return (self.clone(), 0);
        }

        let mut vertex_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::with_capacity(retained_vertex_count);
        for (source, (&point, is_used)) in self.vertices.iter().zip(used).enumerate() {
            if !is_used {
                continue;
            }
            vertex_remap[source] = u32::try_from(vertices.len())
                .expect("a culled mesh cannot have more vertices than its source");
            vertices.push(point);
        }
        let faces = self
            .faces
            .iter()
            .copied()
            .map(|face| face.remapped(|vertex| vertex_remap[vertex as usize]))
            .collect();
        (
            Self::from_validated_parts(vertices, faces),
            removed_vertex_count,
        )
    }

    fn subset_preserving_vertex_order(&self, faces: &[usize]) -> Self {
        let mut used = vec![false; self.vertices.len()];
        for &face in faces {
            for &vertex in self.faces[face].indices() {
                used[vertex as usize] = true;
            }
        }
        let mut vertex_remap = vec![0_u32; self.vertices.len()];
        let mut vertices = Vec::new();
        for (source, (&point, used)) in self.vertices.iter().zip(used).enumerate() {
            if !used {
                continue;
            }
            vertex_remap[source] = u32::try_from(vertices.len())
                .expect("a mesh subset cannot have more vertices than its source");
            vertices.push(point);
        }
        let retained_faces = faces
            .iter()
            .map(|&face| self.faces[face].remapped(|vertex| vertex_remap[vertex as usize]))
            .collect();
        Self::from_validated_parts(vertices, retained_faces)
    }

    fn piece_from_faces(&self, faces: &[usize]) -> Self {
        let mut vertex_remap = vec![None; self.vertices.len()];
        let mut vertices = Vec::new();
        let mut retained_faces = Vec::with_capacity(faces.len());
        for &face in faces {
            let retained_face = self.faces[face].remapped(|source| {
                let source = source as usize;
                *vertex_remap[source].get_or_insert_with(|| {
                    let target = u32::try_from(vertices.len())
                        .expect("a mesh component cannot have more vertices than its source");
                    vertices.push(self.vertices[source]);
                    target
                })
            });
            retained_faces.push(retained_face);
        }
        Self::from_validated_parts(vertices, retained_faces)
    }

    fn topology_data(&self) -> MeshTopologyData {
        let mut locations = BTreeMap::<[u64; 3], usize>::new();
        let mut topological_points = Vec::new();
        let mut topological_vertices = Vec::with_capacity(self.vertices.len());
        for vertex in &self.vertices {
            let key = vertex.to_array().map(canonical_coordinate_bits);
            let id = *locations.entry(key).or_insert_with(|| {
                let id = topological_points.len();
                topological_points.push(*vertex);
                id
            });
            topological_vertices.push(id);
        }

        let mut edges = BTreeMap::<(usize, usize), EdgeIncidence>::new();
        for (face_index, face) in self.faces.iter().enumerate() {
            let indices = face.indices();
            for side in 0..indices.len() {
                let raw_from = indices[side];
                let raw_to = indices[(side + 1) % indices.len()];
                let from = topological_vertices[raw_from as usize];
                let to = topological_vertices[raw_to as usize];
                debug_assert_ne!(from, to, "validated mesh edge collapsed");
                let (edge, forward, raw_vertices) = if from < to {
                    ((from, to), true, [raw_from, raw_to])
                } else {
                    ((to, from), false, [raw_to, raw_from])
                };
                let incidence = edges.entry(edge).or_default();
                incidence.add_use(EdgeUse {
                    face: face_index,
                    side,
                    forward,
                    raw_vertices,
                });
            }
        }

        MeshTopologyData {
            topological_vertex_count: locations.len(),
            topological_points,
            topological_vertices,
            edges,
        }
    }

    fn polygon_face_normals(&self) -> Result<Vec<UnitVector3>, GeometryError> {
        self.faces
            .iter()
            .map(|face| match *face {
                MeshFace::Triangle([a, b, c]) => self.vertices[a as usize]
                    .vector_to(self.vertices[b as usize])?
                    .cross(self.vertices[a as usize].vector_to(self.vertices[c as usize])?)?
                    .normalized_nonzero(),
                MeshFace::Quad([a, b, c, d]) => self.vertices[a as usize]
                    .vector_to(self.vertices[c as usize])?
                    .cross(self.vertices[b as usize].vector_to(self.vertices[d as usize])?)?
                    .normalized_nonzero(),
            })
            .collect()
    }

    pub fn face_normal(&self, index: usize) -> Result<UnitVector3, GeometryError> {
        let points = self
            .triangle_points(index)
            .ok_or(GeometryError::TriangleIndexOutOfRange { triangle: index })?;
        let first = points[0].vector_to(points[1])?.normalized_nonzero()?;
        let second = points[0].vector_to(points[2])?.normalized_nonzero()?;
        first
            .as_vector()
            .cross(second.as_vector())?
            .normalized_nonzero()
    }

    pub fn area(&self) -> Result<Real, GeometryError> {
        let mut sum = 0.0;
        let mut correction = 0.0;
        for index in 0..self.triangles.len() {
            let points = self
                .triangle_points(index)
                .expect("a validated mesh has valid triangle indices");
            let first = points[0].vector_to(points[1])?;
            let second = points[0].vector_to(points[2])?;
            let area = first.cross(second)?.length()? * 0.5;
            require_finite([area], "mesh face area")?;
            let next = sum + area;
            if sum.abs() >= area.abs() {
                correction += (sum - next) + area;
            } else {
                correction += (area - next) + sum;
            }
            sum = next;
        }
        let area = sum + correction;
        require_finite([area], "mesh area")?;
        Ok(area)
    }

    /// Computes oriented mesh volume. Outward winding is positive and
    /// reversing every face negates the result. A bounding-box-center base
    /// point and normalized coordinates keep large translations from
    /// introducing cancellation or intermediate overflow.
    pub fn signed_volume(&self) -> Result<Real, GeometryError> {
        let mut used = vec![false; self.vertices.len()];
        for triangle in &self.triangles {
            for vertex in triangle {
                used[*vertex as usize] = true;
            }
        }
        let base = BoundingBox3::from_points(
            self.vertices
                .iter()
                .zip(&used)
                .filter_map(|(&point, &used)| used.then_some(point)),
        )?
        .center()?;
        let mut relative_vertices = vec![[0.0; 3]; self.vertices.len()];
        let mut scale: Real = 0.0;
        for (index, (&vertex, &used)) in self.vertices.iter().zip(&used).enumerate() {
            if !used {
                continue;
            }
            let relative = base.vector_to(vertex)?.to_array();
            scale = relative
                .iter()
                .fold(scale, |current, coordinate| current.max(coordinate.abs()));
            relative_vertices[index] = relative;
        }
        debug_assert!(scale > 0.0, "a validated mesh has non-coincident vertices");
        for (relative, used) in relative_vertices.iter_mut().zip(used) {
            if !used {
                continue;
            }
            for coordinate in relative.iter_mut() {
                *coordinate /= scale;
            }
        }

        let mut sum = 0.0;
        let mut correction = 0.0;
        for triangle in &self.triangles {
            let a = relative_vertices[triangle[0] as usize];
            let b = relative_vertices[triangle[1] as usize];
            let c = relative_vertices[triangle[2] as usize];
            let cross = [
                b[1].mul_add(c[2], -b[2] * c[1]),
                b[2].mul_add(c[0], -b[0] * c[2]),
                b[0].mul_add(c[1], -b[1] * c[0]),
            ];
            let determinant = a[0].mul_add(cross[0], a[1].mul_add(cross[1], a[2] * cross[2]));
            let next = sum + determinant;
            if sum.abs() >= determinant.abs() {
                correction += (sum - next) + determinant;
            } else {
                correction += (determinant - next) + sum;
            }
            sum = next;
        }
        let normalized_volume = (sum + correction) / 6.0;
        require_finite([normalized_volume], "mesh volume")?;
        if normalized_volume == 0.0 {
            return Ok(0.0);
        }
        let scaled_square = product_three(normalized_volume.abs(), scale, scale, "mesh volume")?;
        let magnitude = scaled_square * scale;
        require_finite([magnitude], "mesh volume")?;
        Ok(normalized_volume.signum() * magnitude)
    }

    pub fn bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(self.vertices.iter().copied())
            .expect("a validated mesh has triangle vertices")
    }

    pub fn transformed(
        &self,
        transform: AffineTransform3,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let vertices = self
            .vertices
            .iter()
            .map(|point| transform.transform_point(*point))
            .collect::<Result<_, _>>()?;
        Self::try_new_faces(vertices, self.faces.clone(), tolerance)
    }
}

fn triangulate_projected_mesh_hole(
    projected: &[[Real; 2]],
) -> Result<Option<Vec<[u32; 3]>>, GeometryError> {
    if projected.len() > u32::MAX as usize {
        return Err(GeometryError::TooManyMeshVertices);
    }
    let doubled_area = projected_polygon_doubled_area(projected);
    let epsilon = 64.0 * Real::EPSILON * projected.len() as Real;
    if !doubled_area.is_finite() || doubled_area.abs() <= epsilon {
        return Ok(None);
    }

    let vertices = projected
        .iter()
        .enumerate()
        .map(|(source_index, point)| MeshHoleTriangulationVertex {
            position: TriangulationPoint2::new(point[0], point[1]),
            source_index,
        })
        .collect::<Vec<_>>();
    let mut triangulation =
        match ConstrainedDelaunayTriangulation::<MeshHoleTriangulationVertex>::bulk_load(vertices) {
            Ok(triangulation) => triangulation,
            Err(_) => return Ok(None),
        };
    if triangulation.num_vertices() != projected.len() {
        return Ok(None);
    }
    let mut handles = vec![None; projected.len()];
    for vertex in triangulation.vertices() {
        handles[vertex.data().source_index] = Some(vertex.fix());
    }
    for source in 0..projected.len() {
        let Some(from) = handles[source] else {
            return Ok(None);
        };
        let Some(to) = handles[(source + 1) % projected.len()] else {
            return Ok(None);
        };
        let before = triangulation.num_constraints();
        if triangulation.try_add_constraint(from, to).is_empty()
            || triangulation.num_constraints() != before + 1
        {
            return Ok(None);
        }
    }

    let mut triangles = Vec::with_capacity(projected.len().saturating_sub(2));
    let mut actual_area = 0.0;
    let mut area_correction = 0.0;
    for face in triangulation.inner_faces() {
        let face_vertices = face.vertices();
        let face_points = face_vertices.map(|vertex| {
            let point = vertex.data().position;
            [point.x, point.y]
        });
        let centroid = [
            (face_points[0][0] + face_points[1][0] + face_points[2][0]) / 3.0,
            (face_points[0][1] + face_points[1][1] + face_points[2][1]) / 3.0,
        ];
        if !point_in_mesh_hole_polygon(centroid, projected, epsilon) {
            continue;
        }
        let triangle_area = mesh_hole_cross(face_points[0], face_points[1], face_points[2]);
        if triangle_area <= epsilon {
            return Ok(None);
        }
        compensated_mesh_hole_sum(&mut actual_area, &mut area_correction, triangle_area);
        let mut triangle = [0_u32; 3];
        for (target, vertex) in triangle.iter_mut().zip(face_vertices) {
            *target = u32::try_from(vertex.data().source_index)
                .map_err(|_| GeometryError::TooManyMeshVertices)?;
        }
        triangles.push(triangle);
    }
    if triangles.len() != projected.len() - 2 {
        return Ok(None);
    }
    let actual_area = actual_area + area_correction;
    let area_tolerance = 4096.0 * Real::EPSILON * projected.len() as Real;
    if (actual_area - doubled_area.abs()).abs() > area_tolerance {
        return Ok(None);
    }
    Ok(Some(triangles))
}

fn project_mesh_hole_boundary(points: &[Point3]) -> Result<Option<Vec<[Real; 2]>>, GeometryError> {
    let Some(origin) = points.first() else {
        return Ok(None);
    };
    let direct = points
        .iter()
        .map(|point| {
            [
                point.x() - origin.x(),
                point.y() - origin.y(),
                point.z() - origin.z(),
            ]
        })
        .collect::<Vec<_>>();
    let mut relative = if direct.iter().flatten().all(|value| value.is_finite()) {
        direct
    } else {
        let global_scale = points
            .iter()
            .flat_map(|point| [point.x().abs(), point.y().abs(), point.z().abs()])
            .fold(0.0, Real::max);
        if global_scale == 0.0 {
            return Ok(None);
        }
        let scaled_origin = [
            origin.x() / global_scale,
            origin.y() / global_scale,
            origin.z() / global_scale,
        ];
        points
            .iter()
            .map(|point| {
                [
                    point.x() / global_scale - scaled_origin[0],
                    point.y() / global_scale - scaled_origin[1],
                    point.z() / global_scale - scaled_origin[2],
                ]
            })
            .collect()
    };
    let scale = relative
        .iter()
        .flatten()
        .map(|value| value.abs())
        .fold(0.0, Real::max);
    if scale == 0.0 {
        return Ok(None);
    }
    for point in &mut relative {
        for coordinate in point {
            *coordinate /= scale;
        }
    }

    let mut normal_sum = [0.0; 3];
    let mut normal_correction = [0.0; 3];
    for index in 0..relative.len() {
        let first = relative[index];
        let second = relative[(index + 1) % relative.len()];
        let cross = [
            first[1].mul_add(second[2], -first[2] * second[1]),
            first[2].mul_add(second[0], -first[0] * second[2]),
            first[0].mul_add(second[1], -first[1] * second[0]),
        ];
        for coordinate in 0..3 {
            compensated_mesh_hole_sum(
                &mut normal_sum[coordinate],
                &mut normal_correction[coordinate],
                cross[coordinate],
            );
        }
    }
    let normal: [Real; 3] =
        std::array::from_fn(|coordinate| normal_sum[coordinate] + normal_correction[coordinate]);
    let normal_scale = normal.iter().map(|value| value.abs()).fold(0.0, Real::max);
    if normal_scale <= 64.0 * Real::EPSILON * points.len() as Real {
        return Ok(None);
    }
    let scaled_normal = normal.map(|value| value / normal_scale);
    let normal_length = scaled_normal
        .iter()
        .map(|value| value * value)
        .sum::<Real>()
        .sqrt();
    let normal = scaled_normal.map(|value| value / normal_length);

    let mut tangent = None;
    for index in 0..relative.len() {
        let first = relative[index];
        let second = relative[(index + 1) % relative.len()];
        let edge =
            std::array::from_fn::<_, 3, _>(|coordinate| second[coordinate] - first[coordinate]);
        let normal_component =
            edge[0].mul_add(normal[0], edge[1].mul_add(normal[1], edge[2] * normal[2]));
        let planar = std::array::from_fn::<_, 3, _>(|coordinate| {
            normal_component.mul_add(-normal[coordinate], edge[coordinate])
        });
        let squared_length = planar.iter().map(|value| value * value).sum::<Real>();
        if tangent.is_none_or(|(_, best_length)| squared_length > best_length) {
            tangent = Some((planar, squared_length));
        }
    }
    let Some((tangent, tangent_squared_length)) = tangent else {
        return Ok(None);
    };
    if tangent_squared_length <= Real::MIN_POSITIVE {
        return Ok(None);
    }
    let tangent_length = tangent_squared_length.sqrt();
    let x_axis = tangent.map(|value| value / tangent_length);
    let y_axis = [
        normal[1].mul_add(x_axis[2], -normal[2] * x_axis[1]),
        normal[2].mul_add(x_axis[0], -normal[0] * x_axis[2]),
        normal[0].mul_add(x_axis[1], -normal[1] * x_axis[0]),
    ];
    let projected = relative
        .into_iter()
        .map(|point| {
            [
                point[0].mul_add(x_axis[0], point[1].mul_add(x_axis[1], point[2] * x_axis[2])),
                point[0].mul_add(y_axis[0], point[1].mul_add(y_axis[1], point[2] * y_axis[2])),
            ]
        })
        .collect::<Vec<_>>();
    require_finite(projected.iter().flatten().copied(), "mesh hole projection")?;
    Ok(Some(projected))
}

fn projected_polygon_doubled_area(points: &[[Real; 2]]) -> Real {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        compensated_mesh_hole_sum(
            &mut sum,
            &mut correction,
            first[0].mul_add(second[1], -first[1] * second[0]),
        );
    }
    sum + correction
}

fn mesh_hole_cross(first: [Real; 2], second: [Real; 2], third: [Real; 2]) -> Real {
    let first_edge = [second[0] - first[0], second[1] - first[1]];
    let second_edge = [third[0] - first[0], third[1] - first[1]];
    first_edge[0].mul_add(second_edge[1], -first_edge[1] * second_edge[0])
}

fn point_in_mesh_hole_polygon(point: [Real; 2], polygon: &[[Real; 2]], epsilon: Real) -> bool {
    let mut winding = 0_i64;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let cross = mesh_hole_cross(start, end, point);
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

fn compensated_mesh_hole_sum(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

fn validate_mesh_edge_filter(filter: MeshEdgeFilter) -> Result<(), GeometryError> {
    let MeshEdgeFilter::FaceAngle {
        greater_than_radians,
        less_than_radians,
    } = filter
    else {
        return Ok(());
    };
    if greater_than_radians.is_finite()
        && less_than_radians.is_finite()
        && greater_than_radians >= 0.0
        && greater_than_radians < less_than_radians
        && less_than_radians <= std::f64::consts::PI
    {
        Ok(())
    } else {
        Err(GeometryError::InvalidMeshFaceAngleInterval)
    }
}

fn closest_point_on_triangle(
    target: Point3,
    a: Point3,
    b: Point3,
    c: Point3,
) -> Result<Point3, GeometryError> {
    let ab_vector = a.vector_to(b)?;
    let ac_vector = a.vector_to(c)?;
    let ap_vector = a.vector_to(target)?;
    let bp_vector = b.vector_to(target)?;
    let cp_vector = c.vector_to(target)?;
    let mut products = [
        ab_vector.dot(ap_vector)?,
        ac_vector.dot(ap_vector)?,
        ab_vector.dot(bp_vector)?,
        ac_vector.dot(bp_vector)?,
        ab_vector.dot(cp_vector)?,
        ac_vector.dot(cp_vector)?,
    ];
    let product_scale = products
        .iter()
        .fold(0.0_f64, |current, value| current.max(value.abs()));
    debug_assert!(
        product_scale > 0.0,
        "a validated triangle has nonzero dot products"
    );
    for product in &mut products {
        *product /= product_scale;
    }
    let [d1, d2, d3, d4, d5, d6] = products;
    let ab = ab_vector.to_array();
    let ac = ac_vector.to_array();
    if d1 <= 0.0 && d2 <= 0.0 {
        return Ok(a);
    }

    if d3 >= 0.0 && d4 <= d3 {
        return Ok(b);
    }

    let vc = d1.mul_add(d4, -d3 * d2);
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return triangle_barycentric_point(a, ab, ac, d1 / (d1 - d3), 0.0);
    }

    if d6 >= 0.0 && d5 <= d6 {
        return Ok(c);
    }

    let vb = d5.mul_add(d2, -d1 * d6);
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return triangle_barycentric_point(a, ab, ac, 0.0, d2 / (d2 - d6));
    }

    let va = d3.mul_add(d6, -d5 * d4);
    let d43 = d4 - d3;
    let d56 = d5 - d6;
    if va <= 0.0 && d43 >= 0.0 && d56 >= 0.0 {
        let edge_parameter = d43 / (d43 + d56);
        return triangle_barycentric_point(a, ab, ac, 1.0 - edge_parameter, edge_parameter);
    }

    let inverse_sum = 1.0 / (va + vb + vc);
    triangle_barycentric_point(a, ab, ac, vb * inverse_sum, vc * inverse_sum)
}

fn triangle_barycentric_point(
    origin: Point3,
    first: [Real; 3],
    second: [Real; 3],
    first_weight: Real,
    second_weight: Real,
) -> Result<Point3, GeometryError> {
    Point3::try_new(
        second[0].mul_add(second_weight, first[0].mul_add(first_weight, origin.x())),
        second[1].mul_add(second_weight, first[1].mul_add(first_weight, origin.y())),
        second[2].mul_add(second_weight, first[2].mul_add(first_weight, origin.z())),
    )
}

fn validate_mesh_break_angle(break_angle_radians: Real) -> Result<(), GeometryError> {
    if break_angle_radians.is_finite()
        && (0.0..=std::f64::consts::PI).contains(&break_angle_radians)
    {
        Ok(())
    } else {
        Err(GeometryError::InvalidMeshBreakAngle)
    }
}

fn mesh_edge_is_logical_boundary(
    incidence: &EdgeIncidence,
    face_normals: &[UnitVector3],
    break_angle_radians: Real,
) -> bool {
    let uses = incidence.uses().collect::<Vec<_>>();
    if uses.len() == 1 || edge_uses_are_unwelded(&uses) {
        return true;
    }
    (0..uses.len() - 1).any(|left| {
        (left + 1..uses.len()).any(|right| {
            unit_vector_angle(
                face_normals[uses[left].face],
                face_normals[uses[right].face],
            ) >= break_angle_radians
        })
    })
}

fn unit_vector_angle(first: UnitVector3, second: UnitVector3) -> Real {
    first
        .as_vector()
        .dot(second.as_vector())
        .expect("finite unit-vector dot products cannot fail")
        .clamp(-1.0, 1.0)
        .acos()
}

fn mesh_edge_matches_filter(
    incidence: &EdgeIncidence,
    filter: MeshEdgeFilter,
    face_normals: Option<&[UnitVector3]>,
) -> bool {
    match filter {
        MeshEdgeFilter::Naked => incidence.count == 1,
        MeshEdgeFilter::Unwelded => {
            incidence.count == 1 || edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
        }
        MeshEdgeFilter::FaceAngle {
            greater_than_radians,
            less_than_radians,
        } => {
            let uses = incidence.uses().collect::<Vec<_>>();
            if uses.len() < 2 {
                return false;
            }
            let normals = face_normals.expect("face-angle filtering computes polygon normals");
            let mut greatest_angle: Real = 0.0;
            for left in 0..uses.len() - 1 {
                for right in left + 1..uses.len() {
                    greatest_angle = greatest_angle.max(unit_vector_angle(
                        normals[uses[left].face],
                        normals[uses[right].face],
                    ));
                }
            }
            greatest_angle > greater_than_radians && greatest_angle < less_than_radians
        }
    }
}

#[derive(Clone, Copy)]
struct AugmentedTopologyEdge {
    vertices: [usize; 2],
    virtual_edge: bool,
}

fn topology_edge_polylines(
    points: &[Point3],
    edges: &[[usize; 2]],
    tolerance: Tolerance,
) -> Result<Vec<Polyline3>, GeometryError> {
    let components = topology_edge_components(points.len(), edges);
    let mut polylines = Vec::new();
    for component in components {
        let mut degrees = vec![0_usize; points.len()];
        let mut augmented = component
            .iter()
            .map(|&edge| {
                let vertices = edges[edge];
                degrees[vertices[0]] += 1;
                degrees[vertices[1]] += 1;
                AugmentedTopologyEdge {
                    vertices,
                    virtual_edge: false,
                }
            })
            .collect::<Vec<_>>();
        let odd_vertices = degrees
            .iter()
            .enumerate()
            .filter_map(|(vertex, &degree)| (degree % 2 == 1).then_some(vertex))
            .collect::<Vec<_>>();
        debug_assert_eq!(odd_vertices.len() % 2, 0);
        for pair in odd_vertices.chunks_exact(2) {
            augmented.push(AugmentedTopologyEdge {
                vertices: [pair[0], pair[1]],
                virtual_edge: true,
            });
        }

        let start = odd_vertices
            .first()
            .copied()
            .unwrap_or(augmented[0].vertices[0]);
        let (walk_vertices, walk_edges) = topology_euler_circuit(points.len(), &augmented, start);
        let virtual_positions = walk_edges
            .iter()
            .enumerate()
            .filter_map(|(position, &edge)| augmented[edge].virtual_edge.then_some(position))
            .collect::<Vec<_>>();
        if virtual_positions.is_empty() {
            polylines.push(Polyline3::try_new(
                walk_vertices
                    .into_iter()
                    .map(|vertex| points[vertex])
                    .collect(),
                tolerance,
            )?);
            continue;
        }

        let edge_count = walk_edges.len();
        for (position, &virtual_position) in virtual_positions.iter().enumerate() {
            let stop = virtual_positions[(position + 1) % virtual_positions.len()];
            let mut edge_position = (virtual_position + 1) % edge_count;
            let mut trail = vec![points[walk_vertices[edge_position]]];
            while edge_position != stop {
                debug_assert!(!augmented[walk_edges[edge_position]].virtual_edge);
                trail.push(points[walk_vertices[(edge_position + 1) % edge_count]]);
                edge_position = (edge_position + 1) % edge_count;
            }
            if trail.len() >= 2 {
                polylines.push(Polyline3::try_new(trail, tolerance)?);
            }
        }
    }
    Ok(polylines)
}

fn topology_edge_components(vertex_count: usize, edges: &[[usize; 2]]) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for (edge, vertices) in edges.iter().copied().enumerate() {
        adjacency[vertices[0]].push(edge);
        adjacency[vertices[1]].push(edge);
    }
    let mut visited = vec![false; edges.len()];
    let mut components = Vec::new();
    for first in 0..edges.len() {
        if visited[first] {
            continue;
        }
        visited[first] = true;
        let mut pending = vec![first];
        let mut component = Vec::new();
        while let Some(edge) = pending.pop() {
            component.push(edge);
            for vertex in edges[edge] {
                for &neighbor in &adjacency[vertex] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        pending.push(neighbor);
                    }
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components
}

fn topology_edge_vertex_order(
    vertex_count: usize,
    edges: &[([usize; 2], &EdgeIncidence)],
    selected: &[bool],
) -> Vec<usize> {
    let selected_edges = edges
        .iter()
        .zip(selected)
        .filter_map(|((vertices, _), &selected)| selected.then_some(*vertices))
        .collect::<Vec<_>>();
    let mut order = Vec::new();
    let mut seen = vec![false; vertex_count];
    for component in topology_edge_components(vertex_count, &selected_edges) {
        let mut degrees = vec![0_usize; vertex_count];
        let mut augmented = component
            .iter()
            .map(|&edge| {
                let vertices = selected_edges[edge];
                degrees[vertices[0]] += 1;
                degrees[vertices[1]] += 1;
                AugmentedTopologyEdge {
                    vertices,
                    virtual_edge: false,
                }
            })
            .collect::<Vec<_>>();
        let odd_vertices = degrees
            .iter()
            .enumerate()
            .filter_map(|(vertex, &degree)| (degree % 2 == 1).then_some(vertex))
            .collect::<Vec<_>>();
        for pair in odd_vertices.chunks_exact(2) {
            augmented.push(AugmentedTopologyEdge {
                vertices: [pair[0], pair[1]],
                virtual_edge: true,
            });
        }
        let start = odd_vertices
            .first()
            .copied()
            .unwrap_or(augmented[0].vertices[0]);
        let (walk, _) = topology_euler_circuit(vertex_count, &augmented, start);
        for vertex in walk {
            if !seen[vertex] {
                seen[vertex] = true;
                order.push(vertex);
            }
        }
    }
    order.extend(
        seen.into_iter()
            .enumerate()
            .filter_map(|(vertex, seen)| (!seen).then_some(vertex)),
    );
    order
}

fn topology_euler_circuit(
    vertex_count: usize,
    edges: &[AugmentedTopologyEdge],
    start: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for (edge, topology_edge) in edges.iter().enumerate() {
        adjacency[topology_edge.vertices[0]].push(edge);
        adjacency[topology_edge.vertices[1]].push(edge);
    }
    let mut next_adjacency = vec![0_usize; vertex_count];
    let mut used = vec![false; edges.len()];
    let mut vertex_stack = vec![start];
    let mut edge_stack = Vec::new();
    let mut reversed_vertices = Vec::with_capacity(edges.len() + 1);
    let mut reversed_edges = Vec::with_capacity(edges.len());
    while let Some(&vertex) = vertex_stack.last() {
        while next_adjacency[vertex] < adjacency[vertex].len()
            && used[adjacency[vertex][next_adjacency[vertex]]]
        {
            next_adjacency[vertex] += 1;
        }
        if let Some(&edge) = adjacency[vertex].get(next_adjacency[vertex]) {
            used[edge] = true;
            next_adjacency[vertex] += 1;
            let [first, second] = edges[edge].vertices;
            vertex_stack.push(if vertex == first { second } else { first });
            edge_stack.push(edge);
        } else {
            reversed_vertices.push(
                vertex_stack
                    .pop()
                    .expect("an Euler traversal has a current vertex"),
            );
            if let Some(edge) = edge_stack.pop() {
                reversed_edges.push(edge);
            }
        }
    }
    debug_assert!(used.into_iter().all(|edge| edge));
    reversed_vertices.reverse();
    reversed_edges.reverse();
    (reversed_vertices, reversed_edges)
}

fn mesh_grid_sample(interval: [Real; 2], step: Real, count: usize, index: usize) -> Real {
    debug_assert!(index <= count);
    if index == 0 {
        interval[0]
    } else if index == count {
        interval[1]
    } else {
        interval[0] + index as Real * step
    }
}

fn mesh_radial_cap_face_count(around_count: usize, style: MeshCapFaceStyle) -> usize {
    if style == MeshCapFaceStyle::Quadrilaterals && around_count.is_multiple_of(2) {
        if around_count == 4 {
            1
        } else {
            around_count / 2
        }
    } else {
        around_count
    }
}

fn append_mesh_radial_cap(
    vertices: &mut Vec<Point3>,
    faces: &mut Vec<MeshFace>,
    frame: Frame3,
    radial_coordinates: &[[Real; 2]],
    height: Real,
    style: MeshCapFaceStyle,
) -> Result<(), GeometryError> {
    let around_count = radial_coordinates.len();
    let offset = vertices.len();
    if style == MeshCapFaceStyle::Quadrilaterals && around_count == 4 {
        for [x, y] in radial_coordinates {
            vertices.push(mesh_frame_point(frame, *x, *y, height)?);
        }
        faces.push(MeshFace::Quad([
            u32::try_from(offset).map_err(|_| GeometryError::TooManyMeshVertices)?,
            u32::try_from(offset + 1).map_err(|_| GeometryError::TooManyMeshVertices)?,
            u32::try_from(offset + 2).map_err(|_| GeometryError::TooManyMeshVertices)?,
            u32::try_from(offset + 3).map_err(|_| GeometryError::TooManyMeshVertices)?,
        ]));
        return Ok(());
    }

    vertices.push(mesh_frame_point(frame, 0.0, 0.0, height)?);
    for [x, y] in radial_coordinates {
        vertices.push(mesh_frame_point(frame, *x, *y, height)?);
    }
    let center = u32::try_from(offset).map_err(|_| GeometryError::TooManyMeshVertices)?;
    if style == MeshCapFaceStyle::Quadrilaterals && around_count.is_multiple_of(2) {
        for face_index in 0..around_count / 2 {
            let first = offset + 1 + 2 * face_index;
            let second = first + 1;
            let third = offset + 1 + (2 * face_index + 2) % around_count;
            faces.push(MeshFace::Quad([
                center,
                u32::try_from(first).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(second).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(third).map_err(|_| GeometryError::TooManyMeshVertices)?,
            ]));
        }
    } else {
        for face_index in 0..around_count {
            faces.push(MeshFace::Triangle([
                center,
                u32::try_from(offset + 1 + face_index)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(offset + 1 + (face_index + 1) % around_count)
                    .map_err(|_| GeometryError::TooManyMeshVertices)?,
            ]));
        }
    }
    Ok(())
}

fn mesh_frame_point(frame: Frame3, x: Real, y: Real, z: Real) -> Result<Point3, GeometryError> {
    frame
        .origin()
        .translated(frame.x_axis().as_vector().scaled(x)?)?
        .translated(frame.y_axis().as_vector().scaled(y)?)?
        .translated(frame.z_axis().as_vector().scaled(z)?)
}

fn validate_subdivision_sphere_options(
    radius: Real,
    subdivisions: usize,
    maximum: usize,
    base_face_count: usize,
) -> Result<(), GeometryError> {
    require_finite([radius], "mesh-sphere radius")?;
    if radius <= 0.0 {
        return Err(GeometryError::InvalidMeshSphereRadius);
    }
    if subdivisions > maximum {
        return Err(GeometryError::InvalidMeshSphereSubdivisionCount {
            subdivisions,
            maximum,
        });
    }
    let face_count = (0..subdivisions).try_fold(base_face_count, |count, _| {
        count.checked_mul(4).ok_or(GeometryError::TooManyMeshFaces)
    })?;
    if face_count > MAX_MESH_SPHERE_FACES {
        return Err(GeometryError::TooManyMeshFaces);
    }
    Ok(())
}

fn mesh_index_edge(first: u32, second: u32) -> (u32, u32) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn project_sphere_coordinate(
    coordinate: [Real; 3],
    radius: Real,
) -> Result<[Real; 3], GeometryError> {
    require_finite(coordinate, "mesh-sphere projection")?;
    let length = coordinate[0].hypot(coordinate[1]).hypot(coordinate[2]);
    if length == 0.0 {
        return Err(GeometryError::Degenerate {
            context: "mesh-sphere projection",
        });
    }
    let scale = radius / length;
    let projected = coordinate.map(|component| component * scale);
    require_finite(projected, "mesh-sphere projection")?;
    Ok(projected)
}

fn average_sphere_coordinates(
    first: [Real; 3],
    second: [Real; 3],
    radius: Real,
) -> Result<[Real; 3], GeometryError> {
    project_sphere_coordinate(
        [
            0.5 * first[0] + 0.5 * second[0],
            0.5 * first[1] + 0.5 * second[1],
            0.5 * first[2] + 0.5 * second[2],
        ],
        radius,
    )
}

fn append_icosphere_midpoint(
    coordinates: &mut Vec<[Real; 3]>,
    edge_vertices: &mut BTreeMap<(u32, u32), u32>,
    first: u32,
    second: u32,
    radius: Real,
) -> Result<u32, GeometryError> {
    let edge = mesh_index_edge(first, second);
    if let Some(index) = edge_vertices.get(&edge) {
        return Ok(*index);
    }
    let midpoint = average_sphere_coordinates(
        coordinates[first as usize],
        coordinates[second as usize],
        radius,
    )?;
    let index = u32::try_from(coordinates.len()).map_err(|_| GeometryError::TooManyMeshVertices)?;
    coordinates
        .try_reserve(1)
        .map_err(|_| GeometryError::TooManyMeshVertices)?;
    coordinates.push(midpoint);
    edge_vertices.insert(edge, index);
    Ok(index)
}

fn append_mesh_grid_side(
    vertices: &mut Vec<Point3>,
    faces: &mut Vec<MeshFace>,
    u_count: usize,
    v_count: usize,
    mut point: impl FnMut(usize, usize) -> Result<Point3, GeometryError>,
) -> Result<(), GeometryError> {
    let offset = vertices.len();
    for v in 0..=v_count {
        for u in 0..=u_count {
            vertices.push(point(u, v)?);
        }
    }
    let width = u_count + 1;
    for v in 0..v_count {
        for u in 0..u_count {
            let lower_left = offset + v * width + u;
            let upper_left = lower_left + width;
            faces.push(MeshFace::Quad([
                u32::try_from(lower_left).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(lower_left + 1).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(upper_left + 1).map_err(|_| GeometryError::TooManyMeshVertices)?,
                u32::try_from(upper_left).map_err(|_| GeometryError::TooManyMeshVertices)?,
            ]));
        }
    }
    Ok(())
}

fn validate_triangle(
    vertices: &[Point3],
    triangle: [u32; 3],
    face_index: usize,
    from_quad: bool,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let point_at = |vertex_index| {
        vertices.get(vertex_index as usize).copied().ok_or({
            if from_quad {
                GeometryError::InvalidQuadIndex {
                    face: face_index,
                    vertex: vertex_index,
                }
            } else {
                GeometryError::InvalidTriangleIndex {
                    triangle: face_index,
                    vertex: vertex_index,
                }
            }
        })
    };
    let points = [
        point_at(triangle[0])?,
        point_at(triangle[1])?,
        point_at(triangle[2])?,
    ];
    let degenerate = || {
        if from_quad {
            GeometryError::DegenerateQuad { face: face_index }
        } else {
            GeometryError::DegenerateTriangle {
                triangle: face_index,
            }
        }
    };
    let first_edge = points[0]
        .vector_to(points[1])?
        .normalized(tolerance)
        .map_err(|_| degenerate())?;
    let second_edge = points[0]
        .vector_to(points[2])?
        .normalized(tolerance)
        .map_err(|_| degenerate())?;
    let sine = first_edge
        .as_vector()
        .cross(second_edge.as_vector())?
        .length()?;
    if sine <= tolerance.angular() {
        return Err(degenerate());
    }
    Ok(())
}

fn trace_boundary_path(
    start: usize,
    first_edge: usize,
    edges: &[[usize; 2]],
    adjacency: &[Vec<usize>],
    used: &mut [bool],
) -> Vec<usize> {
    let mut path = vec![start];
    let mut current = start;
    let mut edge = first_edge;
    loop {
        debug_assert!(!used[edge]);
        used[edge] = true;
        let [first, second] = edges[edge];
        let next = if current == first {
            second
        } else {
            debug_assert_eq!(current, second);
            first
        };
        path.push(next);
        if next == start || adjacency[next].len() != 2 {
            break;
        }
        let Some(next_edge) = adjacency[next]
            .iter()
            .copied()
            .find(|candidate| !used[*candidate])
        else {
            break;
        };
        current = next;
        edge = next_edge;
    }
    path
}

fn canonical_coordinate_bits(coordinate: Real) -> u64 {
    if coordinate == 0.0 {
        0
    } else {
        coordinate.to_bits()
    }
}

fn compare_points_descending(left: &Point3, right: &Point3) -> std::cmp::Ordering {
    left.to_array()
        .into_iter()
        .zip(right.to_array())
        .map(|(left, right)| {
            right
                .partial_cmp(&left)
                .expect("validated point coordinates are finite")
        })
        .find(|ordering| !ordering.is_eq())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn edge_uses_are_unwelded(uses: &[EdgeUse]) -> bool {
    let mut first_endpoint_indices = BTreeSet::new();
    let mut second_endpoint_indices = BTreeSet::new();
    for edge_use in uses {
        if !first_endpoint_indices.insert(edge_use.raw_vertices[0])
            || !second_endpoint_indices.insert(edge_use.raw_vertices[1])
        {
            return false;
        }
    }
    true
}

fn topology_face_edge_indices(mesh: &TriangleMesh, data: &MeshTopologyData) -> Vec<Vec<usize>> {
    let edge_indices = data
        .edges
        .keys()
        .copied()
        .enumerate()
        .map(|(index, edge)| (edge, index))
        .collect::<BTreeMap<_, _>>();
    mesh.faces
        .iter()
        .map(|face| {
            let raw_vertices = face.indices();
            (0..raw_vertices.len())
                .map(|side| {
                    let first = data.topological_vertices[raw_vertices[side] as usize];
                    let second = data.topological_vertices
                        [raw_vertices[(side + 1) % raw_vertices.len()] as usize];
                    edge_indices[&(first.min(second), first.max(second))]
                })
                .collect()
        })
        .collect()
}

/// Reproduces `ON_MeshTopology::SortVertexEdges`: each returned group is one
/// radial fan, starting at a naked/non-manifold edge when one is present.
fn radially_sorted_vertex_edges(
    topological_vertex: usize,
    incident_edges: &[usize],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_edges: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut naked = Vec::new();
    let mut manifold = Vec::new();
    let mut non_manifold = Vec::new();
    for &edge in incident_edges {
        let (vertices, incidence) = edges[edge];
        debug_assert!(vertices.contains(&topological_vertex));
        match incidence.count {
            1 => naked.push(edge),
            2 => manifold.push(edge),
            _ => non_manifold.push(edge),
        }
    }
    naked.extend(non_manifold);

    let mut groups = Vec::new();
    while !naked.is_empty() || !manifold.is_empty() {
        let first = if naked.is_empty() {
            manifold.remove(0)
        } else {
            naked.remove(0)
        };
        let mut group = vec![first];
        let mut current = first;
        let mut group_direction = 0_i8;
        loop {
            let (vertices, incidence) = edges[current];
            let mut next = None;
            for edge_use in incidence.uses() {
                let direction = if vertices[0] == topological_vertex {
                    if edge_use.forward { -1 } else { 1 }
                } else if edge_use.forward {
                    1
                } else {
                    -1
                };
                let side_count = face_edges[edge_use.face].len();
                let next_side = if direction < 0 {
                    (edge_use.side + side_count - 1) % side_count
                } else {
                    (edge_use.side + 1) % side_count
                };
                let candidate = face_edges[edge_use.face][next_side];
                let removed = if let Some(index) = naked.iter().position(|&edge| edge == candidate)
                {
                    naked.remove(index);
                    true
                } else if let Some(index) = manifold.iter().position(|&edge| edge == candidate) {
                    manifold.remove(index);
                    true
                } else {
                    false
                };
                if removed {
                    if group_direction == 0 {
                        group_direction = direction;
                    }
                    next = Some(candidate);
                    break;
                }
            }
            let Some(next_edge) = next else {
                break;
            };
            group.push(next_edge);
            current = next_edge;
        }
        if group_direction > 0 {
            group.reverse();
        }
        groups.push(group);
    }
    groups
}

fn shared_edge_face(first: &EdgeIncidence, second: &EdgeIncidence) -> Option<usize> {
    first.uses().find_map(|first_use| {
        second
            .uses()
            .any(|second_use| second_use.face == first_use.face)
            .then_some(first_use.face)
    })
}

fn ordered_vertex_face_components(
    edge_groups: &[Vec<usize>],
    edges: &[([usize; 2], &EdgeIncidence)],
    face_to_local: &BTreeMap<usize, usize>,
    incident_faces: &[usize],
    parents: &mut [usize],
) -> Vec<usize> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    for edge_group in edge_groups {
        let mut radial_roots = Vec::new();
        let mut add_shared_root = |first: usize, second: usize| {
            let Some(face) = shared_edge_face(edges[first].1, edges[second].1) else {
                return;
            };
            let Some(&local) = face_to_local.get(&face) else {
                return;
            };
            let root = face_root(parents, local);
            if radial_roots.last().copied() != Some(root) {
                radial_roots.push(root);
            }
        };
        for pair in edge_group.windows(2) {
            add_shared_root(pair[0], pair[1]);
        }
        if edge_group.len() > 1 {
            add_shared_root(*edge_group.last().unwrap(), edge_group[0]);
        }
        if radial_roots.len() > 1 && radial_roots.first() == radial_roots.last() {
            radial_roots.remove(0);
        }
        for root in radial_roots {
            if seen.insert(root) {
                order.push(root);
            }
        }
    }
    for &face in incident_faces {
        let root = face_root(parents, face_to_local[&face]);
        if seen.insert(root) {
            order.push(root);
        }
    }
    order
}

fn face_root(parents: &mut [usize], face: usize) -> usize {
    let mut root = face;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = face;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_faces(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = face_root(parents, first);
    let second_root = face_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        std::cmp::Ordering::Less => parents[first_root] = second_root,
        std::cmp::Ordering::Greater => parents[second_root] = first_root,
        std::cmp::Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] += 1;
        }
    }
}

fn index_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_indices_keep_later(parents: &mut [usize], first: usize, second: usize) {
    let first = index_root(parents, first);
    let second = index_root(parents, second);
    if first < second {
        parents[first] = second;
    } else if second < first {
        parents[second] = first;
    }
}

fn union_indices_keep_earlier(parents: &mut [usize], first: usize, second: usize) {
    let first = index_root(parents, first);
    let second = index_root(parents, second);
    if first < second {
        parents[second] = first;
    } else if second < first {
        parents[first] = second;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn topology_edge_is_unwelded_between(
        mesh: &TriangleMesh,
        first: Point3,
        second: Point3,
    ) -> bool {
        let data = mesh.topology_data();
        let (_, incidence) = data
            .edges
            .iter()
            .find(|&(&(a, b), _)| {
                (data.topological_points[a] == first && data.topological_points[b] == second)
                    || (data.topological_points[a] == second && data.topological_points[b] == first)
            })
            .expect("test topology edge exists");
        incidence.count > 1 && edge_uses_are_unwelded(&incidence.uses().collect::<Vec<_>>())
    }

    fn topology_edge_index_between(mesh: &TriangleMesh, first: Point3, second: Point3) -> usize {
        let data = mesh.topology_data();
        data.edges
            .keys()
            .position(|&(a, b)| {
                (data.topological_points[a] == first && data.topological_points[b] == second)
                    || (data.topological_points[a] == second && data.topological_points[b] == first)
            })
            .expect("test topology edge exists")
    }

    #[test]
    fn validates_indices_degeneracy_and_orientation() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ];
        let mesh =
            TriangleMesh::try_new(vertices.clone(), vec![[0, 1, 2]], Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.face_normal(0).unwrap().z(), 1.0);
        assert_eq!(mesh.bounds().max(), point(1.0, 1.0, 0.0));
        assert!(matches!(
            mesh.face_normal(1),
            Err(GeometryError::TriangleIndexOutOfRange { triangle: 1 })
        ));

        assert!(
            TriangleMesh::try_new(vertices.clone(), vec![[0, 1, 3]], Tolerance::DEFAULT).is_err()
        );
        assert!(TriangleMesh::try_new(vertices, vec![[0, 1, 1]], Tolerance::DEFAULT).is_err());
        assert!(matches!(
            TriangleMesh::try_new(Vec::new(), vec![[0, 1, 2]], Tolerance::DEFAULT),
            Err(GeometryError::InvalidTriangleIndex {
                triangle: 0,
                vertex: 0
            })
        ));

        let square = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(square.area().unwrap(), 1.0);
    }

    #[test]
    fn preserves_quadrilateral_faces_without_topology_diagonals() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
        ];
        let mesh = TriangleMesh::try_new_faces(
            vertices.clone(),
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(mesh.faces(), &[MeshFace::Quad([0, 1, 2, 3])]);
        assert_eq!(mesh.triangles(), &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(mesh.topology().edge_count(), 4);
        assert_eq!(mesh.wireframe_lines(Tolerance::DEFAULT).unwrap().len(), 4);
        assert_eq!(mesh.area().unwrap(), 6.0);
        assert_eq!(mesh.reversed().faces(), &[MeshFace::Quad([0, 3, 2, 1])]);

        assert!(matches!(
            TriangleMesh::try_new_faces(
                vertices.clone(),
                vec![MeshFace::Quad([0, 1, 2, 4])],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidQuadIndex { face: 0, vertex: 4 })
        ));
        assert!(matches!(
            TriangleMesh::try_new_faces(
                vertices,
                vec![MeshFace::Quad([0, 1, 2, 1])],
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::DegenerateQuad { face: 0 })
        ));
    }

    #[test]
    fn creates_rhino_ordered_mesh_plane_grids_and_rejects_invalid_extents() {
        let frame = Frame3::try_from_directions(
            point(1.0, -2.0, 5.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mesh =
            TriangleMesh::try_plane_grid(frame, [-2.0, 4.0], [1.0, 10.0], 2, 3, Tolerance::DEFAULT)
                .unwrap();
        assert_eq!(mesh.vertices().len(), 12);
        assert_eq!(mesh.face_count(), 6);
        assert_eq!(mesh.vertices()[0], point(-1.0, -1.0, 5.0));
        assert_eq!(mesh.vertices()[2], point(5.0, -1.0, 5.0));
        assert_eq!(mesh.vertices()[9], point(-1.0, 8.0, 5.0));
        assert_eq!(mesh.vertices()[11], point(5.0, 8.0, 5.0));
        assert_eq!(
            mesh.faces(),
            &[
                MeshFace::Quad([0, 1, 4, 3]),
                MeshFace::Quad([1, 2, 5, 4]),
                MeshFace::Quad([3, 4, 7, 6]),
                MeshFace::Quad([4, 5, 8, 7]),
                MeshFace::Quad([6, 7, 10, 9]),
                MeshFace::Quad([7, 8, 11, 10]),
            ]
        );
        assert_eq!(mesh.area().unwrap(), 54.0);
        assert_eq!(mesh.topology().boundary_edge_count(), 10);

        assert_eq!(
            TriangleMesh::try_plane_grid(frame, [-2.0, 4.0], [1.0, 10.0], 0, 3, Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshPlaneFaceCount {
                x_count: 0,
                y_count: 3,
            })
        );
        assert_eq!(
            TriangleMesh::try_plane_grid(frame, [4.0, -2.0], [1.0, 10.0], 2, 3, Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshPlaneInterval)
        );
        assert_eq!(
            TriangleMesh::try_plane_grid(
                frame,
                [-2.0, 4.0],
                [1.0, 10.0],
                MAX_MESH_PLANE_FACES + 1,
                1,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
        assert!(matches!(
            TriangleMesh::try_plane_grid(
                frame,
                [f64::NAN, 4.0],
                [1.0, 10.0],
                2,
                3,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::NonFinite {
                context: "mesh-plane interval"
            })
        ));
    }

    #[test]
    fn creates_rhino_ordered_unwelded_mesh_box_grids() {
        let world = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let unit_counts = TriangleMesh::try_box_grid(
            world,
            [[0.0, 4.0], [0.0, 3.0], [0.0, 2.0]],
            1,
            1,
            1,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unit_counts.vertices().len(), 24);
        assert_eq!(unit_counts.face_count(), 6);
        assert_eq!(
            unit_counts.faces(),
            &[
                MeshFace::Quad([0, 1, 3, 2]),
                MeshFace::Quad([4, 5, 7, 6]),
                MeshFace::Quad([8, 9, 11, 10]),
                MeshFace::Quad([12, 13, 15, 14]),
                MeshFace::Quad([16, 17, 19, 18]),
                MeshFace::Quad([20, 21, 23, 22]),
            ]
        );
        assert_eq!(unit_counts.vertices()[0], point(0.0, 3.0, 0.0));
        assert_eq!(unit_counts.vertices()[3], point(4.0, 0.0, 0.0));
        assert_eq!(unit_counts.vertices()[4], point(0.0, 0.0, 2.0));
        assert_eq!(unit_counts.vertices()[23], point(0.0, 0.0, 2.0));
        assert_eq!(
            unit_counts.face_normal(0).unwrap().as_vector().to_array(),
            [0.0, 0.0, -1.0]
        );
        assert_eq!(unit_counts.topology().topological_vertex_count(), 8);
        assert_eq!(unit_counts.topology().edge_count(), 12);
        assert!(unit_counts.topology().is_solid());
        assert_eq!(unit_counts.area().unwrap(), 52.0);
        assert!((unit_counts.signed_volume().unwrap() - 24.0).abs() < 1.0e-12);

        let frame = Frame3::try_from_directions(
            point(1.0, -2.0, 5.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let subdivided = TriangleMesh::try_box_grid(
            frame,
            [[-2.0, 4.0], [1.0, 10.0], [-1.0, 5.0]],
            2,
            3,
            2,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(subdivided.vertices().len(), 66);
        assert_eq!(subdivided.face_count(), 32);
        assert_eq!(subdivided.faces()[0], MeshFace::Quad([0, 1, 4, 3]));
        assert_eq!(subdivided.faces()[6], MeshFace::Quad([12, 13, 16, 15]));
        assert_eq!(subdivided.faces()[12], MeshFace::Quad([24, 25, 28, 27]));
        assert_eq!(subdivided.faces()[16], MeshFace::Quad([33, 34, 38, 37]));
        assert_eq!(subdivided.faces()[22], MeshFace::Quad([45, 46, 49, 48]));
        assert_eq!(subdivided.faces()[26], MeshFace::Quad([54, 55, 59, 58]));
        assert_eq!(subdivided.topology().topological_vertex_count(), 34);
        assert_eq!(subdivided.topology().edge_count(), 64);
        assert!(subdivided.topology().is_solid());
        assert!((subdivided.area().unwrap() - 288.0).abs() < 1.0e-12);
        assert!((subdivided.signed_volume().unwrap() - 324.0).abs() < 1.0e-10);
    }

    #[test]
    fn mesh_box_grid_rejects_invalid_counts_extents_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            TriangleMesh::try_box_grid(
                frame,
                [[0.0, 4.0], [0.0, 3.0], [0.0, 2.0]],
                1,
                0,
                1,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshBoxFaceCount {
                x_count: 1,
                y_count: 0,
                z_count: 1,
            })
        );
        assert_eq!(
            TriangleMesh::try_box_grid(
                frame,
                [[4.0, 0.0], [0.0, 3.0], [0.0, 2.0]],
                1,
                1,
                1,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshBoxInterval)
        );
        assert_eq!(
            TriangleMesh::try_box_grid(
                frame,
                [[0.0, 4.0], [0.0, 3.0], [0.0, 2.0]],
                MAX_MESH_BOX_FACES + 1,
                1,
                1,
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn creates_rhino_ordered_mesh_cylinder_walls_and_caps() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let open = TriangleMesh::try_cylinder_grid(
            frame,
            2.0,
            [0.0, 5.0],
            MeshCylinderOptions {
                vertical_count: 1,
                around_count: 4,
                cap_bottom: false,
                cap_top: false,
                circumscribe: false,
                cap_style: MeshCapFaceStyle::Triangles,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(open.vertices().len(), 8);
        assert_eq!(open.face_count(), 4);
        assert_eq!(
            open.faces(),
            &[
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ]
        );
        assert_eq!(open.vertices()[0], point(2.0, 0.0, 0.0));
        assert_eq!(open.vertices()[4], point(2.0, 0.0, 5.0));
        assert_eq!(open.topology().boundary_edge_count(), 8);

        let triangles = TriangleMesh::try_cylinder_grid(
            frame,
            2.0,
            [-1.0, 5.0],
            MeshCylinderOptions {
                vertical_count: 2,
                around_count: 5,
                cap_bottom: true,
                cap_top: true,
                circumscribe: false,
                cap_style: MeshCapFaceStyle::Triangles,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(triangles.vertices().len(), 27);
        assert_eq!(triangles.face_count(), 20);
        assert_eq!(triangles.faces()[10], MeshFace::Triangle([15, 16, 17]));
        assert_eq!(triangles.faces()[14], MeshFace::Triangle([15, 20, 16]));
        assert_eq!(triangles.faces()[15], MeshFace::Triangle([21, 22, 23]));
        assert_eq!(triangles.vertices()[15], point(0.0, 0.0, -1.0));
        assert_eq!(triangles.vertices()[21], point(0.0, 0.0, 5.0));
        assert!(triangles.topology().is_closed());
        assert_eq!(triangles.topology().orientation_conflict_edge_count(), 5);

        let quads = TriangleMesh::try_cylinder_grid(
            frame,
            3.0,
            [0.0, 4.0],
            MeshCylinderOptions {
                vertical_count: 3,
                around_count: 6,
                cap_bottom: true,
                cap_top: true,
                circumscribe: false,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(quads.vertices().len(), 38);
        assert_eq!(quads.face_count(), 24);
        assert_eq!(quads.faces()[18], MeshFace::Quad([24, 25, 26, 27]));
        assert_eq!(quads.faces()[19], MeshFace::Quad([24, 27, 28, 29]));
        assert_eq!(quads.faces()[20], MeshFace::Quad([24, 29, 30, 25]));
        assert_eq!(quads.faces()[21], MeshFace::Quad([31, 32, 33, 34]));

        let circumscribed = TriangleMesh::try_cylinder_grid(
            frame,
            2.0,
            [0.0, 5.0],
            MeshCylinderOptions {
                vertical_count: 2,
                around_count: 4,
                cap_bottom: true,
                cap_top: true,
                circumscribe: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(circumscribed.vertices().len(), 20);
        assert_eq!(circumscribed.face_count(), 10);
        assert!(
            (circumscribed.vertices()[0].x() - 2.0).abs() < 1.0e-12
                && (circumscribed.vertices()[0].y() - 2.0).abs() < 1.0e-12
        );
        assert_eq!(circumscribed.faces()[8], MeshFace::Quad([12, 13, 14, 15]));
        assert_eq!(circumscribed.faces()[9], MeshFace::Quad([16, 17, 18, 19]));
    }

    #[test]
    fn mesh_cylinder_rejects_invalid_counts_dimensions_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |vertical_count, around_count| MeshCylinderOptions {
            vertical_count,
            around_count,
            cap_bottom: true,
            cap_top: true,
            circumscribe: false,
            cap_style: MeshCapFaceStyle::Triangles,
        };
        assert_eq!(
            TriangleMesh::try_cylinder_grid(
                frame,
                2.0,
                [0.0, 5.0],
                options(0, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshCylinderFaceCount {
                vertical_count: 0,
                around_count: 4,
            })
        );
        assert!(matches!(
            TriangleMesh::try_cylinder_grid(
                frame,
                2.0,
                [0.0, 5.0],
                options(1, 2),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshCylinderFaceCount { .. })
        ));
        for (radius, heights) in [(0.0, [0.0, 5.0]), (2.0, [5.0, 5.0]), (2.0, [5.0, 0.0])] {
            assert_eq!(
                TriangleMesh::try_cylinder_grid(
                    frame,
                    radius,
                    heights,
                    options(1, 4),
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidMeshCylinderDimensions)
            );
        }
        assert!(matches!(
            TriangleMesh::try_cylinder_grid(
                frame,
                Real::NAN,
                [0.0, 5.0],
                options(1, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::NonFinite {
                context: "mesh-cylinder dimensions"
            })
        ));
        assert_eq!(
            TriangleMesh::try_cylinder_grid(
                frame,
                2.0,
                [0.0, 5.0],
                options(MAX_MESH_CYLINDER_FACES + 1, 3),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn creates_rhino_ordered_mesh_cone_rings_apex_and_caps() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 5.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let open = TriangleMesh::try_cone_grid(
            frame,
            2.0,
            -5.0,
            MeshConeOptions {
                vertical_count: 1,
                around_count: 4,
                solid: false,
                cap_style: MeshCapFaceStyle::Triangles,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(open.vertices().len(), 5);
        assert_eq!(open.face_count(), 4);
        assert_eq!(open.vertices()[0], point(0.0, 0.0, 5.0));
        assert_eq!(open.vertices()[1], point(2.0, 0.0, 0.0));
        assert_eq!(
            open.faces(),
            &[
                MeshFace::Triangle([0, 2, 1]),
                MeshFace::Triangle([0, 3, 2]),
                MeshFace::Triangle([0, 4, 3]),
                MeshFace::Triangle([0, 1, 4]),
            ]
        );
        assert_eq!(open.topology().boundary_edge_count(), 4);

        let triangles = TriangleMesh::try_cone_grid(
            frame,
            2.0,
            -5.0,
            MeshConeOptions {
                vertical_count: 2,
                around_count: 5,
                solid: true,
                cap_style: MeshCapFaceStyle::Triangles,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(triangles.vertices().len(), 17);
        assert_eq!(triangles.face_count(), 15);
        assert_eq!(triangles.faces()[0], MeshFace::Triangle([0, 2, 1]));
        assert_eq!(triangles.faces()[5], MeshFace::Quad([1, 2, 7, 6]));
        assert_eq!(triangles.faces()[10], MeshFace::Triangle([11, 12, 13]));
        assert_eq!(triangles.vertices()[11], point(0.0, 0.0, 0.0));
        assert!(triangles.topology().is_solid());
        assert!(triangles.signed_volume().unwrap() < 0.0);

        let quads = TriangleMesh::try_cone_grid(
            frame,
            3.0,
            -5.0,
            MeshConeOptions {
                vertical_count: 3,
                around_count: 6,
                solid: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(quads.vertices().len(), 26);
        assert_eq!(quads.face_count(), 21);
        assert_eq!(quads.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
        assert_eq!(quads.faces()[18], MeshFace::Quad([19, 20, 21, 22]));
        assert_eq!(quads.faces()[20], MeshFace::Quad([19, 24, 25, 20]));

        let four = TriangleMesh::try_cone_grid(
            frame,
            2.0,
            5.0,
            MeshConeOptions {
                vertical_count: 2,
                around_count: 4,
                solid: true,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(four.vertices().len(), 13);
        assert_eq!(four.face_count(), 9);
        assert_eq!(four.faces()[8], MeshFace::Quad([9, 10, 11, 12]));
        assert!(four.topology().is_solid());
        assert!(four.signed_volume().unwrap() > 0.0);
    }

    #[test]
    fn mesh_cone_rejects_invalid_counts_dimensions_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 5.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |vertical_count, around_count| MeshConeOptions {
            vertical_count,
            around_count,
            solid: true,
            cap_style: MeshCapFaceStyle::Triangles,
        };
        assert_eq!(
            TriangleMesh::try_cone_grid(frame, 2.0, -5.0, options(0, 4), Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshConeFaceCount {
                vertical_count: 0,
                around_count: 4,
            })
        );
        assert!(matches!(
            TriangleMesh::try_cone_grid(frame, 2.0, -5.0, options(1, 2), Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshConeFaceCount { .. })
        ));
        for (radius, height) in [(0.0, -5.0), (2.0, 0.0)] {
            assert_eq!(
                TriangleMesh::try_cone_grid(
                    frame,
                    radius,
                    height,
                    options(1, 4),
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidMeshConeDimensions)
            );
        }
        assert!(matches!(
            TriangleMesh::try_cone_grid(
                frame,
                2.0,
                Real::INFINITY,
                options(1, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::NonFinite {
                context: "mesh-cone dimensions"
            })
        ));
        assert_eq!(
            TriangleMesh::try_cone_grid(
                frame,
                2.0,
                -5.0,
                options(MAX_MESH_CONE_FACES + 1, 3),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn creates_rhino_ordered_uv_mesh_sphere_poles_rings_and_faces() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let minimal = TriangleMesh::try_uv_sphere_grid(
            frame,
            2.0,
            MeshUvSphereOptions {
                vertical_count: 2,
                around_count: 4,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(minimal.vertices().len(), 6);
        assert_eq!(minimal.face_count(), 8);
        assert_eq!(minimal.vertices()[0], point(0.0, 0.0, -2.0));
        assert_eq!(minimal.vertices()[1], point(2.0, 0.0, 0.0));
        assert_eq!(minimal.vertices()[5], point(0.0, 0.0, 2.0));
        assert_eq!(
            minimal.faces(),
            &[
                MeshFace::Triangle([0, 2, 1]),
                MeshFace::Triangle([0, 3, 2]),
                MeshFace::Triangle([0, 4, 3]),
                MeshFace::Triangle([0, 1, 4]),
                MeshFace::Triangle([1, 2, 5]),
                MeshFace::Triangle([2, 3, 5]),
                MeshFace::Triangle([3, 4, 5]),
                MeshFace::Triangle([4, 1, 5]),
            ]
        );
        assert!(minimal.topology().is_solid());
        assert!(minimal.signed_volume().unwrap() > 0.0);

        let gridded = TriangleMesh::try_uv_sphere_grid(
            frame,
            3.0,
            MeshUvSphereOptions {
                vertical_count: 4,
                around_count: 6,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(gridded.vertices().len(), 20);
        assert_eq!(gridded.face_count(), 24);
        assert_eq!(gridded.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
        assert_eq!(gridded.faces()[18], MeshFace::Triangle([13, 14, 19]));
        assert_eq!(gridded.faces()[23], MeshFace::Triangle([18, 13, 19]));
        assert!(gridded.topology().is_solid());
    }

    #[test]
    fn uv_mesh_sphere_rejects_invalid_counts_radius_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |vertical_count, around_count| MeshUvSphereOptions {
            vertical_count,
            around_count,
        };
        assert_eq!(
            TriangleMesh::try_uv_sphere_grid(frame, 2.0, options(1, 4), Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshSphereFaceCount {
                vertical_count: 1,
                around_count: 4,
            })
        );
        assert!(matches!(
            TriangleMesh::try_uv_sphere_grid(frame, 2.0, options(2, 2), Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshSphereFaceCount { .. })
        ));
        assert_eq!(
            TriangleMesh::try_uv_sphere_grid(frame, 0.0, options(2, 4), Tolerance::DEFAULT,),
            Err(GeometryError::InvalidMeshSphereRadius)
        );
        assert!(matches!(
            TriangleMesh::try_uv_sphere_grid(frame, Real::NAN, options(2, 4), Tolerance::DEFAULT,),
            Err(GeometryError::NonFinite {
                context: "mesh-sphere radius"
            })
        ));
        assert_eq!(
            TriangleMesh::try_uv_sphere_grid(
                frame,
                2.0,
                options(MAX_MESH_SPHERE_FACES + 1, 3),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn creates_rhino_ordered_rational_mesh_ellipsoid_and_pole_caps() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |cap_style| MeshEllipsoidOptions {
            vertical_count: 4,
            around_count: 6,
            cap_style,
        };
        let triangles = TriangleMesh::try_ellipsoid_grid(
            frame,
            [4.0, 3.0, 2.0],
            options(MeshCapFaceStyle::Triangles),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(triangles.vertices().len(), 20);
        assert_eq!(triangles.face_count(), 24);
        assert_eq!(triangles.vertices()[0], point(-4.0, 0.0, 0.0));
        assert_eq!(triangles.vertices()[19], point(4.0, 0.0, 0.0));
        assert!((triangles.vertices()[2].x() + 2.828_427_124_746_190_3).abs() < 1.0e-14);
        assert!((triangles.vertices()[2].y() - 1.037_414_057_018_886_8).abs() < 1.0e-14);
        assert!((triangles.vertices()[2].z() - 1.233_562_514_616_302_5).abs() < 1.0e-14);
        assert_eq!(triangles.faces()[0], MeshFace::Triangle([0, 2, 1]));
        assert_eq!(triangles.faces()[6], MeshFace::Quad([1, 2, 8, 7]));
        assert_eq!(triangles.faces()[18], MeshFace::Triangle([13, 14, 19]));
        assert!(triangles.topology().is_solid());
        assert!(triangles.signed_volume().unwrap() > 0.0);

        let quads = TriangleMesh::try_ellipsoid_grid(
            frame,
            [4.0, 3.0, 2.0],
            options(MeshCapFaceStyle::Quadrilaterals),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(quads.vertices(), triangles.vertices());
        assert_eq!(quads.face_count(), 18);
        assert_eq!(quads.faces()[0], MeshFace::Quad([0, 3, 2, 1]));
        assert_eq!(quads.faces()[2], MeshFace::Quad([0, 1, 6, 5]));
        assert_eq!(quads.faces()[15], MeshFace::Quad([13, 14, 15, 19]));
        assert_eq!(quads.faces()[17], MeshFace::Quad([17, 18, 13, 19]));
        assert!(quads.topology().is_solid());
        assert!(quads.signed_volume().unwrap() > 0.0);

        let odd = TriangleMesh::try_ellipsoid_grid(
            frame,
            [4.0, 3.0, 2.0],
            MeshEllipsoidOptions {
                vertical_count: 3,
                around_count: 5,
                cap_style: MeshCapFaceStyle::Quadrilaterals,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(odd.face_count(), 15);
        assert!(matches!(odd.faces()[0], MeshFace::Triangle(_)));
        assert!(matches!(odd.faces()[14], MeshFace::Triangle(_)));
    }

    #[test]
    fn mesh_ellipsoid_rejects_invalid_counts_radii_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |vertical_count, around_count| MeshEllipsoidOptions {
            vertical_count,
            around_count,
            cap_style: MeshCapFaceStyle::Triangles,
        };
        assert_eq!(
            TriangleMesh::try_ellipsoid_grid(
                frame,
                [1.0, 2.0, 3.0],
                options(1, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshEllipsoidFaceCount {
                vertical_count: 1,
                around_count: 4,
            })
        );
        assert!(matches!(
            TriangleMesh::try_ellipsoid_grid(
                frame,
                [1.0, 2.0, 3.0],
                options(2, 2),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshEllipsoidFaceCount { .. })
        ));
        assert_eq!(
            TriangleMesh::try_ellipsoid_grid(
                frame,
                [1.0, 0.0, 3.0],
                options(2, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshEllipsoidRadii)
        );
        assert!(matches!(
            TriangleMesh::try_ellipsoid_grid(
                frame,
                [1.0, Real::NAN, 3.0],
                options(2, 4),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::NonFinite {
                context: "mesh-ellipsoid radii"
            })
        ));
        assert_eq!(
            TriangleMesh::try_ellipsoid_grid(
                frame,
                [1.0, 2.0, 3.0],
                options(MAX_MESH_ELLIPSOID_FACES + 1, 3),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn creates_rhino_ordered_quad_sphere_subdivisions() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let base = TriangleMesh::try_quad_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 0 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let cube_coordinate = 2.0 / 3.0_f64.sqrt();
        assert_eq!(base.vertices().len(), 8);
        assert_eq!(base.face_count(), 6);
        assert_eq!(
            base.vertices()[0],
            point(-cube_coordinate, -cube_coordinate, -cube_coordinate)
        );
        assert_eq!(
            base.faces(),
            &[
                MeshFace::Quad([3, 2, 1, 0]),
                MeshFace::Quad([2, 6, 5, 1]),
                MeshFace::Quad([5, 6, 7, 4]),
                MeshFace::Quad([0, 4, 7, 3]),
                MeshFace::Quad([3, 7, 6, 2]),
                MeshFace::Quad([1, 5, 4, 0]),
            ]
        );

        let refined = TriangleMesh::try_quad_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 1 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(refined.vertices().len(), 26);
        assert_eq!(refined.face_count(), 24);
        assert_eq!(refined.vertices()[0], point(0.0, 0.0, -2.0));
        assert_eq!(refined.vertices()[5], point(0.0, -2.0, 0.0));
        assert!((refined.vertices()[6].y() + 2.0_f64.sqrt()).abs() < 1.0e-12);
        assert!((refined.vertices()[6].z() + 2.0_f64.sqrt()).abs() < 1.0e-12);
        assert_eq!(refined.faces()[0], MeshFace::Quad([7, 21, 11, 0]));
        assert_eq!(refined.faces()[3], MeshFace::Quad([6, 18, 7, 0]));
        assert_eq!(refined.faces()[23], MeshFace::Quad([8, 18, 6, 5]));
        assert!(refined.topology().is_solid());
        assert!(refined.signed_volume().unwrap() > 0.0);

        let twice_refined = TriangleMesh::try_quad_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 2 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(twice_refined.vertices().len(), 98);
        assert_eq!(twice_refined.face_count(), 96);
        assert_eq!(twice_refined.faces()[0], MeshFace::Quad([48, 79, 27, 0]));
        assert!((twice_refined.vertices()[0].x() + 0.731_390_176_158_283_5).abs() < 1.0e-15);
        assert!((twice_refined.vertices()[0].y() - 0.731_390_176_158_283_5).abs() < 1.0e-15);
        assert!((twice_refined.vertices()[0].z() + 1.711_764_242_072_578_5).abs() < 1.0e-15);
        assert!((twice_refined.vertices()[48].x() + 0.786_898_192_394_776_4).abs() < 1.0e-15);
        assert_eq!(twice_refined.vertices()[48].y(), 0.0);
        assert!((twice_refined.vertices()[48].z() + 1.838_692_805_991_754_9).abs() < 1.0e-15);

        let three_times_refined = TriangleMesh::try_quad_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 3 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(three_times_refined.vertices().len(), 386);
        assert_eq!(three_times_refined.face_count(), 384);
        assert_eq!(
            three_times_refined.faces()[0],
            MeshFace::Quad([192, 336, 145, 0])
        );
        assert!(
            (three_times_refined.vertices()[144].x() + 0.404_369_143_821_347_9).abs() < 1.0e-15
        );
        assert_eq!(three_times_refined.vertices()[144].y(), 0.0);
        assert!(
            (three_times_refined.vertices()[144].z() + 1.958_694_870_449_501_7).abs() < 1.0e-15
        );
    }

    #[test]
    fn creates_rhino_ordered_icosphere_subdivisions() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let base = TriangleMesh::try_ico_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 0 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(base.vertices().len(), 12);
        assert_eq!(base.face_count(), 20);
        assert!((base.vertices()[0].x() + 1.051_462_224_238_267_2).abs() < 1.0e-15);
        assert!((base.vertices()[0].y() - 1.701_301_616_704_08).abs() < 1.0e-15);
        assert_eq!(base.faces()[0], MeshFace::Triangle([0, 11, 5]));
        assert_eq!(base.faces()[19], MeshFace::Triangle([9, 8, 1]));

        let refined = TriangleMesh::try_ico_sphere(
            frame,
            2.0,
            MeshSubdivisionSphereOptions { subdivisions: 1 },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(refined.vertices().len(), 42);
        assert_eq!(refined.face_count(), 80);
        assert!((refined.vertices()[12].x() + 1.618_033_988_749_895).abs() < 1.0e-15);
        assert!((refined.vertices()[12].y() - 1.0).abs() < 1.0e-15);
        assert!((refined.vertices()[12].z() - 0.618_033_988_749_894_9).abs() < 1.0e-15);
        assert_eq!(refined.vertices()[16], point(0.0, 2.0, 0.0));
        assert_eq!(refined.vertices()[41], point(2.0, 0.0, 0.0));
        assert_eq!(refined.faces()[0], MeshFace::Triangle([0, 12, 14]));
        assert_eq!(refined.faces()[3], MeshFace::Triangle([12, 13, 14]));
        assert_eq!(refined.faces()[79], MeshFace::Triangle([41, 30, 23]));
        assert!(refined.topology().is_solid());
        assert!(refined.signed_volume().unwrap() > 0.0);
    }

    #[test]
    fn subdivision_spheres_reject_invalid_radii_and_style_limits() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            TriangleMesh::try_quad_sphere(
                frame,
                0.0,
                MeshSubdivisionSphereOptions { subdivisions: 0 },
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshSphereRadius)
        );
        assert!(matches!(
            TriangleMesh::try_ico_sphere(
                frame,
                Real::NAN,
                MeshSubdivisionSphereOptions { subdivisions: 0 },
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::NonFinite {
                context: "mesh-sphere radius"
            })
        ));
        assert_eq!(
            TriangleMesh::try_quad_sphere(
                frame,
                2.0,
                MeshSubdivisionSphereOptions {
                    subdivisions: MAX_MESH_QUAD_SPHERE_SUBDIVISIONS + 1,
                },
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshSphereSubdivisionCount {
                subdivisions: MAX_MESH_QUAD_SPHERE_SUBDIVISIONS + 1,
                maximum: MAX_MESH_QUAD_SPHERE_SUBDIVISIONS,
            })
        );
        assert_eq!(
            TriangleMesh::try_ico_sphere(
                frame,
                2.0,
                MeshSubdivisionSphereOptions {
                    subdivisions: MAX_MESH_ICO_SPHERE_SUBDIVISIONS + 1,
                },
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::InvalidMeshSphereSubdivisionCount {
                subdivisions: MAX_MESH_ICO_SPHERE_SUBDIVISIONS + 1,
                maximum: MAX_MESH_ICO_SPHERE_SUBDIVISIONS,
            })
        );
    }

    #[test]
    fn creates_rhino_ordered_mesh_torus_rows_and_wrapped_quads() {
        let world = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let minimal = TriangleMesh::try_torus_grid(
            world,
            4.0,
            1.0,
            MeshTorusOptions {
                vertical_count: 3,
                around_count: 3,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(minimal.vertices().len(), 9);
        assert_eq!(minimal.face_count(), 9);
        assert_eq!(minimal.vertices()[0], point(5.0, 0.0, 0.0));
        assert!((minimal.vertices()[3].x() - 3.5).abs() < 1.0e-12);
        assert!((minimal.vertices()[3].z() - 0.5 * 3.0_f64.sqrt()).abs() < 1.0e-12);
        assert_eq!(
            minimal.faces(),
            &[
                MeshFace::Quad([0, 1, 4, 3]),
                MeshFace::Quad([1, 2, 5, 4]),
                MeshFace::Quad([2, 0, 3, 5]),
                MeshFace::Quad([3, 4, 7, 6]),
                MeshFace::Quad([4, 5, 8, 7]),
                MeshFace::Quad([5, 3, 6, 8]),
                MeshFace::Quad([6, 7, 1, 0]),
                MeshFace::Quad([7, 8, 2, 1]),
                MeshFace::Quad([8, 6, 0, 2]),
            ]
        );
        assert!(minimal.topology().is_solid());
        assert!(minimal.signed_volume().unwrap() > 0.0);

        let oblique = Frame3::try_from_directions(
            point(1.0, 2.0, 3.0),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let oriented = TriangleMesh::try_torus_grid(
            oblique,
            4.0,
            1.0,
            MeshTorusOptions {
                vertical_count: 4,
                around_count: 4,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(oriented.vertices().len(), 16);
        assert_eq!(oriented.face_count(), 16);
        assert_eq!(oriented.vertices()[0], point(1.0, 7.0, 3.0));
        assert!((oriented.vertices()[4].x() - 2.0).abs() < 1.0e-12);
        assert!((oriented.vertices()[4].y() - 6.0).abs() < 1.0e-12);
        assert!((oriented.vertices()[4].z() - 3.0).abs() < 1.0e-12);
        assert_eq!(oriented.faces()[15], MeshFace::Quad([15, 12, 0, 3]));
        assert!(oriented.topology().is_solid());
    }

    #[test]
    fn mesh_torus_rejects_invalid_counts_radii_and_resource_overflow() {
        let frame = Frame3::try_from_directions(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let options = |vertical_count, around_count| MeshTorusOptions {
            vertical_count,
            around_count,
        };
        assert_eq!(
            TriangleMesh::try_torus_grid(frame, 4.0, 1.0, options(2, 3), Tolerance::DEFAULT),
            Err(GeometryError::InvalidMeshTorusFaceCount {
                vertical_count: 2,
                around_count: 3,
            })
        );
        assert!(matches!(
            TriangleMesh::try_torus_grid(frame, 4.0, 1.0, options(3, 2), Tolerance::DEFAULT),
            Err(GeometryError::InvalidMeshTorusFaceCount { .. })
        ));
        for (major_radius, minor_radius) in [(4.0, 0.0), (4.0, 4.0), (1.0, 2.0)] {
            assert_eq!(
                TriangleMesh::try_torus_grid(
                    frame,
                    major_radius,
                    minor_radius,
                    options(3, 3),
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidMeshTorusRadii)
            );
        }
        assert!(matches!(
            TriangleMesh::try_torus_grid(frame, Real::NAN, 1.0, options(3, 3), Tolerance::DEFAULT,),
            Err(GeometryError::NonFinite {
                context: "mesh-torus radii"
            })
        ));
        assert_eq!(
            TriangleMesh::try_torus_grid(
                frame,
                4.0,
                1.0,
                options(MAX_MESH_TORUS_FACES + 1, 3),
                Tolerance::DEFAULT,
            ),
            Err(GeometryError::TooManyMeshFaces)
        );
    }

    #[test]
    fn triangulates_quads_by_shortest_diagonal_in_rhino_face_order() {
        let vertices = vec![
            point(-3.0, 0.0, 0.0),
            point(-2.0, 0.0, 0.0),
            point(-3.0, 1.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(7.0, 0.0, 0.0),
            point(8.0, 0.0, 0.0),
            point(7.0, 1.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(11.0, 0.0, 0.0),
            point(12.0, 2.0, 0.0),
            point(10.0, 1.0, 0.0),
            point(15.0, 0.0, 0.0),
            point(16.0, 0.0, 0.0),
            point(16.0, 1.0, 0.0),
            point(15.0, 1.0, 0.0),
            point(20.0, 0.0, 0.0),
            point(22.0, 0.0, 0.0),
            point(22.0, 2.0, 1.0),
            point(20.0, 2.0, 0.0),
            point(99.0, 99.0, 99.0),
        ];
        let mesh = TriangleMesh::try_new_faces(
            vertices.clone(),
            vec![
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Quad([3, 4, 5, 6]),
                MeshFace::Triangle([7, 8, 9]),
                MeshFace::Quad([10, 11, 12, 13]),
                MeshFace::Quad([14, 15, 16, 17]),
                MeshFace::Quad([18, 19, 20, 21]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let (triangulated, converted) = mesh.triangulate_quads(Tolerance::DEFAULT).unwrap();
        assert_eq!(converted, 4);
        assert_eq!(triangulated.vertices(), vertices);
        assert_eq!(
            triangulated.faces(),
            &[
                MeshFace::Triangle([0, 1, 2]),
                MeshFace::Triangle([3, 4, 5]),
                MeshFace::Triangle([7, 8, 9]),
                MeshFace::Triangle([10, 11, 13]),
                MeshFace::Triangle([14, 15, 16]),
                MeshFace::Triangle([18, 19, 21]),
                MeshFace::Triangle([3, 5, 6]),
                MeshFace::Triangle([11, 12, 13]),
                MeshFace::Triangle([14, 16, 17]),
                MeshFace::Triangle([19, 20, 21]),
            ]
        );

        let triangle_only = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            triangle_only.triangulate_quads(Tolerance::DEFAULT),
            Ok((triangle_only.clone(), 0))
        );
    }

    #[test]
    fn swaps_welded_triangle_edge_in_rhino_face_order() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(99.0, 99.0, 99.0),
        ];
        for (faces, expected) in [
            (vec![[0, 1, 2], [0, 2, 3]], vec![[0, 1, 3], [2, 3, 1]]),
            (vec![[0, 2, 3], [0, 1, 2]], vec![[2, 3, 1], [0, 1, 3]]),
            (vec![[1, 2, 0], [2, 3, 0]], vec![[0, 1, 3], [2, 3, 1]]),
        ] {
            let mesh = TriangleMesh::try_new(vertices.clone(), faces, Tolerance::DEFAULT).unwrap();
            let edge =
                topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0));
            let swapped = mesh
                .swap_topology_edge(edge, Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert_eq!(swapped.vertices(), vertices);
            assert_eq!(swapped.triangles(), expected);
        }

        let shuffled_vertices = vec![
            point(2.0, 2.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
        ];
        let shuffled = TriangleMesh::try_new(
            shuffled_vertices.clone(),
            vec![[2, 1, 0], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&shuffled, point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0));
        let swapped = shuffled
            .swap_topology_edge(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(swapped.vertices(), shuffled_vertices);
        assert_eq!(swapped.triangles(), &[[2, 1, 3], [0, 3, 1]]);
    }

    #[test]
    fn rejects_mesh_edges_that_rhino_does_not_swap() {
        let edge_points = [point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0)];
        let rejected = [
            TriangleMesh::try_new(
                vec![
                    edge_points[0],
                    point(2.0, 0.0, 0.0),
                    edge_points[1],
                    point(0.0, 2.0, 0.0),
                ],
                vec![[0, 1, 2], [0, 3, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            TriangleMesh::try_new(
                vec![
                    edge_points[0],
                    point(2.0, 0.0, 0.0),
                    edge_points[1],
                    edge_points[0],
                    point(0.0, 2.0, 0.0),
                    edge_points[1],
                ],
                vec![[0, 1, 2], [3, 5, 4]],
                Tolerance::DEFAULT,
            )
            .unwrap(),
            TriangleMesh::try_new_faces(
                vec![
                    edge_points[0],
                    point(2.0, 0.0, 0.0),
                    edge_points[1],
                    point(0.0, 2.0, 0.0),
                    point(-1.0, 1.0, 0.0),
                ],
                vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Quad([0, 2, 3, 4])],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ];
        for mesh in rejected {
            let edge = topology_edge_index_between(&mesh, edge_points[0], edge_points[1]);
            assert_eq!(
                mesh.swap_topology_edge(edge, Tolerance::DEFAULT).unwrap(),
                None
            );
        }

        let naked = TriangleMesh::try_new(
            vec![edge_points[0], point(2.0, 0.0, 0.0), edge_points[1]],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&naked, edge_points[0], edge_points[1]);
        assert_eq!(
            naked.swap_topology_edge(edge, Tolerance::DEFAULT).unwrap(),
            None
        );
    }

    #[test]
    fn swap_mesh_edge_preserves_the_valid_mesh_invariant() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0));
        assert_eq!(
            mesh.swap_topology_edge(edge, Tolerance::DEFAULT).unwrap(),
            None
        );
        assert_eq!(
            mesh.swap_topology_edge(mesh.topology().edge_count(), Tolerance::DEFAULT),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: mesh.topology().edge_count(),
                edge_count: mesh.topology().edge_count(),
            })
        );
    }

    #[test]
    fn collapses_mesh_edge_to_midpoint_in_rhino_source_order() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(0.0, 0.0, 2.0),
        ];
        let mesh = TriangleMesh::try_new(
            vertices,
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        let collapsed = mesh
            .collapse_topology_edge(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            collapsed.vertices(),
            &[
                point(1.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 2.0),
            ]
        );
        assert_eq!(collapsed.triangles(), &[[0, 1, 2], [1, 0, 2]]);
    }

    #[test]
    fn collapse_mesh_edge_turns_adjacent_quads_into_rotated_triangles() {
        let boundary = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&boundary, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        let collapsed = boundary
            .collapse_topology_edge(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            collapsed.vertices(),
            &[
                point(1.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ]
        );
        assert_eq!(collapsed.triangles(), &[[0, 1, 2]]);

        let quad = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&quad, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        let collapsed = quad
            .collapse_topology_edge(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            collapsed.vertices(),
            &[
                point(0.0, 2.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
            ]
        );
        assert_eq!(collapsed.triangles(), &[[2, 0, 1]]);
    }

    #[test]
    fn collapse_mesh_edge_preserves_independent_unwelded_components() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
                point(-2.0, 1.0, 0.0),
                point(4.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [0, 6, 2], [3, 5, 7]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        let collapsed = mesh
            .collapse_topology_edge(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            collapsed.vertices(),
            &[
                point(1.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
                point(-2.0, 1.0, 0.0),
                point(4.0, -1.0, 0.0),
            ]
        );
        assert_eq!(collapsed.triangles(), &[[0, 4, 1], [2, 3, 5]]);
    }

    #[test]
    fn collapse_mesh_edge_reports_empty_invalid_and_degenerate_results() {
        let square = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let diagonal =
            topology_edge_index_between(&square, point(0.0, 0.0, 0.0), point(2.0, 2.0, 0.0));
        assert_eq!(
            square
                .collapse_topology_edge(diagonal, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        let edge_count = square.topology().edge_count();
        assert_eq!(
            square.collapse_topology_edge(edge_count, Tolerance::DEFAULT),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_count,
                edge_count,
            })
        );

        let degenerate = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(2.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&degenerate, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        assert_eq!(
            degenerate.collapse_topology_edge(edge, Tolerance::DEFAULT),
            Err(GeometryError::DegenerateTriangle { triangle: 0 })
        );

        let disconnected_quad = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(-2.0, 1.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Quad([3, 4, 5, 6])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(
            &disconnected_quad,
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
        );
        assert_eq!(
            disconnected_quad.collapse_topology_edge(edge, Tolerance::DEFAULT),
            Err(GeometryError::DegenerateQuad { face: 0 })
        );
    }

    #[test]
    fn splits_welded_mesh_edges_in_rhino_face_and_vertex_order() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let split = mesh
            .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            split.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
                point(1.0, 0.0, 0.0),
            ]
        );
        assert_eq!(
            split.triangles(),
            &[
                [1, 2, 3],
                [2, 0, 3],
                [2, 4, 0],
                [2, 1, 4],
                [3, 0, 4],
                [3, 4, 1],
            ]
        );

        let quad = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&quad, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let split = quad
            .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            split.faces(),
            &[
                MeshFace::Triangle([0, 4, 3]),
                MeshFace::Triangle([0, 1, 4]),
                MeshFace::Triangle([3, 4, 2]),
            ]
        );
    }

    #[test]
    fn split_mesh_edge_fully_separates_unwelded_replacement_faces() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -4.0, 0.0),
                point(-2.0, 1.0, 0.0),
                point(6.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [0, 6, 2], [3, 5, 7]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let split = mesh
            .split_topology_edge(edge, 0.25, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            split.faces(),
            &[
                MeshFace::Triangle([0, 4, 1]),
                MeshFace::Triangle([2, 3, 5]),
                MeshFace::Triangle([6, 7, 8]),
                MeshFace::Triangle([9, 10, 11]),
                MeshFace::Triangle([12, 14, 13]),
                MeshFace::Triangle([15, 17, 16]),
            ]
        );
        assert_eq!(split.vertices().len(), 18);
        assert_eq!(split.vertices()[6], point(0.0, 4.0, 0.0));
        assert_eq!(split.vertices()[7], point(0.0, 0.0, 0.0));
        assert_eq!(split.vertices()[8], point(1.0, 0.0, 0.0));
        assert_eq!(split.vertices()[12], point(0.0, -4.0, 0.0));
        assert_eq!(split.vertices()[13], point(0.0, 0.0, 0.0));
        assert_eq!(split.vertices()[14], point(1.0, 0.0, 0.0));
    }

    #[test]
    fn split_mesh_edge_matches_endpoint_rejection_and_validation_behavior() {
        let triangle = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&triangle, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let endpoint = triangle
            .split_topology_edge(edge, 0.0, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert_eq!(
            endpoint.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(endpoint.triangles(), &[[0, 1, 2], [2, 3, 1]]);
        assert_eq!(
            triangle
                .split_topology_edge(edge, -0.25, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        assert_eq!(
            triangle
                .split_topology_edge(edge, 1.25, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        assert_eq!(
            triangle
                .split_topology_edge(edge, f64::NAN, Tolerance::DEFAULT)
                .unwrap(),
            None
        );
        assert_eq!(
            triangle.split_topology_edge(edge, 1.0e-12, Tolerance::DEFAULT),
            Err(GeometryError::DegenerateTriangle { triangle: 0 })
        );
        let edge_count = triangle.topology().edge_count();
        assert_eq!(
            triangle.split_topology_edge(edge_count, 0.5, Tolerance::DEFAULT),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_count,
                edge_count,
            })
        );
    }

    #[test]
    fn fills_a_naked_mesh_hole_with_an_oriented_delaunay_patch() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge = topology_edge_index_between(&mesh, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let fill = mesh
            .fill_topology_hole(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();

        assert_eq!(
            fill.patch().vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
            ]
        );
        let mut patch_triangle = fill.patch().triangles()[0];
        patch_triangle.sort_unstable();
        assert_eq!(patch_triangle, [0, 1, 2]);
        assert_eq!(
            &fill.filled().vertices()[4..],
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        let mut joined_triangle = *fill.filled().triangles().last().unwrap();
        joined_triangle.sort_unstable();
        assert_eq!(joined_triangle, [4, 5, 6]);
        assert!(fill.filled().topology().is_solid());
    }

    #[test]
    fn fills_concave_and_nonplanar_mesh_holes_deterministically() {
        let concave = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(2.0, 2.0, 4.0),
            ],
            vec![[0, 1, 5], [1, 2, 5], [2, 3, 5], [3, 4, 5], [4, 0, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&concave, point(0.0, 0.0, 0.0), point(4.0, 0.0, 0.0));
        let patch = concave
            .fill_topology_hole(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap()
            .patch()
            .clone();
        let mut triangles = patch
            .triangles()
            .iter()
            .map(|triangle| {
                let mut triangle = *triangle;
                triangle.sort_unstable();
                triangle
            })
            .collect::<Vec<_>>();
        triangles.sort_unstable();
        assert_eq!(triangles, [[0, 1, 3], [0, 3, 4], [1, 2, 3]]);

        let nonplanar = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 1.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, -1.0),
                point(2.0, 2.0, 4.0),
            ],
            vec![[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&nonplanar, point(0.0, 0.0, 0.0), point(4.0, 0.0, 1.0));
        let filled = nonplanar
            .fill_topology_hole(edge, Tolerance::DEFAULT)
            .unwrap()
            .unwrap()
            .filled()
            .clone();
        assert_eq!(filled.face_count(), 6);
        assert!(filled.topology().is_solid());
    }

    #[test]
    fn mesh_hole_fill_rejects_non_naked_open_and_branched_boundaries() {
        let square = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let diagonal =
            topology_edge_index_between(&square, point(0.0, 0.0, 0.0), point(4.0, 4.0, 0.0));
        assert_eq!(
            square
                .fill_topology_hole(diagonal, Tolerance::DEFAULT)
                .unwrap(),
            None
        );

        let branched = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(-2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edge =
            topology_edge_index_between(&branched, point(0.0, 0.0, 0.0), point(2.0, 0.0, 0.0));
        assert_eq!(
            branched
                .fill_topology_hole(edge, Tolerance::DEFAULT)
                .unwrap(),
            None
        );

        let edge_count = square.topology().edge_count();
        assert_eq!(
            square.fill_topology_hole(edge_count, Tolerance::DEFAULT),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: edge_count,
                edge_count,
            })
        );
    }

    #[test]
    fn fills_all_simple_mesh_holes_without_unused_closing_vertices() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(0.0, 0.0, 4.0),
                point(10.0, 0.0, 0.0),
                point(14.0, 0.0, 0.0),
                point(10.0, 4.0, 0.0),
                point(10.0, 0.0, 4.0),
            ],
            vec![
                [0, 1, 3],
                [1, 2, 3],
                [2, 0, 3],
                [4, 5, 7],
                [5, 6, 7],
                [6, 4, 7],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (filled, hole_count) = mesh.fill_holes(Tolerance::DEFAULT).unwrap();
        assert_eq!(hole_count, 2);
        assert_eq!(filled.face_count(), 8);
        assert_eq!(filled.vertices().len(), 14);
        assert_eq!(
            &filled.vertices()[8..],
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(14.0, 0.0, 0.0),
                point(10.0, 4.0, 0.0),
            ]
        );
        assert_eq!(filled.topology().boundary_edge_count(), 0);
        assert_eq!(filled.topology().orientation_conflict_edge_count(), 0);

        let (unchanged, hole_count) = filled.fill_holes(Tolerance::DEFAULT).unwrap();
        assert_eq!(hole_count, 0);
        assert_eq!(unchanged, filled);
    }

    #[test]
    fn fill_all_holes_leaves_ambiguous_branched_boundaries_unchanged() {
        let branched = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(-2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            branched.fill_holes(Tolerance::DEFAULT).unwrap(),
            (branched.clone(), 0)
        );
    }

    #[test]
    fn computes_closed_quad_mesh_topology_and_volume() {
        let cube = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(1.0, 0.0, 1.0),
                point(1.0, 1.0, 1.0),
                point(0.0, 1.0, 1.0),
            ],
            vec![
                MeshFace::Quad([0, 3, 2, 1]),
                MeshFace::Quad([4, 5, 6, 7]),
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(cube.face_count(), 6);
        assert_eq!(cube.triangles().len(), 12);
        assert_eq!(cube.topology().edge_count(), 12);
        assert!(cube.topology().is_solid());
        assert!((cube.signed_volume().unwrap() - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn topology_detects_closed_oriented_and_open_meshes() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let tetrahedron =
            TriangleMesh::try_new(vertices, faces.clone(), Tolerance::DEFAULT).unwrap();
        let topology = tetrahedron.topology();
        assert_eq!(topology.topological_vertex_count(), 4);
        assert_eq!(topology.edge_count(), 6);
        assert_eq!(topology.boundary_edge_count(), 0);
        assert_eq!(topology.non_manifold_edge_count(), 0);
        assert_eq!(topology.orientation_conflict_edge_count(), 0);
        assert!(topology.is_closed());
        assert!(topology.is_manifold());
        assert!(topology.is_oriented());
        assert!(topology.is_solid());

        let open = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert_eq!(open.boundary_edge_count(), 3);
        assert!(!open.is_closed());
        assert!(open.is_manifold());
        assert!(open.is_oriented());

        let mut flipped_faces = faces;
        flipped_faces[0].swap(1, 2);
        let unoriented = TriangleMesh::try_new(
            tetrahedron.vertices().to_vec(),
            flipped_faces,
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert!(unoriented.is_closed());
        assert!(unoriented.is_manifold());
        assert!(!unoriented.is_oriented());
        assert_eq!(unoriented.orientation_conflict_edge_count(), 3);
        assert!(!unoriented.is_solid());
    }

    #[test]
    fn extracts_welded_mesh_boundary_loops_without_internal_edges() {
        let square_soup = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let borders = square_soup.boundary_polylines(Tolerance::DEFAULT).unwrap();
        assert_eq!(borders.len(), 1);
        assert!(borders[0].is_closed());
        assert_eq!(borders[0].segment_count(), 4);
        assert!((borders[0].length().unwrap() - 8.0).abs() < 1.0e-12);

        let ring = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(3.0, 3.0, 0.0),
                point(1.0, 3.0, 0.0),
            ],
            vec![
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mut lengths = ring
            .boundary_polylines(Tolerance::DEFAULT)
            .unwrap()
            .into_iter()
            .map(|border| {
                assert!(border.is_closed());
                border.length().unwrap()
            })
            .collect::<Vec<_>>();
        lengths.sort_by(Real::total_cmp);
        assert_eq!(lengths, [8.0, 16.0]);

        let closed = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            closed
                .boundary_polylines(Tolerance::DEFAULT)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn filters_naked_unwelded_and_face_angle_edges() {
        let welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            welded
                .filtered_edge_lines(MeshEdgeFilter::Naked, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            welded
                .filtered_edge_lines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );

        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            unwelded
                .filtered_edge_lines(MeshEdgeFilter::Naked, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            unwelded
                .filtered_edge_lines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
                .unwrap()
                .len(),
            5
        );

        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [0, 3, 1]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let around_right_angle = MeshEdgeFilter::FaceAngle {
            greater_than_radians: 89.0_f64.to_radians(),
            less_than_radians: 91.0_f64.to_radians(),
        };
        assert_eq!(
            folded
                .filtered_edge_lines(around_right_angle, Tolerance::DEFAULT)
                .unwrap(),
            vec![
                LineSegment::try_new(
                    point(0.0, 0.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap()
            ]
        );
        for strict_boundary in [
            MeshEdgeFilter::FaceAngle {
                greater_than_radians: 90.0_f64.to_radians(),
                less_than_radians: 91.0_f64.to_radians(),
            },
            MeshEdgeFilter::FaceAngle {
                greater_than_radians: 89.0_f64.to_radians(),
                less_than_radians: 90.0_f64.to_radians(),
            },
        ] {
            assert!(
                folded
                    .filtered_edge_lines(strict_boundary, Tolerance::DEFAULT)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn joins_branched_unwelded_edges_into_edge_exact_euler_trails() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let polylines = mesh
            .filtered_edge_polylines(MeshEdgeFilter::Unwelded, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(polylines.len(), 1);
        assert_eq!(
            polylines[0].vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 3.0, 0.0),
            ]
        );
        assert_eq!(polylines[0].segment_count(), 5);
    }

    #[test]
    fn decomposes_many_odd_edge_vertices_without_losing_edges() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, -1.0, 0.0),
        ];
        let edges = [[0, 1], [0, 2], [0, 3], [0, 4]];
        let polylines = topology_edge_polylines(&points, &edges, Tolerance::DEFAULT).unwrap();
        assert_eq!(polylines.len(), 2);
        assert_eq!(
            polylines
                .iter()
                .map(Polyline3::segment_count)
                .sum::<usize>(),
            edges.len()
        );
        let mut actual = BTreeSet::new();
        for segment in polylines.iter().flat_map(Polyline3::segments) {
            let mut vertices = [segment.start(), segment.end()].map(|point| {
                points
                    .iter()
                    .position(|candidate| *candidate == point)
                    .unwrap()
            });
            vertices.sort_unstable();
            assert!(actual.insert(vertices));
        }
        assert_eq!(actual, edges.into_iter().collect());
    }

    #[test]
    fn logical_edges_join_only_across_locally_smooth_face_regions() {
        let planar = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let split = planar
            .logical_edge_polylines(0.0, Tolerance::DEFAULT)
            .unwrap();
        let mut split_segment_counts = split
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        split_segment_counts.sort_unstable();
        assert_eq!(split_segment_counts, [1, 2, 2]);
        assert_eq!(split_segment_counts.iter().sum::<usize>(), 5);

        let joined = planar
            .logical_edge_polylines(1.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(joined.len(), 1);
        assert!(joined[0].is_closed());
        assert_eq!(joined[0].segment_count(), 4);
    }

    #[test]
    fn logical_edges_use_a_strict_face_grouping_break_angle() {
        let folded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [0, 3, 1]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let at_right_angle = folded
            .logical_edge_polylines(90.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        let mut segment_counts = at_right_angle
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        segment_counts.sort_unstable();
        assert_eq!(segment_counts, [1, 2, 2]);
        assert_eq!(segment_counts.iter().sum::<usize>(), 5);

        let above_right_angle = folded
            .logical_edge_polylines(91.0_f64.to_radians(), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(above_right_angle.len(), 1);
        assert!(above_right_angle[0].is_closed());
        assert_eq!(above_right_angle[0].segment_count(), 4);
    }

    #[test]
    fn logical_edges_never_smooth_across_an_unwelded_seam() {
        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let edges = unwelded
            .logical_edge_polylines(std::f64::consts::PI, Tolerance::DEFAULT)
            .unwrap();
        let mut segment_counts = edges
            .iter()
            .map(Polyline3::segment_count)
            .collect::<Vec<_>>();
        segment_counts.sort_unstable();
        assert_eq!(segment_counts, [1, 2, 2]);
        assert_eq!(segment_counts.iter().sum::<usize>(), 5);
    }

    #[test]
    fn rejects_invalid_mesh_break_angles() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for break_angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.logical_edge_polylines(break_angle, Tolerance::DEFAULT),
                Err(GeometryError::InvalidMeshBreakAngle)
            );
        }
        assert!(mesh.logical_edge_polylines(0.0, Tolerance::DEFAULT).is_ok());
        assert!(
            mesh.logical_edge_polylines(std::f64::consts::PI, Tolerance::DEFAULT)
                .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_mesh_face_angle_intervals() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for (greater, less) in [
            (-1.0, 1.0),
            (1.0, 1.0),
            (0.0, std::f64::consts::PI + 1.0),
            (Real::NAN, 1.0),
            (0.0, Real::INFINITY),
        ] {
            assert_eq!(
                mesh.filtered_edge_lines(
                    MeshEdgeFilter::FaceAngle {
                        greater_than_radians: greater,
                        less_than_radians: less,
                    },
                    Tolerance::DEFAULT,
                ),
                Err(GeometryError::InvalidMeshFaceAngleInterval)
            );
        }
    }

    #[test]
    fn wireframe_returns_each_welded_topology_edge_once() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let lines = mesh.wireframe_lines(Tolerance::DEFAULT).unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines.len(), mesh.topology().edge_count());
        assert_eq!(
            lines
                .iter()
                .filter(|line| {
                    [line.start(), line.end()].contains(&point(0.0, 0.0, 0.0))
                        && [line.start(), line.end()].contains(&point(2.0, 2.0, 0.0))
                })
                .count(),
            1
        );
    }

    #[test]
    fn computes_translation_stable_oriented_volume_and_reports_overflow() {
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let tetrahedron = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 4.0),
            ],
            faces.clone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(tetrahedron.signed_volume().unwrap(), 4.0);
        assert_eq!(tetrahedron.reversed().signed_volume().unwrap(), -4.0);

        let translated = TriangleMesh::try_new(
            vec![
                point(1.0e9, -2.0e9, 3.0e9),
                point(1.0e9 + 2.0, -2.0e9, 3.0e9),
                point(1.0e9, -2.0e9 + 3.0, 3.0e9),
                point(1.0e9, -2.0e9, 3.0e9 + 4.0),
                point(1.0e200, -1.0e200, 1.0e200),
            ],
            faces.clone(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(translated.signed_volume().unwrap(), 4.0);

        let overflowing = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0e110, 0.0, 0.0),
                point(0.0, 3.0e110, 0.0),
                point(0.0, 0.0, 4.0e110),
            ],
            faces,
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            overflowing.signed_volume(),
            Err(GeometryError::NonFinite {
                context: "mesh volume"
            })
        );
    }

    #[test]
    fn topology_welds_triangle_soup_and_reports_non_manifold_edges() {
        let locations = [
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let oriented_faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let mut soup_vertices = Vec::new();
        let mut soup_faces = Vec::new();
        for face in oriented_faces {
            let start = soup_vertices.len() as u32;
            soup_vertices.extend(face.map(|index| locations[index]));
            soup_faces.push([start, start + 1, start + 2]);
        }
        let soup = TriangleMesh::try_new(soup_vertices, soup_faces, Tolerance::DEFAULT).unwrap();
        let topology = soup.topology();
        assert_eq!(soup.vertices().len(), 12);
        assert_eq!(topology.topological_vertex_count(), 4);
        assert!(topology.is_solid());

        let vertices = vec![
            locations[0],
            locations[1],
            locations[2],
            locations[3],
            point(0.0, 0.0, -1.0),
        ];
        let two_tetrahedra = TriangleMesh::try_new(
            vertices,
            vec![
                [0, 2, 1],
                [0, 1, 3],
                [0, 3, 2],
                [1, 2, 3],
                [0, 1, 2],
                [0, 4, 1],
                [0, 2, 4],
                [1, 4, 2],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .topology();
        assert!(two_tetrahedra.is_closed());
        assert_eq!(two_tetrahedra.non_manifold_edge_count(), 3);
        assert!(!two_tetrahedra.is_manifold());
        assert!(!two_tetrahedra.is_oriented());
        assert!(!two_tetrahedra.is_solid());
    }

    #[test]
    fn reverses_and_unifies_face_winding_without_reordering_faces() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let oriented_faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let oriented =
            TriangleMesh::try_new(vertices.clone(), oriented_faces.clone(), Tolerance::DEFAULT)
                .unwrap();
        let reversed = oriented.reversed();
        assert_eq!(reversed.reversed(), oriented);
        for index in 0..oriented.triangles().len() {
            let normal = oriented.face_normal(index).unwrap();
            let reversed_normal = reversed.face_normal(index).unwrap();
            let dot = normal.as_vector().dot(reversed_normal.as_vector()).unwrap();
            assert!((dot + 1.0).abs() <= 2.0 * Real::EPSILON);
        }

        let mut inconsistent_faces = oriented_faces.clone();
        inconsistent_faces[1].swap(1, 2);
        let inconsistent =
            TriangleMesh::try_new(vertices, inconsistent_faces, Tolerance::DEFAULT).unwrap();
        assert_eq!(inconsistent.topology().orientation_conflict_edge_count(), 3);
        let (unified, flipped_face_count) = inconsistent.unified_face_orientations().unwrap();
        assert_eq!(flipped_face_count, 1);
        assert_eq!(unified.triangles(), oriented_faces);
        assert!(unified.topology().is_oriented());
        assert_eq!(inconsistent.triangles()[1], [0, 3, 1]);
    }

    #[test]
    fn unification_uses_exact_locations_and_rejects_non_orientable_topology() {
        let locations = [
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
        ];
        let mut soup_vertices = Vec::new();
        let mut soup_faces = Vec::new();
        for (face_index, mut face) in [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]
            .into_iter()
            .enumerate()
        {
            if face_index == 2 {
                face.swap(1, 2);
            }
            let start = soup_vertices.len() as u32;
            soup_vertices.extend(face.map(|index| locations[index]));
            soup_faces.push([start, start + 1, start + 2]);
        }
        let soup = TriangleMesh::try_new(soup_vertices, soup_faces, Tolerance::DEFAULT).unwrap();
        let (unified, flipped_face_count) = soup.unified_face_orientations().unwrap();
        assert_eq!(flipped_face_count, 1);
        assert!(unified.topology().is_oriented());

        const SEGMENTS: usize = 4;
        let mut vertices = Vec::with_capacity(SEGMENTS * 2);
        for index in 0..SEGMENTS {
            let angle = std::f64::consts::TAU * index as Real / SEGMENTS as Real;
            for lateral in [-0.25, 0.25] {
                let radius = 2.0 + lateral * (angle * 0.5).cos();
                vertices.push(point(
                    radius * angle.cos(),
                    radius * angle.sin(),
                    lateral * (angle * 0.5).sin(),
                ));
            }
        }
        let mut faces = Vec::with_capacity(SEGMENTS * 2);
        for index in 0..SEGMENTS {
            let a = (index * 2) as u32;
            let b = a + 1;
            let (next_a, next_b) = if index + 1 == SEGMENTS {
                (1, 0)
            } else {
                (a + 2, b + 2)
            };
            faces.push([a, next_a, b]);
            faces.push([next_a, next_b, b]);
        }
        let mobius = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();
        assert_eq!(
            mobius.unified_face_orientations(),
            Err(GeometryError::NonOrientableMesh)
        );
    }

    #[test]
    fn splits_edge_connected_components_and_compacts_vertices_in_source_order() {
        let source = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [4, 5, 6], [0, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = source.disjoint_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            pieces[0].vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
            ]
        );
        assert_eq!(pieces[0].triangles(), &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(
            pieces[1].vertices(),
            &[
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
            ]
        );
        assert_eq!(pieces[1].triangles(), &[[0, 1, 2]]);
    }

    #[test]
    fn disjoint_pieces_require_an_edge_but_accept_exact_duplicate_edge_locations() {
        let vertex_touch = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = vertex_touch.disjoint_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].vertices()[0], pieces[1].vertices()[0]);

        let duplicate_edge = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let pieces = duplicate_edge.disjoint_pieces();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0], duplicate_edge);
    }

    #[test]
    fn explode_pieces_split_only_edges_unwelded_at_both_raw_endpoints() {
        let welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(welded.explode_pieces(), vec![welded.clone()]);

        let unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unwelded.disjoint_pieces().len(), 1);
        let pieces = unwelded.explode_pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].triangles(), &[[0, 1, 2]]);
        assert_eq!(pieces[1].triangles(), &[[0, 1, 2]]);
        assert_eq!(pieces[0].vertices(), &unwelded.vertices()[..3]);
        assert_eq!(pieces[1].vertices(), &unwelded.vertices()[3..]);

        let half_welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 2.0),
            ],
            vec![[0, 1, 2], [3, 0, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(half_welded.explode_pieces(), vec![half_welded.clone()]);
    }

    #[test]
    fn extracts_requested_faces_in_caller_order_and_compacts_both_parts() {
        let vertices = vec![
            point(99.0, 99.0, 99.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(88.0, 88.0, 88.0),
            point(0.0, 2.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(1.0, 1.0, 1.0),
        ];
        let faces = vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]];
        let mesh = TriangleMesh::try_new(vertices, faces, Tolerance::DEFAULT).unwrap();

        let extraction = mesh.extract_faces(&[2, 0]).unwrap();
        assert_eq!(
            extraction.extracted().vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(1.0, 1.0, 1.0),
            ]
        );
        assert_eq!(extraction.extracted().triangles(), &[[1, 3, 4], [2, 0, 4]]);
        let remainder = extraction.remainder().unwrap();
        assert_eq!(remainder.vertices(), extraction.extracted().vertices());
        assert_eq!(remainder.triangles(), &[[0, 1, 4], [3, 2, 4]]);

        let all = mesh.extract_faces(&[3, 2, 1, 0]).unwrap();
        assert!(all.remainder().is_none());
        assert_eq!(
            all.extracted().triangles(),
            &[[3, 2, 4], [1, 3, 4], [0, 1, 4], [2, 0, 4]]
        );

        let mixed = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([0, 1, 4])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let mixed_extraction = mixed.extract_faces(&[0]).unwrap();
        assert_eq!(
            mixed_extraction.extracted().faces(),
            &[MeshFace::Quad([0, 1, 2, 3])]
        );
        assert_eq!(
            mixed_extraction.remainder().unwrap().faces(),
            &[MeshFace::Triangle([0, 1, 2])]
        );
    }

    #[test]
    fn deletes_requested_faces_in_source_order_and_compacts_the_remainder() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(99.0, 99.0, 99.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(88.0, 88.0, 88.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(1.0, 1.0, 1.0),
            ],
            vec![[4, 1, 6], [1, 2, 6], [2, 5, 6], [5, 4, 6]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let remainder = mesh.delete_faces(&[2, 0]).unwrap().unwrap();
        assert_eq!(
            remainder.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(2.0, 2.0, 0.0),
                point(1.0, 1.0, 1.0),
            ]
        );
        assert_eq!(remainder.triangles(), &[[0, 1, 4], [3, 2, 4]]);
        assert!(mesh.delete_faces(&[3, 2, 1, 0]).unwrap().is_none());

        let mixed = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([0, 1, 4])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            mixed.delete_faces(&[0]).unwrap().unwrap().faces(),
            &[MeshFace::Triangle([0, 1, 2])]
        );
    }

    #[test]
    fn finds_closest_points_on_triangle_and_quad_faces() {
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 4.0, 0.0),
                point(0.0, 4.0, 0.0),
                point(10.0, 0.0, 2.0),
                point(12.0, 0.0, 2.0),
                point(10.0, 2.0, 2.0),
            ],
            vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 5, 6])],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(
            mesh.closest_point_on_face(0, point(1.0, 3.0, 5.0))
                .unwrap()
                .is_near(point(1.0, 3.0, 0.0), Tolerance::DEFAULT)
        );
        assert!(
            mesh.closest_point_on_face(1, point(13.0, 3.0, 4.0))
                .unwrap()
                .is_near(point(11.0, 1.0, 2.0), Tolerance::DEFAULT)
        );
        assert_eq!(
            mesh.closest_point_on_face(2, point(0.0, 0.0, 0.0)),
            Err(GeometryError::MeshFaceIndexOutOfRange {
                face: 2,
                face_count: 2,
            })
        );
    }

    #[test]
    fn face_subset_edits_reject_empty_duplicate_and_out_of_range_subsets() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            mesh.extract_faces(&[]),
            Err(GeometryError::EmptyMeshFaceSubset)
        );
        assert_eq!(
            mesh.extract_faces(&[0, 0]),
            Err(GeometryError::DuplicateMeshFaceIndex { face: 0 })
        );
        assert_eq!(
            mesh.extract_faces(&[1]),
            Err(GeometryError::MeshFaceIndexOutOfRange {
                face: 1,
                face_count: 1,
            })
        );
        assert_eq!(
            mesh.delete_faces(&[]),
            Err(GeometryError::EmptyMeshFaceSubset)
        );
        assert_eq!(
            mesh.delete_faces(&[0, 0]),
            Err(GeometryError::DuplicateMeshFaceIndex { face: 0 })
        );
        assert_eq!(
            mesh.delete_faces(&[1]),
            Err(GeometryError::MeshFaceIndexOutOfRange {
                face: 1,
                face_count: 1,
            })
        );
    }

    #[test]
    fn extracts_all_or_only_hanging_faces_from_non_manifold_edges() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(0.0, -1.0, 1.0),
        ];
        let faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]];
        let mesh =
            TriangleMesh::try_new(vertices.clone(), faces.clone(), Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.topology().non_manifold_edge_count(), 1);

        let all = mesh.extract_non_manifold_faces(3, false).unwrap().unwrap();
        assert_eq!(all.extracted().vertices(), vertices);
        assert_eq!(all.extracted().triangles(), &[faces[0], faces[1], faces[4]]);
        let remainder = all.remainder().unwrap();
        assert_eq!(remainder.vertices(), &vertices[..4]);
        assert_eq!(remainder.triangles(), &[faces[2], faces[3]]);

        let hanging = mesh.extract_non_manifold_faces(3, true).unwrap().unwrap();
        assert_eq!(
            hanging.extracted().vertices(),
            &[vertices[0], vertices[1], vertices[4]]
        );
        assert_eq!(hanging.extracted().triangles(), &[[0, 1, 2]]);
        assert_eq!(hanging.remainder().unwrap().vertices(), &vertices[..4]);
        assert_eq!(hanging.remainder().unwrap().triangles(), &faces[..4]);

        assert!(mesh.extract_non_manifold_faces(4, false).unwrap().is_none());
        assert_eq!(
            mesh.extract_non_manifold_faces(2, false),
            Err(GeometryError::InvalidNonManifoldMinimumFaceCount(2))
        );
    }

    #[test]
    fn extracts_exact_location_duplicate_faces_independent_of_winding() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(-0.0, 0.0, 0.0),
            point(2.0, -0.0, 0.0),
            point(0.0, 2.0, -0.0),
            point(0.0, 0.0, 1.0e-12),
            point(2.0, 0.0, 1.0e-12),
            point(0.0, 2.0, 1.0e-12),
        ];
        let faces = vec![[0, 1, 2], [1, 2, 0], [3, 5, 4], [6, 7, 8]];
        let mesh =
            TriangleMesh::try_new(points.to_vec(), faces.clone(), Tolerance::DEFAULT).unwrap();
        let extraction = mesh.extract_duplicate_faces().unwrap();

        assert_eq!(extraction.extracted().triangles(), &[[1, 2, 0], [3, 5, 4]]);
        assert_eq!(extraction.extracted().vertices(), &points[..6]);
        let remainder = extraction.remainder().unwrap();
        assert_eq!(remainder.triangles(), &[[0, 1, 2], [3, 4, 5]]);
        assert_eq!(
            remainder.vertices(),
            &[
                points[0], points[1], points[2], points[6], points[7], points[8]
            ]
        );

        let signed_zero_duplicate = TriangleMesh::try_new(
            points[..6].to_vec(),
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(signed_zero_duplicate.extract_duplicate_faces().is_some());
        let truly_unique = TriangleMesh::try_new(
            points.to_vec(),
            vec![[0, 1, 2], [6, 7, 8]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert!(truly_unique.extract_duplicate_faces().is_none());
    }

    #[test]
    fn combines_identical_vertices_in_rhino_order_without_culling_unused() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(2.0, -0.0, 0.0),
            point(-0.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
            point(0.0, 2.0, -0.0),
            point(99.0, 99.0, 99.0),
        ];
        let mesh = TriangleMesh::try_new(vertices, vec![[0, 1, 2], [3, 4, 5]], Tolerance::DEFAULT)
            .unwrap();
        let (combined, removed) = mesh.combined_identical_vertices();
        assert_eq!(removed, 3);
        assert_eq!(
            combined.vertices(),
            &[
                point(99.0, 99.0, 99.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ]
        );
        assert_eq!(combined.triangles(), &[[3, 1, 2], [1, 3, 4]]);

        let unique = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(unique.combined_identical_vertices(), (unique.clone(), 0));

        let near = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 1.0e-12),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(near.combined_identical_vertices(), (near.clone(), 0));
    }

    #[test]
    fn welds_smooth_coincident_edge_endpoints_in_rhino_order() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, removed) = mesh.welded_vertices(0.0).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3]]);
        assert_eq!(welded.topology().edge_count(), 5);
        assert_eq!(welded.explode_pieces().len(), 1);
    }

    #[test]
    fn weld_respects_angle_and_never_merges_a_vertex_only_contact() {
        let right_angle = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 3.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (below, removed) = right_angle.welded_vertices(89.0_f64.to_radians()).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(below.vertices(), &right_angle.vertices()[..6]);
        assert_eq!(below.triangles(), right_angle.triangles());
        let (above, removed) = right_angle
            .welded_vertices(90.001_f64.to_radians())
            .unwrap();
        assert_eq!(removed, 3);
        assert_eq!(above.triangles(), &[[2, 1, 0], [1, 2, 3]]);

        let vertex_only = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(-2.0, 0.0, 0.0),
                point(0.0, -2.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            vertex_only.welded_vertices(std::f64::consts::PI),
            Ok((vertex_only.clone(), 0))
        );
    }

    #[test]
    fn weld_rejects_invalid_angle_tolerances() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.welded_vertices(angle),
                Err(GeometryError::InvalidMeshWeldAngle)
            );
        }
    }

    #[test]
    fn welds_selected_topology_edges_with_earliest_survivors_and_compaction() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = mesh.welded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3]]);
        assert_eq!(mesh.welded_topology_edges(&[0, 0]).unwrap().0, welded);

        let (empty, edge_count) = mesh.welded_topology_edges(&[]).unwrap();
        assert_eq!((empty, edge_count), (mesh.clone(), 0));
        assert_eq!(
            mesh.welded_topology_edges(&[5]),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: 5,
                edge_count: 5,
            })
        );

        let already_welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_welded.welded_topology_edges(&[0]).unwrap(),
            (welded.clone(), 0)
        );

        let half_welded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 0, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            half_welded.welded_topology_edges(&[0]).unwrap(),
            (welded.clone(), 1)
        );

        let naked = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (compacted, edge_count) = naked.welded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(compacted.vertices(), &naked.vertices()[..3]);
        assert_eq!(compacted.triangles(), naked.triangles());
    }

    #[test]
    fn selected_edge_welding_handles_closed_non_manifold_and_disjoint_seams() {
        let fan = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
            ],
            vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [5, 6, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = fan.welded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(welded.vertices(), &fan.vertices()[..5]);
        assert_eq!(
            welded.triangles(),
            &[[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]]
        );

        let non_manifold = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = non_manifold.welded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3], [0, 1, 4]]);

        let disjoint = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 1.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(10.0, -1.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8], [9, 10, 11]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = disjoint.welded_topology_edges(&[5, 0]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(welded.vertices().len(), 8);
        assert_eq!(
            welded.triangles(),
            &[[0, 1, 2], [1, 0, 3], [4, 5, 6], [5, 4, 7]]
        );
    }

    #[test]
    fn weld_limits_non_manifold_edges_to_the_first_two_face_uses() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, removed) = mesh.welded_vertices(std::f64::consts::PI).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 1.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3], [4, 5, 6]]);
    }

    #[test]
    fn welds_every_joined_seam_incident_to_selected_topology_vertices() {
        let seam = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let expected = TriangleMesh::try_new(
            vec![
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[2, 1, 0], [1, 2, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for selection in [&[0][..], &[1][..], &[1, 0, 1][..]] {
            assert_eq!(
                seam.welded_topology_vertices(selection).unwrap(),
                (expected.clone(), 1)
            );
        }
        assert_eq!(
            seam.welded_topology_vertices(&[]).unwrap(),
            (seam.clone(), 0)
        );
        assert_eq!(
            seam.welded_topology_vertices(&[5]),
            Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                vertex: 5,
                vertex_count: 5,
            })
        );

        let two_seams = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 5, 4], [6, 8, 7]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = two_seams.welded_topology_vertices(&[0]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(
            welded.vertices(),
            &[
                point(0.0, -1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
            ]
        );
        assert_eq!(welded.triangles(), &[[2, 1, 3], [2, 1, 0], [2, 4, 3]]);
    }

    #[test]
    fn selected_vertex_welding_ignores_contacts_and_extra_non_manifold_uses() {
        let vertex_contact = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (compacted, edge_count) = vertex_contact.welded_topology_vertices(&[0]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(compacted.vertices(), &vertex_contact.vertices()[..6]);
        assert_eq!(compacted.triangles(), vertex_contact.triangles());

        let non_manifold = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (welded, edge_count) = non_manifold.welded_topology_vertices(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(welded.vertices().len(), 7);
        assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3], [4, 5, 6]]);
    }

    #[test]
    fn unwelds_threshold_edges_in_rhino_radial_order_and_compacts() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[3, 4, 0], [5, 2, 1]]);
        assert_eq!(unwelded.explode_pieces().len(), 2);

        let (smooth, edge_count) = mesh.unwelded_vertices(1.0e-6).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(smooth.vertices(), &mesh.vertices()[..4]);
        assert_eq!(smooth.triangles(), mesh.triangles());

        let (rebuilt, edge_count) = unwelded.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(rebuilt, unwelded);

        let already_unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
            ],
            vec![[0, 1, 2], [3, 4, 5]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (rebuilt, edge_count) = already_unwelded.unwelded_vertices(0.0).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(rebuilt, unwelded);
    }

    #[test]
    fn unweld_uses_an_inclusive_angle_threshold() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 3.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (equal, edge_count) = mesh.unwelded_vertices(std::f64::consts::FRAC_PI_2).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(equal.triangles(), &[[3, 4, 0], [5, 2, 1]]);

        let (above, edge_count) = mesh.unwelded_vertices(90.001_f64.to_radians()).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(above, mesh);
    }

    #[test]
    fn unweld_partitions_a_closed_corner_into_smooth_face_regions() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(1.0, 0.0, 1.0),
            point(1.0, 1.0, 1.0),
            point(0.0, 1.0, 1.0),
        ];
        let mesh = TriangleMesh::try_new(
            vertices.clone(),
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_vertices(std::f64::consts::FRAC_PI_4).unwrap();
        assert_eq!(edge_count, 12);
        assert_eq!(unwelded.vertices().len(), 24);
        assert_eq!(
            unwelded.triangles(),
            &[
                [1, 8, 3],
                [1, 10, 8],
                [13, 16, 19],
                [13, 19, 22],
                [2, 5, 17],
                [2, 17, 12],
                [4, 7, 20],
                [4, 20, 15],
                [6, 9, 23],
                [6, 23, 18],
                [11, 0, 14],
                [11, 14, 21],
            ]
        );
        assert!(
            unwelded
                .vertices()
                .chunks_exact(3)
                .all(|copies| { copies[0] == copies[1] && copies[1] == copies[2] })
        );
        assert_eq!(unwelded.explode_pieces().len(), 6);

        let quad_mesh = TriangleMesh::try_new_faces(
            vertices,
            vec![
                MeshFace::Quad([0, 3, 2, 1]),
                MeshFace::Quad([4, 5, 6, 7]),
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (quad_unwelded, edge_count) = quad_mesh
            .unwelded_vertices(std::f64::consts::FRAC_PI_4)
            .unwrap();
        assert_eq!(edge_count, 12);
        assert_eq!(quad_unwelded.vertices().len(), 24);
        assert!(quad_unwelded.faces().iter().all(|face| face.is_quad()));
        assert_eq!(quad_unwelded.explode_pieces().len(), 6);
    }

    #[test]
    fn unwelds_selected_topology_edges_in_rhino_order_and_compacts() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[2, 5, 0], [4, 3, 1]]);
        assert_eq!(mesh.unwelded_topology_edges(&[0, 0]).unwrap().0, unwelded);

        let (empty, edge_count) = mesh.unwelded_topology_edges(&[]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(empty, mesh);
        let (naked, edge_count) = mesh.unwelded_topology_edges(&[1]).unwrap();
        assert_eq!(edge_count, 0);
        assert_eq!(naked.vertices(), &mesh.vertices()[..4]);
        assert_eq!(naked.triangles(), mesh.triangles());

        assert_eq!(
            mesh.unwelded_topology_edges(&[5]),
            Err(GeometryError::MeshTopologyEdgeIndexOutOfRange {
                edge: 5,
                edge_count: 5,
            })
        );
        let already_unwelded = TriangleMesh::try_new(
            unwelded.vertices().to_vec(),
            unwelded.triangles().to_vec(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_unwelded.unwelded_topology_edges(&[0]).unwrap(),
            (already_unwelded.clone(), 0)
        );
    }

    #[test]
    fn selected_edge_splits_closed_radial_fans_deterministically() {
        let fan = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (one, edge_count) = fan.unwelded_topology_edges(&[0]).unwrap();
        assert_eq!(edge_count, 1);
        assert_eq!(
            one.triangles(),
            &[[4, 0, 5], [4, 1, 0], [4, 2, 1], [3, 6, 2]]
        );
        assert_eq!(one.vertices().len(), 7);

        let (opposite, edge_count) = fan.unwelded_topology_edges(&[0, 2]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(opposite.vertices().len(), 8);
        assert!(topology_edge_is_unwelded_between(
            &opposite,
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0)
        ));
        assert!(topology_edge_is_unwelded_between(
            &opposite,
            point(0.0, 0.0, 0.0),
            point(-1.0, 0.0, 0.0)
        ));
        assert_eq!(opposite.explode_pieces().len(), 2);

        let (adjacent, edge_count) = fan.unwelded_topology_edges(&[0, 1]).unwrap();
        assert_eq!(edge_count, 2);
        assert_eq!(adjacent.vertices().len(), 8);
        assert!(topology_edge_is_unwelded_between(
            &adjacent,
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0)
        ));
        assert!(topology_edge_is_unwelded_between(
            &adjacent,
            point(0.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0)
        ));
        assert_eq!(adjacent.explode_pieces().len(), 2);
    }

    #[test]
    fn selected_edge_loop_detaches_a_cube_face_without_splitting_diagonals() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(1.0, 0.0, 1.0),
                point(1.0, 1.0, 1.0),
                point(0.0, 1.0, 1.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, edge_count) = mesh.unwelded_topology_edges(&[0, 2, 5, 8]).unwrap();
        assert_eq!(edge_count, 4);
        assert_eq!(unwelded.vertices().len(), 12);
        for (first, second) in [
            (point(0.0, 0.0, 0.0), point(1.0, 0.0, 0.0)),
            (point(0.0, 0.0, 0.0), point(0.0, 1.0, 0.0)),
            (point(1.0, 0.0, 0.0), point(1.0, 1.0, 0.0)),
            (point(1.0, 1.0, 0.0), point(0.0, 1.0, 0.0)),
        ] {
            assert!(topology_edge_is_unwelded_between(&unwelded, first, second));
        }
        for (first, second) in [
            (point(0.0, 0.0, 0.0), point(1.0, 1.0, 0.0)),
            (point(0.0, 0.0, 0.0), point(1.0, 0.0, 1.0)),
            (point(1.0, 0.0, 0.0), point(1.0, 1.0, 1.0)),
            (point(1.0, 1.0, 0.0), point(0.0, 1.0, 1.0)),
            (point(0.0, 1.0, 0.0), point(0.0, 0.0, 1.0)),
            (point(0.0, 0.0, 1.0), point(1.0, 1.0, 1.0)),
        ] {
            assert!(!topology_edge_is_unwelded_between(&unwelded, first, second));
        }
        assert_eq!(unwelded.explode_pieces().len(), 2);
    }

    #[test]
    fn unwelds_selected_topology_vertices_in_rhino_order_and_compacts() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 0, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(mesh.topology_vertex_points(), mesh.vertices());

        let (unwelded, vertex_count) = mesh.unwelded_topology_vertices(&[0]).unwrap();
        assert_eq!(vertex_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[4, 0, 1], [0, 3, 2]]);
        assert_eq!(
            mesh.unwelded_topology_vertices(&[0, 0]).unwrap().0,
            unwelded
        );

        let (empty, vertex_count) = mesh.unwelded_topology_vertices(&[]).unwrap();
        assert_eq!((empty, vertex_count), (mesh.clone(), 0));
        let (naked, vertex_count) = mesh.unwelded_topology_vertices(&[2]).unwrap();
        assert_eq!(vertex_count, 0);
        assert_eq!(naked.vertices(), &mesh.vertices()[..4]);
        assert_eq!(naked.triangles(), mesh.triangles());
        assert_eq!(
            mesh.unwelded_topology_vertices(&[5]),
            Err(GeometryError::MeshTopologyVertexIndexOutOfRange {
                vertex: 5,
                vertex_count: 5,
            })
        );

        let already_unwelded = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, -3.0, 0.0),
                point(99.0, 99.0, 99.0),
            ],
            vec![[0, 1, 2], [1, 3, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_unwelded.unwelded_topology_vertices(&[0]).unwrap(),
            (unwelded, 0)
        );
    }

    #[test]
    fn selected_topology_vertex_separates_closed_and_non_manifold_fans() {
        let fan = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(-1.0, 0.0, 0.0),
                point(0.0, -1.0, 0.0),
            ],
            vec![[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, vertex_count) = fan.unwelded_topology_vertices(&[0]).unwrap();
        assert_eq!(vertex_count, 1);
        assert_eq!(unwelded.vertices().len(), 8);
        assert_eq!(
            unwelded.triangles(),
            &[[6, 1, 0], [5, 2, 1], [4, 3, 2], [7, 0, 3]]
        );

        let non_manifold = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (unwelded, vertex_count) = non_manifold.unwelded_topology_vertices(&[0]).unwrap();
        assert_eq!(vertex_count, 1);
        assert_eq!(
            unwelded.vertices(),
            &[
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, -1.0, 0.0),
                point(0.0, 0.0, 1.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(unwelded.triangles(), &[[5, 0, 1], [0, 4, 2], [6, 0, 3]]);
    }

    #[test]
    fn selected_topology_vertex_handles_triangle_and_quad_cube_corners() {
        let vertices = vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(0.0, 0.0, 1.0),
            point(1.0, 0.0, 1.0),
            point(1.0, 1.0, 1.0),
            point(0.0, 1.0, 1.0),
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let mesh = TriangleMesh::try_new(vertices.clone(), triangles, Tolerance::DEFAULT).unwrap();
        let (corner, vertex_count) = mesh.unwelded_topology_vertices(&[0]).unwrap();
        assert_eq!(vertex_count, 1);
        assert_eq!(corner.vertices().len(), 12);
        assert_eq!(
            corner.triangles(),
            &[
                [10, 1, 0],
                [9, 2, 1],
                [3, 4, 5],
                [3, 5, 6],
                [11, 0, 4],
                [7, 4, 3],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 8, 3],
                [2, 3, 6],
            ]
        );

        let all_vertices = (0..mesh.topology().topological_vertex_count()).collect::<Vec<_>>();
        let (all, vertex_count) = mesh.unwelded_topology_vertices(&all_vertices).unwrap();
        assert_eq!(vertex_count, 8);
        assert_eq!(all.vertices().len(), 36);
        assert_eq!(
            all.faces()
                .iter()
                .flat_map(|face| face.indices())
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            36
        );

        let quad_mesh = TriangleMesh::try_new_faces(
            vertices,
            vec![
                MeshFace::Quad([0, 3, 2, 1]),
                MeshFace::Quad([4, 5, 6, 7]),
                MeshFace::Quad([0, 1, 5, 4]),
                MeshFace::Quad([1, 2, 6, 5]),
                MeshFace::Quad([2, 3, 7, 6]),
                MeshFace::Quad([3, 0, 4, 7]),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let (quad_corner, vertex_count) = quad_mesh.unwelded_topology_vertices(&[0]).unwrap();
        assert_eq!(vertex_count, 1);
        assert_eq!(quad_corner.vertices().len(), 10);
        assert!(quad_corner.faces().iter().all(|face| face.is_quad()));
    }

    #[test]
    fn unweld_rejects_invalid_angle_tolerances() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        for angle in [-1.0, std::f64::consts::PI.next_up(), Real::NAN] {
            assert_eq!(
                mesh.unwelded_vertices(angle),
                Err(GeometryError::InvalidMeshUnweldAngle)
            );
        }
    }

    #[test]
    fn culls_only_unused_vertices_in_source_order() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(99.0, 99.0, 99.0),
                point(0.0, 0.0, 0.0),
                point(98.0, 98.0, 98.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(97.0, 97.0, 97.0),
            ],
            vec![[1, 3, 4], [3, 5, 4]],
            Tolerance::DEFAULT,
        )
        .unwrap();

        let (culled, removed) = mesh.culled_unused_vertices();
        assert_eq!(removed, 3);
        assert_eq!(
            culled.vertices(),
            &[
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(culled.triangles(), &[[0, 1, 2], [1, 3, 2]]);

        let already_compact = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(
            already_compact.culled_unused_vertices(),
            (already_compact.clone(), 0)
        );
    }

    #[test]
    fn derived_normals_do_not_depend_on_a_new_model_tolerance() {
        let tolerance = Tolerance::try_new(1.0e-15, 1.0e-15, 1.0e-15).unwrap();
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0e-12, 0.0, 0.0),
                point(0.0, 1.0e-12, 0.0),
            ],
            vec![[0, 1, 2]],
            tolerance,
        )
        .unwrap();

        assert_eq!(mesh.face_normal(0).unwrap().z(), 1.0);
    }

    #[test]
    fn transforms_vertices_and_rejects_collapsed_faces() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let moved = mesh
            .transformed(
                AffineTransform3::from_translation(crate::Vector3::try_new(2.0, 3.0, 4.0).unwrap()),
                Tolerance::DEFAULT,
            )
            .unwrap();
        assert_eq!(moved.vertices()[0], point(2.0, 3.0, 4.0));
        assert_eq!(moved.vertices()[2], point(2.0, 4.0, 4.0));

        let collapsed = AffineTransform3::try_new(
            [[1.0, 0.0, 0.0], [0.0; 3], [0.0; 3]],
            crate::Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        )
        .unwrap();
        assert!(mesh.transformed(collapsed, Tolerance::DEFAULT).is_err());
    }
}
