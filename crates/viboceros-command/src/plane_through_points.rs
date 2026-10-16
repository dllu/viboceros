//! Least-squares rectangular planes from immutable point and grip selections.
use super::*;

pub(super) struct PlaneThroughPointsCommand;
const USAGE: &str = "PlaneThroughPt (select at least three points, cloud points or control points)";

impl Command for PlaneThroughPointsCommand {
    fn name(&self) -> &'static str {
        "PlaneThroughPt"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, USAGE)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::PlaneFitPoints,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        run(document, arguments, false)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run(document, arguments, true)
    }
}

fn run(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    require_consumed(arguments, 0, USAGE)?;
    let maximum = viboceros_geometry::MAX_PLANE_FIT_POINTS;
    let points = document
        .selected_objects()
        .flat_map(|object| match object.geometry() {
            Geometry::Point(p) => std::slice::from_ref(p),
            Geometry::PointCloud(cloud) => cloud.points(),
            _ => &[],
        })
        .copied()
        .chain(document.selected_control_points().map(|(_, point)| point))
        .take(maximum + 1)
        .collect::<Vec<_>>();
    let Some(surface) = NurbsSurface::try_fit_plane(&points)? else {
        return Err(CommandError::OperationDeclined);
    };
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let id = document.add_geometry(Geometry::NurbsSurface(surface))?;
    if postselected {
        document.release_command_selection_on_history_replay(selection)?;
        document.clear_selection();
    }
    Ok(format!(
        "Added fitted planar surface {id} from {} points",
        points.len()
    ))
}

#[cfg(test)]
mod tests;
