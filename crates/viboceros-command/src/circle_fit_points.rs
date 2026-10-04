//! Point-object Circle FitPoints construction and selection/history policy.
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

pub(super) fn run(document: &mut Document) -> Result<String, CommandError> {
    let points = document
        .selected_objects()
        .filter_map(|object| match object.geometry() {
            Geometry::Point(point) => Some(*point),
            _ => None,
        })
        .take(viboceros_geometry::MAX_CIRCLE_FIT_POINTS + 1)
        .collect::<Vec<_>>();
    let Some(circle) = Circle3::try_fit_to_points(&points)? else {
        // Native collinear/coincident selections finish without any model edit.
        // The replay-only marker folds into prior history, preserving Redo.
        let selection = document.selected_object_ids().collect::<Vec<_>>();
        document.release_command_selection_on_history_replay(selection)?;
        return Ok("No circle fits the selected points".into());
    };
    let radius = circle.radius();
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let id = document.add_geometry(Geometry::Circle(circle))?;
    // Completed fits retain picks, including irrelevant preselected objects.
    // Native Undo and Redo release all of that command selection.
    document.release_command_selection_on_history_replay(selection)?;
    Ok(format!("Added circle {id} (radius {radius:.6})"))
}

#[cfg(test)]
mod tests;
