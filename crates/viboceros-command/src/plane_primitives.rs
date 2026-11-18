//! Primitives whose default orientation comes from the command context.

use super::*;
use crate::circle_input::{CircleSizeMode, PointConstruction, diameter_prefix, vertical_size};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct CircleCommand(
    crate::remembered::Remembered<crate::circle_around_input::Preferences>,
);

impl CircleCommand {
    fn deformable_run(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
        postselected: bool,
    ) -> Result<String, CommandError> {
        use crate::circle_deform_input::{Construction, from_arguments};
        let (output, construction) = from_arguments(
            document,
            arguments,
            context.construction_plane,
            self.0.get(),
        )?;
        let (circle, preferences) = match construction {
            Construction::Circle {
                circle,
                preferences,
            } => (circle, preferences),
            Construction::Around(d) => {
                if d.edge.is_none() {
                    if d.postselected {
                        document.release_command_selection_on_history_replay([d.source])?;
                        document.select_command_results([])?;
                    } else {
                        document.select_command_results([d.source])?;
                    }
                }
                (d.circle, d.preferences)
            }
            Construction::FitPoints => {
                let result =
                    circle_fit_points::run_with_output(document, postselected, Some(output))?;
                let mut preferences = self.0.get();
                preferences.deformation = output.deformation;
                self.0.set(preferences);
                return Ok(result);
            }
            Construction::Tangent(d) => (d.circle, d.preferences),
        };
        let id = document.add_geometry(output.geometry(circle)?)?;
        self.0.set(preferences);
        Ok(format!("Added circle {id} (radius {:.6})", circle.radius()))
    }
}

fn circle_size_argument<'a>(arguments: &[&'a str]) -> Option<(CircleSizeMode, &'a str)> {
    match arguments {
        [single] if let Some((name, value)) = single.split_once('=') => {
            CircleSizeMode::parse(name).map(|mode| (mode, value))
        }
        [name, value] => CircleSizeMode::parse(name).map(|mode| (mode, *value)),
        [single] if !single.contains(',') => Some((CircleSizeMode::Radius, *single)),
        _ => None,
    }
}

fn circle_numeric_radius(arguments: &[&str], remembered: f64) -> Result<Option<f64>, CommandError> {
    circle_size_argument(arguments)
        .map(|(mode, value)| {
            if option_name_eq(value, "Enter")
                && matches!(mode, CircleSizeMode::Radius | CircleSizeMode::Diameter)
            {
                Ok(remembered)
            } else {
                parse_finite_real(value).map(|value| mode.radius(value))
            }
        })
        .transpose()
}

fn vertical_circle(
    arguments: &[&str],
    plane: Frame3,
    tolerance: Tolerance,
    remembered: f64,
) -> Result<Circle3, CommandError> {
    let (center, count) = parse_point(arguments)?;
    let remaining = &arguments[count..];
    let measured = |mode| matches!(mode, CircleSizeMode::Circumference | CircleSizeMode::Area);
    if let Some((mode, value)) = circle_size_argument(remaining)
        && measured(mode)
        && !value.contains(',')
    {
        return Ok(vertical_size(
            center,
            mode.radius(parse_finite_real(value)?),
            plane,
            tolerance,
        )?);
    }
    if let Some((mode, point)) = remaining.split_first().and_then(|(token, rest)| {
        let mode = CircleSizeMode::parse(token).filter(|mode| measured(*mode))?;
        let (point, count) = parse_point(rest).ok()?;
        (count == rest.len()).then_some((mode, point))
    }) {
        return Ok(PointConstruction::Vertical {
            center,
            radius: None,
            mode,
        }
        .circle(point, plane, tolerance)?);
    }
    if let Ok((point, count)) = parse_point(remaining)
        && count == remaining.len()
    {
        return Ok(PointConstruction::Vertical {
            center,
            radius: None,
            mode: CircleSizeMode::Radius,
        }
        .circle(point, plane, tolerance)?);
    }
    for size_tokens in [1, 2] {
        if remaining.len() <= size_tokens
            || size_tokens == 1 && CircleSizeMode::parse(remaining[0]).is_some()
        {
            continue;
        }
        if let Some(radius) = circle_numeric_radius(&remaining[..size_tokens], remembered)?
            && let Ok((point, count)) = parse_point(&remaining[size_tokens..])
            && size_tokens + count == remaining.len()
        {
            return Ok(PointConstruction::Vertical {
                center,
                radius: Some(radius),
                mode: CircleSizeMode::Radius,
            }
            .circle(point, plane, tolerance)?);
        }
    }
    Err(CommandError::Usage(
        "Circle Vertical center point | center radius direction-point | center Circumference=value | center Area=value",
    ))
}

