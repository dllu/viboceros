//! Point and grip Circle FitPoints construction and selection/history policy.
use super::*;

pub(super) fn selection_prompt() -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: "Circle FitPoints",
        filter: ObjectSelectionFilter::Points,
        options: Vec::new(),
        menus: Vec::new(),
        choices: Vec::new(),
        workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
    }
}

pub(super) fn run(document: &mut Document, postselected: bool) -> Result<String, CommandError> {
    run_with_output(document, postselected, None)
}

pub(super) fn run_with_output(
    document: &mut Document,
    postselected: bool,
    output: Option<crate::circle_deform_input::Output>,
) -> Result<String, CommandError> {
    let Some(circle) = selected_circle(document)? else {
        // Native collinear/coincident selections finish without any model edit.
        // The replay-only marker folds into prior history, preserving Redo.
        if postselected {
            let selection = document.selected_object_ids().collect::<Vec<_>>();
            document.release_command_selection_on_history_replay(selection)?;
            document.clear_selection();
        }
        return Ok("No circle fits the selected points".into());
    };
    let radius = circle.radius();
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let geometry = output.map_or_else(|| Ok(Geometry::Circle(circle)), |o| o.geometry(circle))?;
    let id = document.add_geometry(geometry)?;
    if postselected {
        document.release_command_selection_on_history_replay(selection)?;
        document.clear_selection();
    }
    Ok(format!("Added circle {id} (radius {radius:.6})"))
}

/// Fit selected point objects and visible selected grips without changing the document.
pub fn selected_circle(document: &Document) -> Result<Option<Circle3>, CommandError> {
    let points = selected_points(document)
        .take(viboceros_geometry::MAX_CIRCLE_FIT_POINTS + 1)
        .collect::<Vec<_>>();
    Ok(Circle3::try_fit_to_points(&points)?)
}

/// Selected point occurrences used by Circle FitPoints and reference getters.
pub fn selected_points(document: &Document) -> impl Iterator<Item = Point3> + '_ {
    document
        .selected_objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Point(point) => Some(*point),
            _ => None,
        })
        .chain(document.selected_control_points().map(|(_, point)| point))
}

#[cfg(test)]
mod tests;
