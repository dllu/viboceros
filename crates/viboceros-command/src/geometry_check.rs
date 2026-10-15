//! Read-only native structural checks and direct selection of invalid objects.
use super::*;
pub use viboceros_io::{GeometryCheck, GeometryCheckStatus};
const CHECK: &str = "Check";
const SELECT: &str = "SelBadObjects";
const MAX_REPORT_BYTES: usize = 128 * 1024;
pub(super) struct CheckCommand;
pub(super) struct SelBadObjectsCommand;

/// Check the native equivalent of the actual stored geometry. Instance reference
/// validity checks UUID/placement; catalog integrity belongs to the document.
pub fn check_geometry(geometry: &Geometry) -> GeometryCheck {
    if let Geometry::BlockInstance(instance) = geometry {
        let reference = instance.reference();
        return viboceros_io::check_3dm_instance_reference(
            &reference.definition().to_string(),
            reference.transform(),
        );
    }
    match crate::interchange::geometry_to_3dm(geometry) {
        Ok(geometry) => viboceros_io::check_3dm_geometry(&geometry),
        Err(error) => GeometryCheck::not_checked(error.to_string()),
    }
}
impl Command for CheckCommand {
    fn name(&self) -> &'static str {
        CHECK
    }
    fn records_history(&self) -> bool {
        false
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, CHECK)?;
        Ok(Some(ObjectSelectionPrompt {
            command: CHECK,
            filter: ObjectSelectionFilter::Any,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        require_consumed(args, 0, CHECK)?;
        let mut counts = [0usize; 3];
        let mut details = String::new();
        let mut omitted = 0;
        for object in doc.selected_objects() {
            let result = check_geometry(object.geometry());
            let (index, label) = match result.status {
                GeometryCheckStatus::Valid => (0, "valid"),
                GeometryCheckStatus::Invalid => (1, "invalid"),
                GeometryCheckStatus::NotChecked => (2, "not checked"),
            };
            counts[index] += 1;
            let record = if result.diagnostics.is_empty() {
                format!("\n{}: {label}", object.id())
            } else {
                format!("\n{}: {label}\n{}", object.id(), result.diagnostics)
            };
            if details.len() + record.len() <= MAX_REPORT_BYTES {
                details.push_str(&record);
            } else {
                omitted += 1;
            }
        }
        if counts.iter().sum::<usize>() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut report = format!(
            "Check: {} valid, {} invalid, {} not checked",
            counts[0], counts[1], counts[2]
        );
        report.push_str(&details);
        if omitted > 0 {
            report.push_str(&format!("\nReports omitted for {omitted} object(s); select fewer objects for their diagnostics"));
        }
        Ok(report)
    }
}
impl Command for SelBadObjectsCommand {
    fn name(&self) -> &'static str {
        SELECT
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        require_consumed(args, 0, SELECT)?;
        let mut ids = Vec::new();
        let mut skipped = 0;
        for object in doc.selectable_objects() {
            match check_geometry(object.geometry()).status {
                GeometryCheckStatus::Invalid => ids.push(object.id()),
                GeometryCheckStatus::NotChecked => skipped += 1,
                GeometryCheckStatus::Valid => {}
            }
        }
        // Geometry validity never selects valid peers through group expansion.
        // Finish every check before changing selection; failures are not badness.
        let matched = ids.len();
        if !ids.is_empty() {
            doc.select_objects_direct(ids, SelectionMode::Add)?;
        }
        Ok(format!(
            "Selected {matched} bad object(s); {skipped} object(s) not checked"
        ))
    }
}
#[cfg(test)]
mod tests;
