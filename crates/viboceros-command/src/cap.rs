//! In-place planar surface/B-rep hole capping.
use super::*;

#[cfg(test)]
mod tests;

pub(super) struct CapCommand;

impl Command for CapCommand {
    fn name(&self) -> &'static str {
        "Cap"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "Cap")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Cap",
            filter: ObjectSelectionFilter::SurfaceComponents,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Cap")?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut replacements = Vec::new();
        let mut face_count = 0;
        for object in document.selected_objects() {
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
                // The kernel retains the source shell sense. Rhino's command
                // orients newly closed solids outward, including inward inputs.
                if capped.is_solid() && capped.signed_volume(document.tolerance())? < 0.0 {
                    capped = capped.reversed();
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
        let count = document.replace_object_geometries(replacements)?;
        Ok(format!(
            "Capped {count} object(s) with {face_count} planar face(s)"
        ))
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
