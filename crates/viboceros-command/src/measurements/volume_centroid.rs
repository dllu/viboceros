//! Signed-volume aggregation and an atomic current-layer centroid marker.
use super::*;
use crate::{
    BooleanSelectionOption, ObjectSelectionFilter, ObjectSelectionPrompt, ObjectSelectionWorkflow,
};
use viboceros_geometry::{VolumeBoundary, VolumeMassProperties};

#[cfg(test)]
mod tests;

pub(crate) struct VolumeCentroidCommand;
const USAGE: &str = "VolumeCentroid [Continue=Yes|No]";
const QUESTION: &str = "Some objects are not closed. Volume is meaningful only when the selected objects jointly enclose it. Continue? Yes/No; Enter or Esc uses Yes";

fn continuation(arguments: &[&str]) -> Result<Option<bool>, CommandError> {
    if arguments.is_empty() {
        return Ok(None);
    }
    let (name, value, used) = crate::orient_option(arguments, 0, USAGE)?;
    if !crate::option_name_eq(name, "Continue") || used != arguments.len() {
        return Err(CommandError::Usage(USAGE));
    }
    crate::parse_yes_no(value)
        .map(Some)
        .ok_or(CommandError::Usage(USAGE))
}

fn selected_boundaries(document: &Document) -> Result<Vec<VolumeBoundary<'_>>, CommandError> {
    let boundaries = document
        .selected_objects()
        .filter_map(|o| o.geometry().volume_boundary())
        .collect::<Vec<_>>();
    if boundaries.is_empty() {
        return Err(if document.selected_object_count() == 0 {
            CommandError::NoObjectsSelected
        } else {
            CommandError::UnsupportedVolumeGeometry
        });
    }
    Ok(boundaries)
}

fn requires_confirmation(
    boundaries: &[VolumeBoundary<'_>],
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    for boundary in boundaries {
        if !boundary.is_closed(tolerance)? {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Command for VolumeCentroidCommand {
    fn name(&self) -> &'static str {
        "VolumeCentroid"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let answer = continuation(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "VolumeCentroid",
            filter: ObjectSelectionFilter::Volume,
            options: answer
                .into_iter()
                .map(|value| BooleanSelectionOption {
                    name: "Continue",
                    value,
                    aliases: &[],
                })
                .collect(),
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::QuestionAfterSelection {
                message: QUESTION,
                escape_answer: Some(true),
            },
        }))
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let answer = continuation(arguments)?;
        let boundaries = selected_boundaries(document)?;
        if answer.is_some() || !requires_confirmation(&boundaries, document.tolerance())? {
            return Ok(None);
        }
        self.object_selection_prompt(&["Continue=Yes"])
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let answer = continuation(arguments)?;
        let boundaries = selected_boundaries(document)?;
        if requires_confirmation(&boundaries, document.tolerance())? {
            match answer {
                Some(true) => {}
                Some(false) => return Err(CommandError::OperationDeclined),
                None => return Err(CommandError::OpenVolumeConfirmationRequired),
            }
        }
        let count = boundaries.len();
        let total = VolumeMassProperties::from_boundaries(&boundaries, document.tolerance())?;
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
        postselected: bool,
    ) {
        if matches!(error, CommandError::UnsupportedVolumeGeometry)
            || (postselected && matches!(error, CommandError::OperationDeclined))
        {
            document.clear_selection();
        }
    }
}
