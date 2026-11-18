//! Surface and topology construction for Tube's explicit boundary faces.
use super::*;

impl Brep {
    /// Two coaxial walls and two trimmed annuli, with the caps first.
    pub fn try_tube_polysurface(
        frame: Frame3,
        radii: [Real; 2],
        height: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let radii = [radii[0].max(radii[1]), radii[0].min(radii[1])];
        let [outer, inner] =
            radii.map(|radius| NurbsSurface::try_cylinder(frame, radius, 0., height));
        let (outer, inner) = (outer?, inner?);
        let outer_ids = [0, 2];
        let inner_ids = [1, 3];
        let mut edges = Vec::with_capacity(6);
        let outer_face = tube_wall(&outer, outer_ids, BrepTrimType::Mated, false, &mut edges)?;
        let inner_face = tube_wall(&inner, inner_ids, BrepTrimType::Mated, true, &mut edges)?;
        // The seam vertices are ordered by cap, with the outer rim first.
        let vertices = [0, 3, 2, 5]
            .into_iter()
            .map(|edge| BrepVertex::try_new(edges[edge].curve().evaluate(0.)?, 0.))
            .collect::<Result<Vec<_>, _>>()?;
        let make_cap = |index, reversed| {
            tube_cap(
                frame,
                [
                    (outer_ids[index], 2 * index, radii[0]),
                    (inner_ids[index], 2 * index + 3, radii[1]),
                ]
                .as_slice(),
                &edges,
                reversed,
                tolerance,
            )
        };
        let faces = vec![
            make_cap(0, true)?,
            make_cap(1, false)?,
            outer_face,
            inner_face,
        ];
        Self::try_new(vertices, edges, faces, tolerance)
    }

    /// An uncapped Tube wall retains its angular U interval in polysurface mode.
    pub fn try_tube_wall_face(
        frame: Frame3,
        radius: Real,
        height: Real,
        inward: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let wall = NurbsSurface::try_cylinder(frame, radius, 0., height)?;
        let mut edges = Vec::with_capacity(3);
        let face = tube_wall(&wall, [0, 1], BrepTrimType::Boundary, inward, &mut edges)?;
        let vertices = wall_vertices(&wall)?;
        Self::try_new(vertices, edges, vec![face], tolerance)
    }

    /// A coincident wall pair leaves separate circular boundaries. Capped
    /// mode combines the two walls and four disk faces in a single B-rep.
    pub fn try_coincident_tube_faces(
        frame: Frame3,
        radius: Real,
        height: Real,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        let wall = NurbsSurface::try_cylinder(frame, radius, 0., height)?;
        let make_disk = |row| {
            let ring = surface_u_control_curve(&wall, row)?
                .try_reparameterized(0.0..=std::f64::consts::TAU * radius)?;
            let vertex = BrepVertex::try_new(ring.evaluate(0.)?, 0.)?;
            let edges = vec![BrepEdge::try_new([0, 0], ring, 0.)?];
            let face = tube_cap(frame, &[(0, 0, radius)], &edges, !solid, tolerance)?;
            Self::try_new(vec![vertex], edges, vec![face], tolerance)
        };
        let start = make_disk(0)?;
        let end = make_disk(1)?;
        let mut parts = vec![start.clone(), start, end.clone(), end];
        if !solid {
            return Ok(parts);
        }
        let mut edges = vec![];
        let wall_face = tube_wall(&wall, [0, 1], BrepTrimType::Boundary, false, &mut edges)?;
        // The coincident fallback creates both walls with reversed north
        // curves and the matching forward trim uses.
        edges[2].curve = edges[2].curve.reversed()?;
        let mut wall_face = wall_face;
        wall_face.loops[0].trims[2].reversed_3d = false;
        let part = Self::try_new(wall_vertices(&wall)?, edges, vec![wall_face], tolerance)?;
        parts.extend([part.clone(), part]);
        Ok(vec![
            Self::try_combine(parts, tolerance)?.try_weld_coincident_vertices(tolerance)?,
        ])
    }
}

fn wall_vertices(wall: &NurbsSurface) -> Result<Vec<BrepVertex>, GeometryError> {
    let v = wall.domain_v();
    [*v.start(), *v.end()]
        .into_iter()
        .map(|height| BrepVertex::try_new(wall.evaluate(0., height)?, 0.))
        .collect()
}

fn tube_wall(
    wall: &NurbsSurface,
    [start, end]: [usize; 2],
    rim: BrepTrimType,
    reversed: bool,
    edges: &mut Vec<BrepEdge>,
) -> Result<BrepFace, GeometryError> {
    let first = edges.len();
    edges.extend([
        BrepEdge::try_new([start, start], surface_u_control_curve(wall, 0)?, 0.)?,
        BrepEdge::try_new([start, end], surface_v_control_curve(wall, 0)?, 0.)?,
        BrepEdge::try_new([end, end], surface_u_control_curve(wall, 1)?, 0.)?,
    ]);
    let loop_record = rectangular_surface_loop(
        wall,
        [
            RectangularTrimSpec::edge([start, start], first, false, rim),
            RectangularTrimSpec::edge([start, end], first + 1, false, BrepTrimType::Seam),
            RectangularTrimSpec::edge([end, end], first + 2, true, rim),
            RectangularTrimSpec::edge([end, start], first + 1, true, BrepTrimType::Seam),
        ],
    )?;
    BrepFace::try_new(wall.clone(), reversed, vec![loop_record])
}

fn tube_cap(
    frame: Frame3,
    rings: &[(usize, usize, Real)],
    edges: &[BrepEdge],
    reversed: bool,
    tolerance: Tolerance,
) -> Result<BrepFace, GeometryError> {
    let origin = edges[rings[0].1].curve().evaluate(0.)?;
    let normal = frame.z_axis().as_vector();
    let reference = if normal.x().abs() < 1. - 1e-12 {
        Vector3::try_new(1., 0., 0.)?
    } else {
        Vector3::try_new(0., 1., 0.)?
    };
    let x = normal
        .cross(reference)?
        .cross(normal)?
        .normalized_nonzero()?;
    let plane = Frame3::try_from_unit_x_and_normal(origin, x, frame.z_axis(), tolerance)?;
    let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
    let mut loops = Vec::with_capacity(rings.len());
    for (index, &(vertex, edge, radius)) in rings.iter().enumerate() {
        let curve = edges[edge]
            .curve()
            .try_reparameterized(0.0..=std::f64::consts::TAU * radius)?;
        let (trim, residual) = project_curve_to_frame(&curve, plane, tolerance)?;
        let inner = index != 0;
        if !inner {
            for control in trim.control_points() {
                let p = control.point();
                for (axis, value) in bounds.iter_mut().zip([p.x(), p.y()]) {
                    axis[0] = axis[0].min(value);
                    axis[1] = axis[1].max(value);
                }
            }
        }
        loops.push(single_edge_loop(
            vertex,
            edge,
            if inner {
                BrepLoopType::Inner
            } else {
                BrepLoopType::Outer
            },
            if inner { trim.reversed()? } else { trim },
            inner,
            if rings.len() == 1 {
                BrepTrimType::Boundary
            } else {
                BrepTrimType::Mated
            },
            [residual, residual],
        )?);
    }
    for interval in &mut bounds {
        let padding = ((interval[1] - interval[0]) * 0.01).max(0.01);
        interval[0] -= padding;
        interval[1] += padding;
    }
    BrepFace::try_new(
        planar_cap_surface(plane, Vector3::try_new(0., 0., 0.)?, bounds)?,
        reversed,
        loops,
    )
}
