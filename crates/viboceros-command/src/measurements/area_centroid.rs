//! Cumulative area centroids and one atomic, current-layer point marker.
use super::*;
use crate::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow};
use viboceros_geometry::AreaMassProperties;

#[cfg(test)]
mod tests;

pub(crate) struct AreaCentroidCommand;

impl Command for AreaCentroidCommand {
    fn name(&self) -> &'static str {
        "AreaCentroid"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "AreaCentroid")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "AreaCentroid",
            filter: ObjectSelectionFilter::Area,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "AreaCentroid")?;
        let mut total = AreaMassProperties::default();
        let mut count = 0;
        for object in document.selected_objects() {
            let mass = match object.geometry() {
                Geometry::NurbsSurface(surface) => {
                    surface.area_mass_properties(document.tolerance())?
                }
                Geometry::Brep(brep) => brep.area_mass_properties(document.tolerance())?,
                Geometry::Mesh(mesh) => mesh.area_mass_properties()?,
                geometry => {
                    let Some(curve) = geometry.curve_ref() else {
                        continue;
                    };
                    if !curve.is_closed()? || !curve.is_planar(document.tolerance())? {
                        continue;
                    }
                    curve.planar_area_mass_properties(document.tolerance())?
                }
            };
            total.add(&mass);
            count += 1;
        }
        if count == 0 {
            return Err(if document.selected_object_count() == 0 {
                CommandError::NoObjectsSelected
            } else {
                CommandError::UnsupportedAreaGeometry
            });
        }
        let centroid = total.centroid()?;
        document.add_geometry(Geometry::Point(centroid))?;
        let [x, y, z] = centroid.to_array().map(format_measurement);
        Ok(format!("Area centroid = {x},{y},{z} for {count} object(s)"))
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: crate::CommandContext,
    ) -> Result<String, CommandError> {
        let message = self.run(document, arguments)?;
        document.clear_selection();
        Ok(message)
    }

    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(error, CommandError::UnsupportedAreaGeometry) {
            document.clear_selection();
        }
    }
}
