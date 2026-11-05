//! Hollow cylinder command; parsing is shared with axial primitives.
use super::*;

pub(super) struct TubeCommand(pub use_extrusions::Preference);

impl Command for TubeCommand {
    fn name(&self) -> &'static str {
        "Tube"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = parse_tube_options(arguments)?;
        let tolerance = document.tolerance();
        let (base_frame, first_radius) = axial_primitive_frame(
            options.center,
            options.first_radius,
            options.axis,
            tolerance,
        )?;
        let second_radius = if let Some(radius) = options.second_radius {
            axial_primitive_radius_value(options.center, radius, base_frame)?
        } else {
            first_radius + options.wall_thickness.expect("validated thickness form")
        };
        let direction = if options.height > 0. {
            base_frame.z_axis()
        } else {
            base_frame.z_axis().opposite()
        };
        let start = if options.both_sides {
            options
                .center
                .translated(base_frame.z_axis().as_vector().scaled(-options.height)?)?
        } else {
            options.center
        };
        let frame =
            Frame3::try_from_unit_x_and_normal(start, base_frame.x_axis(), direction, tolerance)?;
        let height = options.height.abs() * if options.both_sides { 2. } else { 1. };
        let tube = Brep::try_tube_extrusion(
            frame,
            [first_radius, second_radius],
            height,
            true,
            tolerance,
        )?;
        let tube = use_extrusions::output(tube, &self.0, tolerance)?;
        let inner_radius = first_radius.min(second_radius);
        let outer_radius = first_radius.max(second_radius);
        let representation = if tube.is_extrusion() {
            "extrusion"
        } else {
            "B-rep"
        };
        let id = document.add_geometry(Geometry::Brep(tube))?;
        Ok(format!(
            "Added closed {representation} tube {id} (inner radius {inner_radius:.6}, outer radius {outer_radius:.6}, height {:.6}{})",
            options.height,
            if options.both_sides {
                ", both sides"
            } else {
                ""
            }
        ))
    }
}
