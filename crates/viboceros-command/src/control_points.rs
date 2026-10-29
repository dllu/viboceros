//! Transient control and edit-point display commands.
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

/// Unlike PointsOff, native PtOffSelected discards Redo at command startup.
pub(super) struct PtOffSelectedCommand;
impl Command for PtOffSelectedCommand {
    fn name(&self) -> &'static str {
        "PtOffSelected"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(arguments, 0, "PtOffSelected")?;
        Ok(Some(ObjectSelectionPrompt {
            command: "PtOffSelected",
            filter: ObjectSelectionFilter::PointDisplayOwners,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn begin_object_selection(&self, document: &mut Document) -> Result<(), CommandError> {
        run_command_transaction(document, self.name(), |document| {
            document.record_accepted_unchanged_command()?;
            Ok(())
        })
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "PtOffSelected")?;
        self.begin_object_selection(document)?;
        close_selected_displays(document, false)
    }
    fn run_after_object_selection(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
        _projection: Option<picking::Projection>,
    ) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "PtOffSelected")?;
        close_selected_displays(document, true)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "PtOffSelected")?;
        self.begin_object_selection(document)?;
        close_selected_displays(document, true)
    }
}

fn close_selected_displays(
    document: &mut Document,
    postselected: bool,
) -> Result<String, CommandError> {
    let ids = document
        .selected_objects()
        .filter(|o| ObjectSelectionFilter::PointDisplayOwners.accepts_in_document(document, o))
        .map(|o| o.id())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let count = document.disable_control_points_for_objects(ids.iter().copied());
    if postselected {
        document.select_objects_direct(ids, SelectionMode::Remove)?;
    }
    Ok(format!("Points off for {count} object(s)"))
}

#[cfg(test)]
mod tests;