impl Command for CircleCommand {
    fn circle_preferences(&self) -> Option<crate::circle_around_input::Preferences> {
        Some(self.0.get())
    }
    fn remember_circle_diameter(&self, diameter: bool) {
        self.0.set(crate::circle_around_input::Preferences {
            diameter,
            ..self.0.get()
        });
    }
    fn name(&self) -> &'static str {
        "Circle"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["C"]
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if let Some((_, consumed)) =
            crate::circle_deform_input::output_prefix(arguments, self.0.get())?
        {
            return Ok(
                matches!(&arguments[consumed..], [w] if option_name_eq(w,"FitPoints"))
                    .then(circle_fit_points::selection_prompt),
            );
        }
        Ok(
            matches!(arguments, [option] if option_name_eq(option, "FitPoints"))
                .then(circle_fit_points::selection_prompt),
        )
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if crate::circle_deform_input::is_deformable_start(arguments) {
            return self.deformable_run(document, arguments, context, true);
        }
        if arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "FitPoints"))
        {
            require_consumed(
                arguments,
                1,
                "Circle FitPoints (select points or control points)",
            )?;
            return circle_fit_points::run(document, true);
        }
        self.run_in_context(document, arguments, context)
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if crate::circle_deform_input::is_deformable_start(arguments) {
            return self.deformable_run(document, arguments, context, false);
        }
        if arguments
            .first()
            .is_some_and(|w| option_name_eq(w, "Tangent"))
        {
            let d = crate::circle_tangent_input::from_arguments(
                document,
                &arguments[1..],
                context.construction_plane,
                self.0.get(),
            )?;
            let id = document.add_geometry(Geometry::Circle(d.circle))?;
            self.0.set(d.preferences);
            return Ok(format!(
                "Added circle {id} (radius {:.6})",
                d.circle.radius()
            ));
        }
        if arguments
            .first()
            .is_some_and(|w| option_name_eq(w, "AroundCurve"))
        {
            let d = crate::circle_around_input::from_arguments(
                document,
                &arguments[1..],
                self.0.get(),
            )?;
            if d.edge.is_none() {
                if d.postselected {
                    document.release_command_selection_on_history_replay([d.source])?;
                    document.select_command_results([])?;
                } else {
                    document.select_command_results([d.source])?;
                }
            }
            let id = document.add_geometry(Geometry::Circle(d.circle))?;
            self.0.set(d.preferences);
            return Ok(format!(
                "Added circle {id} (radius {:.6})",
                d.circle.radius()
            ));
        }
        if arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "FitPoints"))
        {
            require_consumed(
                arguments,
                1,
                "Circle FitPoints (select points or control points)",
            )?;
            return circle_fit_points::run(document, false);
        }
        let plane = context.construction_plane;
        let remembered = self.0.get().radius;
        let diameter = diameter_prefix(arguments);
        let circle = if let Some((vertical, count)) = diameter {
            let points = &arguments[count..];
            let (first, first_count) = parse_point(points)?;
            let (second, second_count) = parse_point(&points[first_count..])?;
            require_consumed(
                points,
                first_count + second_count,
                "Circle 2Point [Vertical] diameter_start diameter_end",
            )?;
            PointConstruction::Diameter { first, vertical }.circle(
                second,
                plane,
                document.tolerance(),
            )?
        } else if arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "Vertical"))
        {
            vertical_circle(&arguments[1..], plane, document.tolerance(), remembered)?
        } else if arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "Orientation"))
        {
            let arguments = &arguments[1..];
            let (center, center_count) = parse_point(arguments)?;
            let (normal_point, normal_count) = parse_point(&arguments[center_count..])?;
            let remaining = &arguments[center_count + normal_count..];
            if let Some(radius) = circle_numeric_radius(remaining, remembered)? {
                let frame = Frame3::try_from_normal(
                    center,
                    center.vector_to(normal_point)?,
                    document.tolerance(),
                )?;
                Circle3::try_from_frame(
                    center,
                    radius,
                    frame.x_axis(),
                    frame.z_axis(),
                    document.tolerance(),
                )?
            } else {
                let (point, count) = parse_point(remaining)?;
                require_consumed(
                    remaining,
                    count,
                    "Circle Orientation center normal-point radius | radius-point",
                )?;
                PointConstruction::Oriented {
                    center,
                    normal_point,
                    mode: CircleSizeMode::Radius,
                }
                .circle(point, plane, document.tolerance())?
            }
        } else if arguments
            .first()
            .is_some_and(|option| option_name_eq(option, "3Point"))
        {
            let arguments = &arguments[1..];
            let (first, first_count) = parse_point(arguments)?;
            let (second, second_count) = parse_point(&arguments[first_count..])?;
            let remaining = &arguments[first_count + second_count..];
            let radius = remaining.first().and_then(|option| {
                if option_name_eq(option, "Radius") {
                    Some((1, remaining.get(1).copied()))
                } else if let Some((name, value)) = option.split_once('=')
                    && option_name_eq(name, "Radius")
                {
                    Some((0, Some(value)))
                } else {
                    None
                }
            });
            if let Some((extra, value)) = radius {
                if extra == 1
                    && let Ok((radius_point, count)) = parse_point(&remaining[1..])
                    && count + 1 == remaining.len()
                {
                    PointConstruction::ThreePointRadius {
                        first,
                        second,
                        radius: None,
                    }
                    .circle(radius_point, plane, document.tolerance())?
                } else {
                    let value = value.ok_or(CommandError::Usage(
                        "Circle 3Point first second Radius radius direction-point",
                    ))?;
                    let radius = if option_name_eq(value, "Enter") {
                        remembered
                    } else {
                        parse_finite_real(value)?
                    };
                    let direction_arguments = &remaining[1 + extra..];
                    let (direction_point, count) = parse_point(direction_arguments)?;
                    require_consumed(
                        direction_arguments,
                        count,
                        "Circle 3Point first second Radius radius direction-point",
                    )?;
                    PointConstruction::ThreePointRadius {
                        first,
                        second,
                        radius: Some(radius),
                    }
                    .circle(direction_point, plane, document.tolerance())?
                }
            } else {
                let (third, third_count) = parse_point(remaining)?;
                require_consumed(remaining, third_count, "Circle 3Point first second third")?;
                PointConstruction::ThreePoint { first, second }.circle(
                    third,
                    plane,
                    document.tolerance(),
                )?
            }
        } else {
            let (center, consumed) = parse_point(arguments)?;
            let remaining = &arguments[consumed..];
            let normal = plane.z_axis();
            if let Some(radius) = circle_numeric_radius(remaining, remembered)? {
                Circle3::try_from_frame(
                    center,
                    radius,
                    plane.x_axis(),
                    normal,
                    document.tolerance(),
                )?
            } else {
                let (point_on_circle, point_consumed) = parse_point(remaining)?;
                require_consumed(
                    remaining,
                    point_consumed,
                    "Circle center radius | center point-on-circle",
                )?;
                PointConstruction::Center {
                    center,
                    mode: CircleSizeMode::Radius,
                }
                .circle(point_on_circle, plane, document.tolerance())?
            }
        };
        let radius = circle.radius();
        let id = document.add_geometry(Geometry::Circle(circle))?;
        let mut preferences = self.0.get();
        let point_construction = diameter.is_some()
            || arguments
                .first()
                .is_some_and(|w| option_name_eq(w, "3Point"));
        let radius_option = arguments
            .iter()
            .any(|w| option_name_eq(w.split_once('=').map_or(*w, |(k, _)| k), "Radius"));
        // Diameter endpoints and plain three-point constructions do not use
        // the radius getter; the three-point Radius option does.
        if !point_construction || radius_option {
            preferences.radius = radius;
        }
        if !point_construction {
            for word in arguments {
                match CircleSizeMode::parse(word.split_once('=').map_or(*word, |(k, _)| k)) {
                    Some(CircleSizeMode::Radius) => preferences.diameter = false,
                    Some(CircleSizeMode::Diameter) => preferences.diameter = true,
                    _ => {}
                }
            }
        }
        self.0.set(preferences);
        Ok(format!("Added circle {id} (radius {radius:.6})"))
    }
}

