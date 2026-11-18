//! Ruled walls and shared topology for matching polygon rings.
use super::*;
use std::cmp::Ordering;

#[derive(Clone, Copy)]
struct WallDomains {
    u: [Real; 2],
    v: [Real; 2],
}

fn rim_domain(base: Real, top: Real, size_order: Ordering) -> [Real; 2] {
    match size_order {
        Ordering::Less => [-base, 0.],
        Ordering::Equal => [0., base],
        Ordering::Greater => [0., top],
    }
}

fn reverse_domain([start, end]: [Real; 2]) -> [Real; 2] {
    [-end, -start]
}

impl Brep {
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

        let slant = top[0].distance_to(base[0])?;
        let v = rim_domain(
            base[0].distance_to(base[1])?,
            top[0].distance_to(top[1])?,
            radii[1].total_cmp(&radii[0]),
        );
        let domains = vec![WallDomains { u: [0., slant], v }; side_count];
        Self::truncated_pyramid_loft(frame, &base, &top, domains, solid, height < 0.0, tolerance)
    }

    /// Constructs a joined loft between corresponding polygon rings in
    /// parallel planes. The second ring is a scaled copy of the first and
    /// its center may be offset laterally.
    pub fn try_truncated_pyramid_from_rims(
        frame: Frame3,
        base: &[Point3],
        top: &[Point3],
        scale: Real,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        validate_polygon_sides(base.len())?;
        require_finite([scale], "truncated pyramid scale")?;
        if base.len() != top.len() || scale <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "truncated pyramid rims",
            });
        }
        let slants = base
            .iter()
            .zip(top)
            .map(|(&base, &top)| base.distance_to(top))
            .collect::<Result<Vec<_>, _>>()?;
        let size_order = scale.total_cmp(&1.0);
        let domains = base
            .iter()
            .enumerate()
            .map(|(index, &start)| {
                let next = (index + 1) % base.len();
                let (left, right) = (slants[index], slants[next]);
                let u = if left > right {
                    [-left, 0.]
                } else {
                    [0., right]
                };
                let v = rim_domain(
                    start.distance_to(base[next])?,
                    top[index].distance_to(top[next])?,
                    size_order,
                );
                Ok(WallDomains { u, v })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let height = base[0].vector_to(top[0])?.dot(frame.z_axis().as_vector())?;
        let reverse = solid && (height < 0. || height == 0. && scale < 1.0);
        Self::truncated_pyramid_loft(frame, base, top, domains, solid, reverse, tolerance)
    }

    fn truncated_pyramid_loft(
        frame: Frame3,
        base: &[Point3],
        top: &[Point3],
        domains: Vec<WallDomains>,
        solid: bool,
        reverse: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let side_count = base.len();
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
        let mut edges = Vec::with_capacity(3 * side_count);
        let mut slant_edges = vec![usize::MAX; side_count];
        let mut base_edges = vec![usize::MAX; side_count];
        let mut top_edges = vec![usize::MAX; side_count];
        slant_edges[0] = push_line_edge(
            &mut edges,
            &vertices,
            [top_vertex(0), base_vertex(0)],
            domains[0].u,
        )?;
        for (index, wall) in domains.iter().enumerate() {
            let next = (index + 1) % side_count;
            base_edges[index] = push_line_edge(
                &mut edges,
                &vertices,
                [base_vertex(index), base_vertex(next)],
                wall.v,
            )?;
            if next != 0 {
                slant_edges[next] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [base_vertex(next), top_vertex(next)],
                    reverse_domain(wall.u),
                )?;
                top_edges[index] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [top_vertex(next), top_vertex(index)],
                    reverse_domain(wall.v),
                )?;
            } else {
                top_edges[index] = push_line_edge(
                    &mut edges,
                    &vertices,
                    [top_vertex(0), top_vertex(index)],
                    reverse_domain(wall.v),
                )?;
            }
        }

        let rim_trim_type = if solid {
            BrepTrimType::Mated
        } else {
            BrepTrimType::Boundary
        };
        let mut faces = Vec::with_capacity(side_count + 2 * usize::from(solid));
        for (index, WallDomains { u, v }) in domains.into_iter().enumerate() {
            let next = (index + 1) % side_count;
            let surface = NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![top[index], base[index], top[next], base[next]],
                vec![u[0], u[0], u[1], u[1]],
                vec![v[0], v[0], v[1], v[1]],
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
            faces.push(BrepFace::try_new(surface, reverse, vec![loop_record])?);
        }
        if solid {
            let base_indices = (0..side_count).map(base_vertex).collect::<Vec<_>>();
            let top_indices = (0..side_count).map(top_vertex).collect::<Vec<_>>();
            faces.push(polygon_cap_face(
                fitted_polygon_cap_frame(base, frame.z_axis(), tolerance)?,
                base,
                &base_indices,
                &base_edges,
                false,
                !reverse,
                tolerance,
            )?);
            faces.push(polygon_cap_face(
                fitted_polygon_cap_frame(top, frame.z_axis(), tolerance)?,
                top,
                &top_indices,
                &top_edges,
                true,
                reverse,
                tolerance,
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}
