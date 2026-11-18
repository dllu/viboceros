//! Staged Tube execution and the explicit-axis positional form.
use super::*;
use solid_axial::{Definition, Kind, MeshTopology, TubeSettings};
use std::sync::Arc;

pub(super) struct TubeCommand {
    pub extrusions: use_extrusions::Preference,
    pub settings: Arc<remembered::Remembered<TubeSettings>>,
}

impl Command for TubeCommand {
    fn name(&self) -> &'static str {
        Kind::Tube.name()
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
        let tolerance = document.tolerance();
        let (definition, preferences) = if let Ok(options) = parse_tube_options(arguments) {
            (positional_definition(options, tolerance)?, None)
        } else {
            let input = solid_axial::from_arguments(
                document,
                arguments,
                Kind::Tube,
                context.construction_plane,
                self.settings.get().0,
            )?;
            (
                input.definition.expect("complete Tube"),
                Some(input.preferences),
            )
        };
        let use_extrusions = self.extrusions.get().0;
        let geometries = if preferences.is_some() {
            definition.geometries(use_extrusions, tolerance)?
        } else {
            vec![definition.geometry(use_extrusions, tolerance)?]
        };
        let representation = match &geometries[0] {
            Geometry::Brep(brep) if brep.is_extrusion() => "extrusion",
            _ => "B-rep",
        };
        let inner = definition.radius.min(definition.end_radius);
        let outer = definition.radius.max(definition.end_radius);
        let closed = match &geometries[0] {
            Geometry::Brep(brep) if brep.is_solid() => "closed",
            _ => "open",
        };
        let ids = geometries
            .into_iter()
            .map(|geometry| document.add_geometry(geometry).map(|id| id.to_string()))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        if let Some(preferences) = preferences {
            self.settings.set(TubeSettings(preferences));
        }
        let span = if definition.both_sides {
            ", both sides"
        } else {
            ""
        };
        Ok(format!(
            "Added {closed} {representation} tube {ids} (inner radius {inner:.6}, outer radius {outer:.6}, height {:.6}{span})",
            definition.height,
        ))
    }
}

fn positional_definition(
    options: TubeCommandOptions,
    tolerance: Tolerance,
) -> Result<Definition, CommandError> {
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
    Ok(Definition {
        kind: Kind::Tube,
        frame: base_frame,
        radius: first_radius,
        height: options.height,
        solid: true,
        both_sides: options.both_sides,
        end_radius: second_radius,
        topology: MeshTopology::default(),
    })
}
