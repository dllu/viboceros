//! Primitives whose default orientation comes from the command context.

use super::*;

#[cfg(test)]
mod tests;

pub(super) struct CircleCommand;

impl Command for CircleCommand {
    fn name(&self) -> &'static str {
        "Circle"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["C"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let (center, consumed) = parse_point(arguments)?;
        let remaining = &arguments[consumed..];
        let normal = plane.z_axis();
        let circle = if remaining.len() == 1 && !remaining[0].contains(',') {
            let radius = parse_finite_real(remaining[0])?;
            Circle3::try_from_frame(center, radius, plane.x_axis(), normal, document.tolerance())?
        } else {
            let (point_on_circle, point_consumed) = parse_point(remaining)?;
            require_consumed(
                remaining,
                point_consumed,
                "Circle center radius | center point-on-circle",
            )?;
            let delta = center.vector_to(point_on_circle)?;
            let frame = Frame3::try_from_x_and_normal(
                center,
                delta,
                normal.as_vector(),
                document.tolerance(),
            )
            .or_else(|_| {
                Frame3::try_from_directions(
                    center,
                    delta,
                    plane.y_axis().as_vector(),
                    document.tolerance(),
                )
            })?;
            Circle3::try_from_frame(
                center,
                delta.length()?,
                frame.x_axis(),
                frame.z_axis(),
                document.tolerance(),
            )?
        };
        let radius = circle.radius();
        let id = document.add_geometry(Geometry::Circle(circle))?;
        Ok(format!("Added circle {id} (radius {radius:.6})"))
    }
}

pub(super) struct RectangleCommand;

impl Command for RectangleCommand {
    fn name(&self) -> &'static str {
        "Rectangle"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Rect"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let (first, first_consumed) = parse_point(arguments)?;
        let (opposite, opposite_consumed) = parse_point(&arguments[first_consumed..])?;
        require_consumed(
            arguments,
            first_consumed + opposite_consumed,
            "Rectangle first-corner opposite-corner",
        )?;
        let polyline = plane_rectangle(plane, first, opposite, document.tolerance())?;
        let width = polyline.vertices()[0].distance_to(polyline.vertices()[1])?;
        let height = polyline.vertices()[1].distance_to(polyline.vertices()[2])?;
        let id = document.add_geometry(Geometry::Polyline(polyline))?;
        Ok(format!("Added rectangle {id} ({width:.6} × {height:.6})"))
    }
}

fn plane_rectangle(
    plane: Frame3,
    first: Point3,
    opposite: Point3,
    tolerance: Tolerance,
) -> Result<Polyline3, GeometryError> {
    let plane = plane.with_origin(first);
    let [x, y, _] = plane.coordinates_of(opposite)?;
    let [x0, x1] = [x.min(0.0), x.max(0.0)];
    let [y0, y1] = [y.min(0.0), y.max(0.0)];
    let vertices = [
        [x0, y0, 0.0],
        [x1, y0, 0.0],
        [x1, y1, 0.0],
        [x0, y1, 0.0],
        [x0, y0, 0.0],
    ]
    .into_iter()
    .map(|p| plane.point_at(p))
    .collect::<Result<Vec<_>, _>>()?;
    Polyline3::try_new(vertices, tolerance)?.try_chord_length_parameterized()
}

pub(super) struct PolygonCommand;

impl Command for PolygonCommand {
    fn name(&self) -> &'static str {
        "Polygon"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Poly"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let side_text = arguments.first().ok_or(CommandError::Usage(
            "Polygon sides center radius | sides center first-vertex",
        ))?;
        let side_count = side_text
            .parse::<usize>()
            .map_err(|_| CommandError::InvalidInteger((*side_text).to_owned()))?;
        if !(3..=MAX_REGULAR_POLYGON_SIDES).contains(&side_count) {
            return Err(GeometryError::InvalidRegularPolygonSides {
                actual: side_count,
                maximum: MAX_REGULAR_POLYGON_SIDES,
            }
            .into());
        }
        let (center, center_consumed) = parse_point(&arguments[1..])?;
        let remaining = &arguments[1 + center_consumed..];
        let normal = plane.z_axis();
        let first_vertex = if remaining.len() == 1 && !remaining[0].contains(',') {
            let radius = parse_finite_real(remaining[0])?;
            Circle3::try_from_frame(center, radius, plane.x_axis(), normal, document.tolerance())?
                .point_at_angle(0.0)?
        } else {
            let (first_vertex, consumed) = parse_point(remaining)?;
            require_consumed(
                remaining,
                consumed,
                "Polygon sides center radius | sides center first-vertex",
            )?;
            first_vertex
        };
        let normal = Frame3::try_from_x_and_normal(
            center,
            center.vector_to(first_vertex)?,
            normal.as_vector(),
            document.tolerance(),
        )?
        .z_axis();
        let polygon = Polyline3::try_regular_polygon(
            side_count,
            center,
            first_vertex,
            normal,
            document.tolerance(),
        )?
        .try_chord_length_parameterized()?;
        let perimeter = polygon.length()?;
        let id = document.add_geometry(Geometry::Polyline(polygon))?;
        Ok(format!(
            "Added {side_count}-sided polygon {id} (perimeter {perimeter:.6})"
        ))
    }
}

