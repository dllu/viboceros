//! Control-point display commands leave model history and Redo intact.
use super::*;

pub(super) struct PointsOnCommand;
impl Command for PointsOnCommand {
    fn name(&self) -> &'static str {
        "PointsOn"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(arguments.is_empty().then_some(ObjectSelectionPrompt {
            command: "PointsOn",
            filter: ObjectSelectionFilter::ControlPoints,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "PointsOn (select curves, surfaces or meshes)")?;
        let ids = document.selected_object_ids().collect::<Vec<_>>();
        let count = document.enable_control_points(ids)?;
        Ok(format!("Control points on for {count} object(s)"))
    }
}

pub(super) struct PointsOffCommand;
impl Command for PointsOffCommand {
    fn name(&self) -> &'static str {
        "PointsOff"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "PointsOff")?;
        let count = document.disable_control_points();
        Ok(format!("Control points off for {count} object(s)"))
    }
}
