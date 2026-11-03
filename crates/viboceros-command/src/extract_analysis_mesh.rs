//! Copy the shared analysis representation into independent model mesh objects.
use super::*;
use std::sync::Arc;
#[cfg(test)]
mod tests;

pub(crate) struct ExtractCommand {
    pub draft: Arc<draft_angle_analysis::Session>,
    pub curvature: Arc<curvature_analysis::Session>,
}
const USAGE: &str = "ExtractAnalysisMesh";
impl ExtractCommand {
    fn extract(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        require_consumed(args, 0, USAGE)?;
        let selected = doc.selected_objects().collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let draft = self.draft.view(doc);
        let curvature = self.curvature.view(doc);
        let cache = self.draft.mesh_cache();
        cache.retain(doc);
        let mut output = Vec::new();
        for object in selected {
            if let Geometry::BlockInstance(instance) = object.geometry() {
                for member in instance.members() {
                    if let Some(mesh) = analysis_mesh::generate(
                        &member.geometry,
                        doc.tolerance(),
                        doc.analysis_subd_density(),
                    )? {
                        output.push((
                            analysis_mesh::copy_mesh(&mesh, None, doc.tolerance())?,
                            member.attributes.clone(),
                        ));
                    }
                }
                continue;
            }
            let Some(mesh) = cache.get(object, doc)? else {
                continue;
            };
            let colors = if let Some(view) = curvature
                .as_ref()
                .filter(|v| v.objects.contains(&object.id()))
            {
                let mapping = view.options.mapping()?;
                Some(
                    mesh.principal_curvatures(None)
                        .into_iter()
                        .map(|c| c.map(|k| k.and_then(|k| mapping.color(k)).unwrap_or([128; 3])))
                        .collect::<Vec<_>>(),
                )
            } else if let Some(view) = draft.as_ref().filter(|v| v.objects.contains(&object.id())) {
                Some(analysis_mesh::draft_colors(&mesh, view.options.mapping()?))
            } else {
                None
            };
            output.push((
                analysis_mesh::copy_mesh(&mesh, colors.as_deref(), doc.tolerance())?,
                object.attributes().clone(),
            ));
        }
        let count = output.len();
        for (mesh, attributes) in output {
            doc.add_geometry_with_attributes(Geometry::Mesh(mesh), attributes)?;
        }
        if postselected {
            doc.clear_selection();
        }
        Ok(format!("Extracted {count} analysis mesh(es)"))
    }
}
impl Command for ExtractCommand {
    fn name(&self) -> &'static str {
        USAGE
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, USAGE)?;
        Ok(Some(ObjectSelectionPrompt {
            command: USAGE,
            filter: ObjectSelectionFilter::Any,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.extract(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        self.extract(doc, args, true)
    }
}
