use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, LineSegment, NurbsCurve, NurbsCurve2,
    NurbsSurface, Point2, Point3, Real, Tolerance, require_finite,
};

const LOOP_SAMPLES_PER_SPAN: usize = 4;

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
}
