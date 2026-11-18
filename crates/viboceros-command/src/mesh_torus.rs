//! Polygon torus output and legacy axis support for the shared torus getters.

use crate::{
    Command, CommandContext, CommandError, CommandRegistry, axial_primitive_frame,
    circle_input::CircleDefinition,
    option_name_eq, parse_radial_primitive_positionals, parse_unit_axis_option,
    remembered::Remembered,
    require_consumed,
    torus_input::{self, Definition, Input, Kind, Preferences},
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry};

const KIND: Kind = Kind::Mesh;
const USAGE: &str = KIND.usage();

pub(super) struct MeshTorusCommand(pub(super) Arc<Remembered<Settings>>);

#[derive(Clone, Copy)]
pub(super) struct Settings(Preferences);

impl Default for Settings {
    fn default() -> Self {
        Self(KIND.default_preferences())
    }
}

impl CommandRegistry {
    pub fn mesh_torus_preferences(&self) -> Preferences {
        self.mesh_torus_preferences.get().0
    }

    pub fn remember_mesh_torus_preferences(&self, preferences: Preferences) {
        self.mesh_torus_preferences.set(Settings(preferences));
    }
}

impl Command for MeshTorusCommand {
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
        let (preferences, words) = topology_arguments(args, self.0.get().0)?;
        let (definition, preferences) = if let Some((last, positionals)) = words.split_last()
            && let Some((name, axis)) = last.split_once('=')
            && option_name_eq(name, "Axis")
        {
            let args = parse_radial_primitive_positionals(positionals, USAGE)?;
            require_consumed(positionals, args.option_start, USAGE)?;
            let axis = parse_unit_axis_option(axis.trim_start_matches('_'), USAGE)?;
            let (frame, radius) =
                axial_primitive_frame(args.center, args.radius, axis, doc.tolerance())?;
            (
                Definition {
                    circle: CircleDefinition { frame, radius },
                    tube_radius: args.value,
                    fix_inner_dimension: false,
                },
                preferences,
            )
        } else {
            let input = torus_input::parse(
                doc,
                &words,
                Input::for_kind(KIND, context.construction_plane, preferences),
                context.construction_plane,
            )?;
            (
                input.definition.expect("completed torus definition"),
                input.preferences,
            )
        };
        let mesh = definition.mesh(preferences, doc.tolerance())?;
        let id = doc.add_geometry(Geometry::Mesh(mesh))?;
        self.0.set(Settings(preferences));
        let major = definition.major_radius();
        Ok(format!(
            "Added closed mesh torus {id} (major radius {major:.6}, minor radius {:.6}; {} around × {} vertical faces)",
            definition.tube_radius, preferences.around_faces, preferences.vertical_faces
        ))
    }
}

fn topology_arguments<'a>(
    args: &[&'a str],
    mut preferences: Preferences,
) -> Result<(Preferences, Vec<&'a str>), CommandError> {
    let mut words = args.iter().copied();
    let mut definition = Vec::new();
    let mut seen = [false; 2];
    while let Some(word) = words.next() {
        let (name, value) = word
            .split_once('=')
            .map_or((word, None), |(name, value)| (name, Some(value)));
        let around = option_name_eq(name, "AroundFaces");
        if !around && !option_name_eq(name, "VerticalFaces") {
            definition.push(word);
            continue;
        }
        let index = usize::from(around);
        if seen[index] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[index] = true;
        let value = value
            .or_else(|| words.next())
            .ok_or(CommandError::Usage(USAGE))?;
        let count = value
            .trim_start_matches('_')
            .parse::<usize>()
            .ok()
            .filter(|&n| n >= 3)
            .ok_or_else(|| {
                if around {
                    CommandError::InvalidMeshTorusAroundFaceCount(value.into())
                } else {
                    CommandError::InvalidMeshTorusVerticalFaceCount(value.into())
                }
            })?;
        if around {
            preferences.around_faces = count;
        } else {
            preferences.vertical_faces = count;
        }
    }
    preferences.validate_for(KIND)?;
    Ok((preferences, definition))
}

#[cfg(test)]
mod tests;
