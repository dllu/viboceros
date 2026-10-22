//! Cylinder construction and its typed extrusion representation.
use super::*;

pub(super) struct CylinderCommand;

impl Command for CylinderCommand {
    fn name(&self) -> &'static str {
        "Cylinder"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Cyl"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut options = parse_axial_primitive_options(arguments, CYLINDER_USAGE, true)?;
        if !arguments.iter().any(|a| {
            a.split_once('=')
                .is_some_and(|(k, _)| option_name_eq(k, "Solid"))
        }) {
            options.solid = true;
        }
        let tolerance = document.tolerance();
        let (frame, radius) =
            axial_primitive_frame(options.center, options.radius, options.axis, tolerance)?;
        let heights = if options.both_sides {
            [-options.height, options.height]
        } else {
            [0., options.height]
        };
        let brep = Brep::try_cylinder_extrusion(frame, radius, heights, options.solid, tolerance)?;
        let id = document.add_geometry(Geometry::Brep(brep))?;
        Ok(format!(
            "Added {} extrusion cylinder {id} (radius {radius:.6}, heights {:.6} to {:.6})",
            if options.solid { "closed" } else { "open" },
            heights[0],
            heights[1]
        ))
    }
}

#[cfg(test)]
mod tests;
