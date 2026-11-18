//! Exact torus surfaces from complete scripts and interactive definitions.
use crate::{
    Command, CommandContext, CommandError, CommandRegistry, axial_primitive_frame,
    circle_input::CircleDefinition,
    option_name_eq, parse_radial_primitive_positionals, parse_unit_axis_option,
    remembered::Remembered,
    torus_input::{self, Definition, Input, Kind, Preferences},
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::Vector3;

const KIND: Kind = Kind::Solid;
const USAGE: &str = KIND.usage();

pub(super) struct TorusCommand(pub(super) Arc<Remembered<Settings>>);

#[derive(Clone, Copy)]
pub(super) struct Settings(Preferences);

impl Default for Settings {
    fn default() -> Self {
        Self(KIND.default_preferences())
    }
}

impl CommandRegistry {
    pub fn torus_preferences(&self) -> Preferences {
        self.torus_preferences.get().0
    }

    pub fn remember_torus_preferences(&self, preferences: Preferences) {
        self.torus_preferences.set(Settings(preferences));
    }
}

impl Command for TorusCommand {
    fn name(&self) -> &'static str {
        KIND.name()
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut preferences = self.0.get().0;
        let definition = if let Some(definition) = positional_definition(doc, args) {
            let definition = definition?;
            preferences.radius = definition.circle.radius;
            preferences.tube_radius = definition.tube_radius;
            definition
        } else {
            let input = torus_input::parse(
                doc,
                args,
                Input::for_kind(KIND, context.construction_plane, preferences),
                context.construction_plane,
            )?;
            preferences = input.preferences;
            input.definition.expect("completed torus definition")
        };
        let surface = definition.surface()?;
        let id = doc.add_geometry(Geometry::NurbsSurface(surface))?;
        self.0.set(Settings(preferences));
        let major = definition.major_radius();
        Ok(format!(
            "Added NURBS torus {id} (major radius {major:.6}, minor radius {:.6})",
            definition.tube_radius,
        ))
    }
}

/// The short positional syntax has two explicit radii and an optional world
/// axis. Named sizing options use the staged circle and tube getters.
fn positional_definition(
    doc: &Document,
    args: &[&str],
) -> Option<Result<Definition, CommandError>> {
    let positionals = parse_radial_primitive_positionals(args, USAGE).ok()?;
    let axis = match &args[positionals.option_start..] {
        [] => Vector3::try_new(0., 0., 1.)
            .and_then(Vector3::normalized_nonzero)
            .map_err(Into::into),
        [option] => {
            let (name, value) = option.split_once('=')?;
            if !option_name_eq(name, "Axis") {
                return None;
            }
            parse_unit_axis_option(value.trim_start_matches('_'), USAGE)
        }
        _ => return None,
    };
    Some(axis.and_then(|axis| {
        let (frame, radius) = axial_primitive_frame(
            positionals.center,
            positionals.radius,
            axis,
            doc.tolerance(),
        )?;
        Ok(Definition {
            circle: CircleDefinition { frame, radius },
            tube_radius: positionals.value,
            fix_inner_dimension: false,
        })
    }))
}

#[cfg(test)]
mod tests;
