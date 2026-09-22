//! Shared volume selection, scalar queries and atomic centroid markers.
use super::*;
use crate::{
    BooleanSelectionOption, ChoiceSelectionOption, ObjectSelectionFilter, ObjectSelectionPrompt,
    ObjectSelectionWorkflow,
};
use viboceros_geometry::{SurfaceVolumeMoments, VolumeBoundary, VolumeMassProperties};

#[cfg(test)]
mod tests;
mod units;
#[cfg(test)]
mod units_tests;

pub(crate) struct VolumeCommand {
    centroid: bool,
    display_units: crate::remembered::Remembered<&'static str>,
}
impl VolumeCommand {
    pub(crate) fn new(centroid: bool) -> Self {
        Self {
            centroid,
            display_units: crate::remembered::Remembered::new("ModelUnits"),
        }
    }

    fn usage(&self) -> &'static str {
        if self.centroid {
            "VolumeCentroid [Continue=Yes|No]"
        } else {
            "Volume [Units=name] [Continue=Yes|No]"
        }
    }

    fn parse(&self, arguments: &[&str]) -> Result<Options, CommandError> {
        let mut options = Options {
            answer: None,
            units: self.display_units.get(),
        };
        let mut unit_seen = false;
        let mut i = 0;
        while i < arguments.len() {
            let (name, value, used) = crate::orient_option(arguments, i, self.usage())?;
            if crate::option_name_eq(name, "Continue") && options.answer.is_none() {
                options.answer =
                    Some(crate::parse_yes_no(value).ok_or(CommandError::Usage(self.usage()))?);
            } else if !self.centroid && crate::option_name_eq(name, "Units") && !unit_seen {
                options.units = units::parse(value).ok_or(CommandError::Usage(self.usage()))?;
                unit_seen = true;
            } else {
                return Err(CommandError::Usage(self.usage()));
            }
            i += used;
        }
        Ok(options)
    }
}
const QUESTION: &str = "Some objects are not closed. Volume is meaningful only when the selected objects jointly enclose it. Continue? Yes/No; Enter or Esc uses Yes";

struct Options {
    answer: Option<bool>,
    units: &'static str,
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

impl Command for VolumeCommand {
    fn name(&self) -> &'static str {
        if self.centroid {
            "VolumeCentroid"
        } else {
            "Volume"
        }
    }

    fn records_history(&self) -> bool {
        self.centroid
    }

    fn cancel_empty_object_selection(&self) -> bool {
        !self.centroid
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Volume,
            options: options
                .answer
                .into_iter()
                .map(|value| BooleanSelectionOption {
                    name: "Continue",
                    value,
                    aliases: &[],
                })
                .collect(),
            menus: vec![],
            choices: if self.centroid {
                vec![]
            } else {
                vec![ChoiceSelectionOption {
                    name: "Units",
                    value: options.units,
                    choices: units::CHOICES,
                    toggle: None,
                }]
            },
            workflow: ObjectSelectionWorkflow::QuestionAfterSelection {
                message: QUESTION,
                escape_answer: Some(true),
            },
        }))
    }

    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        let options = self.parse(arguments)?;
        self.display_units.set(options.units);
        Ok(())
    }

    fn object_selection_confirmation(
        &self,
        document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(arguments)?;
        let boundaries = selected_boundaries(document)?;
        if options.answer.is_some() || !requires_confirmation(&boundaries, document.tolerance())? {
            return Ok(None);
        }
        // Unit choices belong to the picking phase, not the modal question.
        // Preserve the accepted display choice while keeping only Yes/No there.
        self.display_units.set(options.units);
        let mut question = self.object_selection_prompt(&["Continue=Yes"])?.unwrap();
        question.choices.clear();
        Ok(Some(question))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = self.parse(arguments)?;
        self.display_units.set(options.units);
        let boundaries = selected_boundaries(document)?;
        if requires_confirmation(&boundaries, document.tolerance())? {
            match options.answer {
                Some(true) => {}
                Some(false) => return Err(CommandError::OperationDeclined),
                None => return Err(CommandError::OpenVolumeConfirmationRequired),
            }
        }
        let count = boundaries.len();
        if !self.centroid {
            let volume = if let Some(target) = units::target(options.units) {
                VolumeMassProperties::signed_volume_from_boundaries_in_units(
                    &boundaries,
                    document.tolerance(),
                    document.units(),
                    &target,
                )?
            } else {
                VolumeMassProperties::signed_volume_from_boundaries(
                    &boundaries,
                    document.tolerance(),
                )?
            };
            return Ok(format!(
                "Measured {count} object(s): total volume {}{}",
                format_measurement(volume),
                units::label(options.units)
            ));
        }
        // Rhino averages coordinate primitives for surfaces but tetrahedral
        // cones for meshes. This differs from uniform physical cone moments
        // on open pieces (and on mixed unjoined boundaries); keep the choice
        // explicit here rather than redefining the kernel's default integral.
        let total = VolumeMassProperties::from_boundaries_with_surface_moments(
            &boundaries,
            document.tolerance(),
            SurfaceVolumeMoments::CoordinatePrimitives,
        )?;
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
        // Scalar Volume retains an unsupported-only preselection in the real
        // command capture; VolumeCentroid clears it. Do not conflate their
        // failure cleanup merely because their successful selection is shared.
        if (self.centroid && matches!(error, CommandError::UnsupportedVolumeGeometry))
            || (postselected && matches!(error, CommandError::OperationDeclined))
        {
            document.clear_selection();
        }
    }
}
