//! Planar surface, B-rep, and mesh hole capping.
use super::*;
use viboceros_geometry::BrepSolidOrientation;

#[cfg(test)]
mod tests;

const MESH_CAP_USAGE: &str = "Cap [DeleteInput=Yes|No] [Crease=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeshCapOptions {
    delete_input: bool,
    crease: bool,
}

impl Default for MeshCapOptions {
    fn default() -> Self {
        Self {
            delete_input: true,
            crease: true,
        }
    }
}

pub(super) struct CapCommand {
    options: remembered::Remembered<MeshCapOptions>,
}

impl Default for CapCommand {
    fn default() -> Self {
        Self {
            options: remembered::Remembered::new(MeshCapOptions::default()),
        }
    }
}

impl CapCommand {
    fn parse(&self, arguments: &[&str]) -> Result<MeshCapOptions, CommandError> {
        let mut options = self.options.get();
        let (mut delete_seen, mut crease_seen, mut index) = (false, false, 0);
        while index < arguments.len() {
            if arguments.len() == 1
                && let Some(value) = parse_yes_no(arguments[index])
            {
                options.delete_input = value;
                break;
            }
            let (name, value, consumed) = orient_option(arguments, index, MESH_CAP_USAGE)?;
            let value = parse_yes_no(value).ok_or(CommandError::Usage(MESH_CAP_USAGE))?;
            if option_name_eq(name, "DeleteInput") && !delete_seen {
                options.delete_input = value;
                delete_seen = true;
            } else if option_name_eq(name, "Crease") && !crease_seen {
                options.crease = value;
                crease_seen = true;
            } else {
                return Err(CommandError::Usage(MESH_CAP_USAGE));
            }
            index += consumed;
        }
        Ok(options)
    }
}

impl Command for CapCommand {
    fn name(&self) -> &'static str {
        "Cap"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Cap",
            filter: ObjectSelectionFilter::Cap,
            options: vec![
                BooleanSelectionOption {
                    name: "DeleteInput",
                    value: options.delete_input,
                    aliases: &[],
                },
                BooleanSelectionOption {
                    name: "Crease",
                    value: options.crease,
                    aliases: &[],
                },
            ],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        self.options.set(self.parse(arguments)?);
        Ok(())
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = self.parse(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut replacements = Vec::new();
        let mut mesh_copies = Vec::new();
        let mut face_count = 0;
        let mut unresolved_compounds = 0;
        let mut selected_mesh = false;
        for object in document.selected_objects() {
            if let Geometry::Mesh(mesh) = object.geometry() {
                selected_mesh = true;
                let (capped, count) =
                    mesh.cap_planar_holes_with_crease(document.tolerance(), options.crease)?;
                if count > 0 {
                    face_count += capped.face_count() - mesh.face_count();
                    if options.delete_input {
                        replacements.push((object.id(), Geometry::Mesh(capped)));
                    } else {
                        mesh_copies.push((object.id(), Geometry::Mesh(capped)));
                    }
                }
                continue;
            }
            let converted;
            let brep = match object.geometry() {
                Geometry::Brep(brep) => brep,
                Geometry::NurbsSurface(surface) => {
                    converted = Brep::try_surface_face(surface.clone(), document.tolerance())?;
                    &converted
                }
                _ => return Err(CommandError::UnsupportedCapGeometry),
            };
            if let Some(mut capped) = brep.try_cap_planar_holes(document.tolerance())? {
                // Subdivide newly capped boundaries at geometric tangent
                // breaks, preserving every incident trim and native domain.
                let original_uses = brep.edge_use_counts();
                let uses = capped.edge_use_counts();
                let boundaries = (0..brep.edges().len())
                    .filter(|&edge| original_uses[edge] == 1 && uses[edge] == 2)
                    .collect::<Vec<_>>();
                let split = capped.try_split_kinky_edges(
                    &boundaries,
                    1_f64.to_radians(),
                    document.tolerance(),
                )?;
                let split_edges = split.is_some();
                if let Some(result) = split {
                    capped = result;
                }
                // Normalize the entire result, not each component: cavity and
                // mixed-shell senses must be retained. Total signed volume
                // does not determine a compound solid's spatial orientation.
                match orientation_action(&capped, document.tolerance())? {
                    CapOrientationAction::Reverse => capped = capped.reversed(),
                    CapOrientationAction::Preserve => {}
                    CapOrientationAction::UnresolvedCompound => unresolved_compounds += 1,
                }
                // A single periodic face keeps its seam first in Rhino's
                // capped B-rep. This is table ordering, not curve rebuilding.
                if brep.faces().len() == 1 && !split_edges {
                    let seam = brep.faces()[0]
                        .loops()
                        .iter()
                        .flat_map(|l| l.trims())
                        .filter(|t| t.trim_type() == viboceros_geometry::BrepTrimType::Seam)
                        .filter_map(|t| t.edge())
                        .collect::<BTreeSet<_>>();
                    if !seam.is_empty() {
                        let order = (0..capped.edges().len())
                            .filter(|i| seam.contains(i))
                            .chain((0..capped.edges().len()).filter(|i| !seam.contains(i)))
                            .collect::<Vec<_>>();
                        capped = capped.reordered_edges(&order, document.tolerance())?;
                    }
                }
                face_count += capped.faces().len() - brep.faces().len();
                replacements.push((object.id(), Geometry::Brep(capped)));
            }
        }
        if !selected_mesh && !arguments.is_empty() {
            return Err(CommandError::Usage("Cap"));
        }
        let replaced = document.replace_object_geometries(replacements)?;
        let copied = document
            .copy_object_geometries_into_source_groups(mesh_copies)?
            .len();
        let count = replaced + copied;
        self.options.set(options);
        let mut message = format!("Capped {count} object(s) with {face_count} planar face(s)");
        if copied > 0 {
            message.push_str(&format!("; retained {copied} source mesh(es)"));
        }
        if unresolved_compounds > 0 {
            message.push_str(&format!(
                "; orientation unresolved for {unresolved_compounds} compound solid(s)"
            ));
        }
        Ok(message)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        let message = self.run(document, arguments)?;
        document.clear_selection();
        Ok(message)
    }
}

enum CapOrientationAction {
    Preserve,
    Reverse,
    UnresolvedCompound,
}

fn orientation_action(
    capped: &Brep,
    tolerance: Tolerance,
) -> Result<CapOrientationAction, GeometryError> {
    Ok(match capped.solid_orientation()? {
        BrepSolidOrientation::Inward => CapOrientationAction::Reverse,
        BrepSolidOrientation::Outward | BrepSolidOrientation::NotSolid => {
            CapOrientationAction::Preserve
        }
        BrepSolidOrientation::Unknown if capped.edge_connected_face_components().len() == 1 => {
            // The exact classifier does not yet cover all curved faces. Keep
            // the numerical volume fallback for a single connected shell only.
            // This is not an exact orientation certificate or a validity test.
            if capped.signed_volume(tolerance)? < 0.0 {
                CapOrientationAction::Reverse
            } else {
                CapOrientationAction::Preserve
            }
        }
        BrepSolidOrientation::Unknown => CapOrientationAction::UnresolvedCompound,
    })
}