#[derive(Default)]
pub(super) struct RectangleCommand(
    crate::remembered::Remembered<crate::rectangle_rounding::Options>,
);

impl Command for RectangleCommand {
    fn rectangle_options(&self) -> Option<crate::rectangle_rounding::Options> {
        Some(self.0.get())
    }
    fn remember_rectangle_options(&self, value: crate::rectangle_rounding::Options) -> bool {
        self.0.set(value);
        true
    }

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
        let (definition, options) = crate::rectangle_input::from_arguments_with_options(
            document,
            arguments,
            context.construction_plane,
            self.0.get(),
        )?;
        let geometry = definition.geometry(document.tolerance())?;
        let width = definition.corners[0].distance_to(definition.corners[1])?;
        let height = definition.corners[1].distance_to(definition.corners[2])?;
        let id = document.add_geometry(geometry)?;
        self.0.set(options);
        Ok(format!("Added rectangle {id} ({width:.6} × {height:.6})"))
    }
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
        if arguments
            .iter()
            .any(|argument| box_input::is_extended(argument))
            || matches!(arguments, [count, first, ..]
                if count.parse::<usize>().is_ok_and(|n| n > 0) && first.contains(','))
        {
            let scripted = box_input::from_arguments(document, arguments, plane, Some([10; 3]))?;
            let counts = scripted.counts.unwrap();
            let geometry = box_input::mesh(
                scripted.definition,
                counts,
                scripted.ordered_height,
                document.tolerance(),
            )?;
            let id = document.add_geometry(Geometry::Mesh(geometry))?;
            let [x, y, z] = counts;
            return Ok(format!(
                "Added closed mesh box {id} ({x} × {y} × {z} side divisions)"
            ));
        }
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

pub(super) struct BoxCommand(pub use_extrusions::Preference);

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
        if arguments
            .iter()
            .any(|argument| box_input::is_extended(argument))
        {
            let scripted = box_input::from_arguments(document, arguments, plane, None)?;
            let extrusion = self.0.get().0;
            let geometry = box_input::solid(scripted.definition, extrusion, document.tolerance())?;
            let id = document.add_geometry(Geometry::Brep(geometry))?;
            let kind = if extrusion { "extrusion" } else { "B-rep" };
            return Ok(format!("Added closed {kind} box {id}"));
        }
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
        let brep = Brep::try_box_extrusion(
            frame,
            [
                increasing_interval(base_delta[0]),
                increasing_interval(base_delta[1]),
            ],
            height,
            document.tolerance(),
        )?;
        let brep = use_extrusions::output(brep, &self.0, document.tolerance())?;
        let id = document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Added closed {} box {id}",
            if self.0.get().0 { "extrusion" } else { "B-rep" }
        ))
    }
}
