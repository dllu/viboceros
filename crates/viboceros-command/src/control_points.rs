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

pub(super) struct EditPointsOnCommand;
impl Command for EditPointsOnCommand {
    fn name(&self) -> &'static str {
        "EditPtOn"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(arguments.is_empty().then_some(ObjectSelectionPrompt {
            command: "EditPtOn",
            filter: ObjectSelectionFilter::EditPoints,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "EditPtOn (select curves or SubDs)")?;
        let ids = document.selected_object_ids().collect::<Vec<_>>();
        let count = document.enable_edit_points(ids.iter().copied())?;
        // Unlike PointsOn, native EditPtOn retains the preselected owners.
        document.select_objects_direct(ids, SelectionMode::Add)?;
        Ok(format!("Edit points on for {count} object(s)"))
    }
}
