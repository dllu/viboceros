//! Mesh channel preservation with command-instance defaults and staged edits.
use crate::*;
#[cfg(test)]
mod tests;
const USAGE: &str = "RebuildMesh [PreserveTextureCoordinates=Yes|No] [PreserveVertexColors=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshRebuildOptions {
    pub preserve_texture_coordinates: bool,
    pub preserve_vertex_colors: bool,
}
impl MeshRebuildOptions {
    pub fn command(self) -> String {
        format!(
            "RebuildMesh PreserveTextureCoordinates={} PreserveVertexColors={}",
            if self.preserve_texture_coordinates {
                "Yes"
            } else {
                "No"
            },
            if self.preserve_vertex_colors {
                "Yes"
            } else {
                "No"
            }
        )
    }
}
fn parse(
    args: &[&str],
    mut options: MeshRebuildOptions,
) -> Result<MeshRebuildOptions, CommandError> {
    let mut index = 0;
    let mut seen = [false; 2];
    while index < args.len() {
        let (key, inline) = args[index]
            .split_once('=')
            .map_or((args[index], None), |(k, v)| (k, Some(v)));
        let slot = if option_name_eq(key, "PreserveTextureCoordinates") {
            0
        } else if option_name_eq(key, "PreserveVertexColors") {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if seen[slot] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[slot] = true;
        let current = if slot == 0 {
            &mut options.preserve_texture_coordinates
        } else {
            &mut options.preserve_vertex_colors
        };
        if let Some(value) = inline {
            *current = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            index += 1;
        } else if let Some(value) = args.get(index + 1).and_then(|v| parse_yes_no(v)) {
            *current = value;
            index += 2;
        } else {
            *current = !*current;
            index += 1;
        }
    }
    Ok(options)
}

pub(crate) struct RebuildMeshCommand(
    pub std::sync::Arc<remembered::Remembered<MeshRebuildOptions>>,
);
impl CommandRegistry {
    pub fn rebuild_mesh_options(&self, args: &[&str]) -> Result<MeshRebuildOptions, CommandError> {
        parse(args, self.rebuild_mesh_preferences.get())
    }
}
impl Command for RebuildMeshCommand {
    fn name(&self) -> &'static str {
        "RebuildMesh"
    }
    fn update_object_selection_options(
        &self,
        prompt: &mut ObjectSelectionPrompt,
        input: &str,
    ) -> Result<(), CommandError> {
        let current = prompt.command_line();
        let current = parse(
            &current.split_whitespace().skip(1).collect::<Vec<_>>(),
            self.0.get(),
        )?;
        let updated = parse(&input.split_whitespace().collect::<Vec<_>>(), current)?.command();
        prompt.update_options(updated.split_once(' ').unwrap().1)
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = parse(args, self.0.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Mesh,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![
                BooleanSelectionOption {
                    name: "PreserveTextureCoordinates",
                    value: options.preserve_texture_coordinates,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "PreserveVertexColors",
                    value: options.preserve_vertex_colors,
                    aliases: &[],
                },
            ],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl RebuildMeshCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = parse(args, self.0.get())?;
        if doc.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let replacements = doc
            .selected_objects()
            .filter_map(|object| {
                let Geometry::Mesh(mesh) = object.geometry() else {
                    return None;
                };
                Some(
                    mesh.rebuilt(
                        options.preserve_texture_coordinates,
                        options.preserve_vertex_colors,
                    )
                    .map(|mesh| (object.id(), mesh)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if replacements.is_empty() {
            return Err(CommandError::UnsupportedRebuildMeshGeometry);
        }
        let count = replacements.len();
        super::replacement::replace_meshes(doc, replacements, postselected)?;
        self.0.set(options);
        Ok(format!("Rebuilt {count} mesh(es)"))
    }
}
