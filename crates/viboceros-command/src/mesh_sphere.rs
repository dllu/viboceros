//! Mesh topology and output for the shared sphere construction getters.

use crate::{
    Command, CommandContext, CommandError, CommandRegistry, ObjectSelectionPrompt,
    axial_primitive_frame, circle_input::CircleDefinition, option_name_eq,
    parse_radial_radius_positionals, parse_unit_axis_option, point_fitting, remembered::Remembered,
    require_consumed, sphere_command, sphere_input,
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry, ObjectId};

const USAGE: &str = "MeshSphere [Style=UV|Quad|Triangle] [VerticalFaces=n AroundFaces=n | Subdivisions=n] (sphere definition)";
pub const FIT_COMMAND: &str = "MeshSphere FitPoints";

mod topology;

pub use topology::{Topology, TopologyOption, parse_topology};

pub(super) struct MeshSphereCommand(pub(super) Arc<Remembered<Topology>>);

impl CommandRegistry {
    pub fn mesh_sphere_preferences(&self) -> Topology {
        self.mesh_sphere_preferences.get()
    }

    pub fn remember_mesh_sphere_preferences(&self, preferences: Topology) {
        self.mesh_sphere_preferences.set(preferences);
    }
}

impl Command for MeshSphereCommand {
    fn name(&self) -> &'static str {
        "MeshSphere"
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !args.iter().any(|word| option_name_eq(word, "FitPoints")) {
            return Ok(None);
        }
        let (_, definition) = parse_topology(args, self.0.get())?;
        Ok(
            matches!(definition.as_slice(), [mode] if option_name_eq(mode, "FitPoints")).then(
                || {
                    let mut prompt =
                        point_fitting::selection_prompt(point_fitting::PointFitKind::Sphere);
                    prompt.command = FIT_COMMAND;
                    prompt
                },
            ),
        )
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
        self.create(doc, args, context, false)
    }

    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.create(doc, args, context, true)
    }
}

impl MeshSphereCommand {
    fn create(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
        postselected: bool,
    ) -> Result<String, CommandError> {
        let (topology, words) = parse_topology(args, self.0.get())?;
        let fitting = words
            .first()
            .is_some_and(|v| option_name_eq(v, "FitPoints"));
        let (definition, release) = if fitting {
            require_consumed(&words, 1, FIT_COMMAND)?;
            let points = point_fitting::PointFitKind::Sphere.selected_points(doc);
            let definition = sphere_input::SphereInput::fit_optional(&points)?
                .ok_or(CommandError::OperationDeclined)?;
            let release = postselected
                .then(|| doc.selected_object_ids().collect())
                .unwrap_or_default();
            (definition, release)
        } else if let Some((last, positionals)) = words.split_last()
            && let Some((option, axis)) = last.split_once('=')
            && option_name_eq(option, "Axis")
        {
            let args = parse_radial_radius_positionals(positionals, USAGE)?;
            require_consumed(positionals, args.option_start, USAGE)?;
            let axis = parse_unit_axis_option(axis, USAGE)?;
            let (frame, radius) =
                axial_primitive_frame(args.center, args.radius, axis, doc.tolerance())?;
            (CircleDefinition { frame, radius }, Vec::new())
        } else {
            let (definition, release) = sphere_command::from_arguments(
                doc,
                &words,
                sphere_input::SphereInput::new(context.construction_plane),
                &mut sphere_input::Preferences::default(),
            )?;
            (definition, release.into_iter().collect())
        };
        let message = add(doc, topology, definition, &release)?;
        if fitting && postselected {
            doc.clear_control_point_selection();
        }
        self.0.set(topology);
        Ok(message)
    }
}

pub fn commit(
    doc: &mut Document,
    topology: Topology,
    definition: CircleDefinition,
    release: Option<ObjectId>,
) -> Result<String, CommandError> {
    crate::run_command_transaction(doc, "MeshSphere", |doc| {
        add(doc, topology, definition, release.as_slice())
    })
}

fn add(
    doc: &mut Document,
    topology: Topology,
    definition: CircleDefinition,
    release: &[ObjectId],
) -> Result<String, CommandError> {
    let mesh = topology.mesh(definition, doc.tolerance())?;
    if !release.is_empty() {
        doc.release_command_selection_on_history_replay(release.iter().copied())?;
        doc.select_command_results([])?;
    }
    let (style, details) = topology.output_description(mesh.face_count());
    let id = doc.add_geometry(Geometry::Mesh(mesh))?;
    Ok(format!("Added {style} mesh sphere {id} ({details})"))
}

#[cfg(test)]
mod tests;
