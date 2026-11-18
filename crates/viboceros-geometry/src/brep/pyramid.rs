//! Polygonal apex and loft topology.
use super::*;
use crate::MAX_REGULAR_POLYGON_SIDES;

mod frustum;

impl Brep {
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
        Self::pyramid_from_rim(frame, &base, apex, solid, height < 0.0, tolerance)
    }

    /// Builds a joined pyramid from a planar rim in its construction frame.
    /// The rim is counterclockwise around frame Z and may have concave corners.
    pub fn try_pyramid_from_rim(
        frame: Frame3,
        rim: &[Point3],
        apex: Point3,
        solid: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let height = frame
            .origin()
            .vector_to(apex)?
            .dot(frame.z_axis().as_vector())?;
        validate_polygon_sides(rim.len())?;
        require_finite([height], "pyramid height")?;
        Self::pyramid_from_rim(frame, rim, apex, solid, solid && height < 0.0, tolerance)
    }

    fn pyramid_from_rim(
        frame: Frame3,
        base: &[Point3],
        apex: Point3,
        solid: bool,
        negative_height: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let side_count = base.len();
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
            let (surface, parameters) = triangular_wall(base[index], base[next], apex)?;
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
            faces.push(BrepFace::try_new(
                surface,
                negative_height,
                vec![loop_record],
            )?);
        }
        if solid {
            let indices = (0..side_count).map(base_vertex).collect::<Vec<_>>();
            faces.push(polygon_cap_face(
                frame,
                base,
                &indices,
                &base_edges,
                false,
                !negative_height,
                tolerance,
            )?);
        }
        Self::try_new(vertices, edges, faces, tolerance)
    }
}

/// Center the plane on the triangle centroid, with U along the first edge.
fn triangular_wall(
    first: Point3,
    second: Point3,
    apex: Point3,
) -> Result<(NurbsSurface, [Point2; 3]), GeometryError> {
    let edge = first.vector_to(second)?;
    let x_axis = edge.normalized_nonzero()?;
    let length = edge.length()?;
    let to_apex = first.vector_to(apex)?;
    let along = to_apex.dot(x_axis.as_vector())?;
    let across = apex.distance_to(first.translated(x_axis.as_vector().scaled(along)?)?)?;
    let center_u = (length + along) / 3.0;
    let u = [along.min(0.0), along.max(length)];
    let v = [-across / 3.0, 2.0 * across / 3.0];
    let surface = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            first.translated(x_axis.as_vector().scaled(u[0])?)?,
            first.translated(x_axis.as_vector().scaled(u[1])?)?,
            apex.translated(x_axis.as_vector().scaled(u[0] - along)?)?,
            apex.translated(x_axis.as_vector().scaled(u[1] - along)?)?,
        ],
        vec![
            u[0] - center_u,
            u[0] - center_u,
            u[1] - center_u,
            u[1] - center_u,
        ],
        vec![v[0], v[0], v[1], v[1]],
    )?;
    Ok((
        surface,
        [
            Point2::try_new(-center_u, v[0])?,
            Point2::try_new(length - center_u, v[0])?,
            Point2::try_new(along - center_u, v[1])?,
        ],
    ))
}

fn validate_pyramid_dimensions<const N: usize>(
    side_count: usize,
    radii: [Real; N],
    height: Real,
    context: &'static str,
) -> Result<(), GeometryError> {
    require_finite(radii.into_iter().chain(std::iter::once(height)), context)?;
    validate_polygon_sides(side_count)?;
    if radii.into_iter().any(|radius| radius <= 0.0) || height == 0.0 {
        return Err(GeometryError::Degenerate { context });
    }
    Ok(())
}

fn validate_polygon_sides(side_count: usize) -> Result<(), GeometryError> {
    if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
        return Err(GeometryError::InvalidRegularPolygonSides {
            actual: side_count,
            maximum: MAX_REGULAR_POLYGON_SIDES,
        });
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
    Frame3::try_from_unit_x_and_normal(
        origin,
        points[0].vector_to(points[1])?.normalized(tolerance)?,
        normal,
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