pub(super) struct MeshPlaneCommand;

impl Command for MeshPlaneCommand {
    fn name(&self) -> &'static str {
        "MeshPlane"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let leading_options = mesh_plane_leading_option_count(arguments);
        let (first, opposite, options) = if leading_options == 0 {
            let (first, first_consumed) = parse_point(arguments)?;
            let (opposite, opposite_consumed) = parse_point(&arguments[first_consumed..])?;
            let options =
                parse_mesh_plane_arguments(&arguments[first_consumed + opposite_consumed..])?;
            (first, opposite, options)
        } else {
            let options = parse_mesh_plane_arguments(&arguments[..leading_options])?;
            let (first, first_consumed) = parse_point(&arguments[leading_options..])?;
            let point_start = leading_options + first_consumed;
            let (opposite, opposite_consumed) = parse_point(&arguments[point_start..])?;
            require_consumed(arguments, point_start + opposite_consumed, MESH_PLANE_USAGE)?;
            (first, opposite, options)
        };
        let frame = plane.with_origin(first);
        let [x, y, _] = frame.coordinates_of(opposite)?;
        let mesh = TriangleMesh::try_plane_grid(
            frame,
            [x.min(0.0), x.max(0.0)],
            [y.min(0.0), y.max(0.0)],
            options.x_count,
            options.y_count,
            document.tolerance(),
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added mesh plane {id} ({} × {} faces)",
            options.x_count, options.y_count
        ))
    }
}

pub(super) struct MeshBoxCommand;

impl Command for MeshBoxCommand {
    fn name(&self) -> &'static str {
        "MeshBox"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let leading_options = mesh_box_leading_option_count(arguments);
        let (base_corner, opposite_corner, height, options) = if leading_options == 0 {
            let (base_corner, base_consumed) = parse_point(arguments)?;
            let (opposite_corner, opposite_consumed) = parse_point(&arguments[base_consumed..])?;
            let height_start = base_consumed + opposite_consumed;
            let (height, height_consumed) =
                parse_mesh_box_height(&arguments[height_start..], base_corner, plane.z_axis())?;
            let options = parse_mesh_box_arguments(&arguments[height_start + height_consumed..])?;
            (base_corner, opposite_corner, height, options)
        } else {
            let options = parse_mesh_box_arguments(&arguments[..leading_options])?;
            let (base_corner, base_consumed) = parse_point(&arguments[leading_options..])?;
            let opposite_start = leading_options + base_consumed;
            let (opposite_corner, opposite_consumed) = parse_point(&arguments[opposite_start..])?;
            let height_start = opposite_start + opposite_consumed;
            let (height, height_consumed) =
                parse_mesh_box_height(&arguments[height_start..], base_corner, plane.z_axis())?;
            require_consumed(arguments, height_start + height_consumed, MESH_BOX_USAGE)?;
            (base_corner, opposite_corner, height, options)
        };
        // The command's mesh grids retain the base first. Rhino uses an
        // oppositely oriented Y/Z frame for a positive extrusion direction.
        let frame = if height > 0.0 {
            Frame3::try_from_directions(
                base_corner,
                plane.x_axis().as_vector(),
                plane.y_axis().as_vector().scaled(-1.0)?,
                document.tolerance(),
            )?
        } else {
            plane.with_origin(base_corner)
        };
        let base_delta = frame.coordinates_of(opposite_corner)?;
        let height = -height.abs();
        let increasing_interval = |value: Real| [value.min(0.0), value.max(0.0)];
        let mesh = TriangleMesh::try_box_grid(
            frame,
            [
                increasing_interval(base_delta[0]),
                increasing_interval(base_delta[1]),
                [0.0, height],
            ],
            options.x_count,
            options.y_count,
            options.z_count,
            document.tolerance(),
        )?;
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        Ok(format!(
            "Added closed mesh box {id} ({} × {} × {} side divisions)",
            options.x_count, options.y_count, options.z_count
        ))
    }
}

pub(super) struct BoxCommand;

impl Command for BoxCommand {
    fn name(&self) -> &'static str {
        "Box"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let (base_corner, base_consumed) = parse_point(arguments)?;
        let (opposite_corner, opposite_consumed) = parse_point(&arguments[base_consumed..])?;
        let option_start = base_consumed + opposite_consumed;
        let remaining = &arguments[option_start..];
        let height = if remaining.len() == 1 && !remaining[0].contains(',') {
            parse_finite_real(remaining[0])?
        } else {
            let (height_point, height_consumed) = parse_point(remaining)?;
            require_consumed(remaining, height_consumed, BOX_USAGE)?;
            base_corner
                .vector_to(height_point)?
                .dot(plane.z_axis().as_vector())?
        };
        let frame = plane.with_origin(base_corner);
        let base_delta = frame.coordinates_of(opposite_corner)?;
        let increasing_interval = |value: Real| [value.min(0.0), value.max(0.0)];
        let brep = Brep::try_box(
            frame,
            [
                increasing_interval(base_delta[0]),
                increasing_interval(base_delta[1]),
                increasing_interval(height),
            ],
            document.tolerance(),
        )?;
        let id = document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!("Added closed B-rep box {id}"))
    }
}
