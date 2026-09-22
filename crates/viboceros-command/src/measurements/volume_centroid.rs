//! Signed-volume aggregation and an atomic current-layer centroid marker.
use super::*;
use crate::{ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow};
use viboceros_geometry::VolumeMassProperties;

#[cfg(test)]
mod tests;

pub(crate) struct VolumeCentroidCommand;

impl Command for VolumeCentroidCommand {
    fn name(&self) -> &'static str {
        "VolumeCentroid"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "VolumeCentroid")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "VolumeCentroid",
            filter: ObjectSelectionFilter::Volume,
            options: vec![],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "VolumeCentroid")?;
        let mut total = VolumeMassProperties::default();
        let mut count = 0;
        for object in document.selected_objects() {
            let mass = match object.geometry() {
                Geometry::Mesh(m) => m.volume_mass_properties()?,
                Geometry::Brep(b) => b.volume_mass_properties(document.tolerance())?,
                Geometry::NurbsSurface(s) => s.volume_mass_properties(document.tolerance())?,
                _ => continue,
            };
            total.add(&mass);
            count += 1;
        }
        if count == 0 {
            return Err(if document.selected_object_count() == 0 {
                CommandError::NoObjectsSelected
            } else {
                CommandError::UnsupportedVolumeGeometry
            });
        }
        // Rhino completes a signed zero-volume query without inventing a marker.
        if total.is_zero() {
            return Ok("Volume centroid is undefined: total signed volume is zero".into());
        }
        let centroid = total.centroid()?;
        document.add_geometry(Geometry::Point(centroid))?;
        let [x, y, z] = centroid.to_array().map(format_measurement);
        Ok(format!(
            "Volume centroid = {x},{y},{z} for {count} object(s)"
        ))
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
        if matches!(error, CommandError::UnsupportedVolumeGeometry) {
            document.clear_selection();
        }
    }
}
