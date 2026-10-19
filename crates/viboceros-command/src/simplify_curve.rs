//! Simplify complete selected curves without changing their document identities.
use super::*;
const USAGE: &str = "SimplifyCrv";
pub(super) struct SimplifyCurveCommand;
impl Command for SimplifyCurveCommand {
    fn name(&self) -> &'static str {
        "SimplifyCrv"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() {
            return Err(CommandError::Usage(USAGE));
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        run_selected(document, arguments, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run_selected(document, arguments, true)
    }
}
fn run_selected(
    document: &mut Document,
    arguments: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    if !arguments.is_empty() {
        return Err(CommandError::Usage(USAGE));
    }
    let mut plans = Vec::new();
    let mut count = 0;
    for object in document.selected_objects() {
        let Some(curve) = object.geometry().curve_ref() else {
            continue;
        };
        if !document.is_object_selectable(object.id()) {
            continue;
        }
        count += 1;
        let source = curve.to_owned();
        let result = simplified(&source, document.tolerance())?;
        if result.curve != source {
            plans.push((
                object.id(),
                Geometry::from(result.curve),
                !result.rebuilt_representation,
                object.geometry_user_text().clone(),
            ));
        }
    }
    if count == 0 {
        return Err(CommandError::Usage("SimplifyCrv requires selected curves"));
    }
    let changed = plans.len();
    // Native command-first replacements and their Undo/Redo states are
    // unselected. Clear before recording replacement edits; no-ops keep picks.
    if postselected && changed != 0 {
        document.clear_selection();
    }
    document.replace_object_geometries(
        plans
            .iter()
            .map(|(id, geometry, _, _)| (*id, geometry.clone())),
    )?;
    for (id, _, preserve, text) in plans {
        for (key, value) in text {
            document.set_object_geometry_user_text(
                [id],
                &key,
                preserve.then_some(value.as_str()),
            )?;
        }
    }
    Ok(if changed == 0 {
        "No curves were simplified".into()
    } else {
        format!("Simplified {changed} curve(s)")
    })
}

fn simplified(
    curve: &Curve3,
    tolerance: Tolerance,
) -> Result<viboceros_geometry::CurveSimplification, CommandError> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_geometry::simplify_curve_with_split(
            curve,
            tolerance,
            &mut viboceros_smlib::split_curve,
        )
        .map_err(knot_refinement::native_geometry_error)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        Ok(viboceros_geometry::simplify_curve(curve, tolerance)?)
    }
}

#[cfg(test)]
mod tests;
