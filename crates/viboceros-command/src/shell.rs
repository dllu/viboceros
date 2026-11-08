//! Hollow selected solid faces through the native shell/Boolean executive.
use super::*;
use crate::extrude_surface::{SurfaceExtrusionInput, SurfaceFaces, surface_extrusion_targets};
use viboceros_geometry::LengthUnitSystem;
pub const USAGE: &str =
    "Shell [Thickness=distance] [DeleteInput=Yes|No] [Face=index|Faces=i,j|FaceRefs=id:index,...]";
pub(super) struct ShellCommand(pub std::sync::Arc<remembered::Remembered<ShellOptions>>);
mod options;
pub use options::{ShellInput, ShellOptions};
pub fn shell_targets(
    doc: &Document,
    faces: &SurfaceFaces,
) -> Result<BTreeMap<ObjectId, Vec<usize>>, CommandError> {
    let refs = surface_extrusion_targets(doc, faces).map_err(|_| CommandError::Usage(USAGE))?;
    let mut result = BTreeMap::<ObjectId, Vec<usize>>::new();
    for (id, index) in refs {
        let Some(Geometry::Brep(b)) = doc.object(id).map(|o| o.geometry()) else {
            return Err(CommandError::Usage(
                "Shell requires closed manifold polysurfaces",
            ));
        };
        if !b.is_solid() || b.faces().len() < 2 {
            return Err(CommandError::Usage(
                "Shell requires closed manifold polysurfaces",
            ));
        }
        result.entry(id).or_default().push(index);
    }
    for (id, indices) in &result {
        let Geometry::Brep(b) = doc.object(*id).unwrap().geometry() else {
            unreachable!()
        };
        if indices.len() >= b.faces().len() {
            return Err(CommandError::Usage(
                "Shell must retain at least one face on every source",
            ));
        }
    }
    Ok(result)
}
impl Command for ShellCommand {
    fn name(&self) -> &'static str {
        "Shell"
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        let input =
            ShellInput::parse_with_options(args, &LengthUnitSystem::default(), self.0.get())?;
        Ok(input.faces.is_none().then_some(ComponentSelectionPrompt {
            command: "Shell",
            kind: ComponentSelectionKind::BrepFace,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: input.options.delete_input,
                aliases: &[],
            }],
            numbers: vec![NumberSelectionOption {
                name: "Thickness",
                value: input.options.thickness,
                minimum: 0.,
            }],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let input = ShellInput::parse_with_options(args, doc.units(), self.0.get())?;
        self.0.set(input.options);
        let faces = input.faces.ok_or(CommandError::Usage(
            "Shell requires faces to remove; specify Face/Faces/FaceRefs",
        ))?;
        let sources = shell_targets(doc, &faces)?;
        let staged = stage_shell(doc, &sources, input.options.thickness)?;
        let count = staged.len();
        if input.options.delete_input {
            let mut replacements = BTreeMap::new();
            let mut extra = Vec::new();
            for (id, geometry) in staged {
                match replacements.entry(id) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(geometry);
                    }
                    std::collections::btree_map::Entry::Occupied(_) => extra.push((id, geometry)),
                }
            }
            doc.copy_object_pieces_into_source_groups(extra)?;
            doc.replace_object_geometries(replacements)?;
        } else {
            doc.copy_object_pieces_into_source_groups(staged)?;
        }
        doc.clear_selection();
        Ok(format!("Shelled {} solid object(s)", count))
    }
}
mod native;
mod zero;
fn stage_shell(
    doc: &Document,
    sources: &BTreeMap<ObjectId, Vec<usize>>,
    thickness: f64,
) -> Result<Vec<(ObjectId, Geometry)>, CommandError> {
    if thickness == 0. {
        sources
            .iter()
            .map(|(&id, removed)| {
                let Geometry::Brep(source) = doc.object(id).unwrap().geometry() else {
                    unreachable!()
                };
                Ok((
                    id,
                    Geometry::Brep(zero::shell(source, removed, doc.tolerance())?),
                ))
            })
            .collect()
    } else {
        native::stage_positive_shell(doc, sources, thickness)
    }
}
#[cfg(test)]
mod tests;
