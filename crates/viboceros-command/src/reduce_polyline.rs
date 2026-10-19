//! Reduction policy is separate from polyline recognition and geometry work.
use super::*;
use crate::remembered::Remembered;

pub const USAGE: &str = "ReducePolyline [tolerance|Tolerance=value] [Copy=Yes|No]";
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub tolerance: Option<Real>,
    pub copy: Option<bool>,
}
/// Accept a complete script or a partial numeric getter without changing state.
pub fn parse(arguments: &[&str], mut input: Input) -> Result<Input, CommandError> {
    let mut i = 0;
    let mut value = false;
    let mut copy = false;
    while i < arguments.len() {
        let word = arguments[i];
        if let Some((name, answer)) = word.split_once('=') {
            if option_name_eq(name, "Tolerance") && !value {
                input.tolerance = Some(parse_finite_real(answer)?);
                value = true;
            } else if option_name_eq(name, "Copy") && !copy {
                input.copy = Some(parse_yes_no(answer).ok_or(CommandError::Usage(USAGE))?);
                copy = true;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        } else if option_name_eq(word, "Tolerance") && !value {
            i += 1;
            if let Some(answer) = arguments.get(i) {
                input.tolerance = Some(parse_finite_real(answer)?);
                value = true;
            }
        } else if option_name_eq(word, "Copy") && !copy {
            i += 1;
            input.copy = Some(
                parse_yes_no(arguments.get(i).ok_or(CommandError::Usage(USAGE))?)
                    .ok_or(CommandError::Usage(USAGE))?,
            );
            copy = true;
        } else if !value {
            input.tolerance = Some(parse_finite_real(word)?);
            value = true;
        } else {
            return Err(CommandError::Usage(USAGE));
        }
        i += 1;
    }
    if input.tolerance.is_some_and(|v| !v.is_finite() || v <= 0.) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(input)
}
pub fn accepts_curve(curve: CurveRef<'_>) -> bool {
    match curve {
        CurveRef::Line(_) | CurveRef::Polyline(_) => true,
        CurveRef::NurbsCurve(c) if c.degree() == 1 => true,
        CurveRef::PolyCurve(c) => c.segments().iter().all(|s| accepts_curve(s.as_ref())),
        _ => curve.polyline_vertices().is_ok_and(|p| p.is_some()),
    }
}

#[derive(Default)]
pub(super) struct ReducePolylineCommand(Remembered<Option<Real>>);
impl Command for ReducePolylineCommand {
    fn name(&self) -> &'static str {
        "ReducePolyline"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn scalar_default(&self) -> Option<Real> {
        self.0.get()
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let input = parse(args, Input::default())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Polylines,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            options: vec![BooleanSelectionOption {
                name: "Copy",
                value: input.copy.unwrap_or(false),
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn object_selection_confirmation(
        &self,
        _: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(args)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl ReducePolylineCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let input = parse(args, Input::default())?;
        let tolerance = input
            .tolerance
            .or(self.0.get())
            .unwrap_or(doc.tolerance().absolute());
        let mut plans = Vec::new();
        let mut eligible = 0;
        let mut removed = 0;
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                continue;
            }
            let Some(curve) = object.geometry().curve_ref() else {
                continue;
            };
            let Some(vertices) = curve.polyline_vertices()? else {
                continue;
            };
            eligible += 1;
            let source = Polyline3::try_new(vertices, Tolerance::NUMERICAL_VALIDATION)?;
            let mut result = source.try_reduced(tolerance)?;
            if result.vertices().len() == source.vertices().len() {
                continue;
            }
            removed += source.vertices().len() - result.vertices().len();
            if input.copy.unwrap_or(false) {
                result = result.try_chord_length_parameterized()?;
            }
            plans.push((object.id(), Geometry::Polyline(result)));
        }
        if eligible == 0 {
            return Err(CommandError::Usage(
                "ReducePolyline requires selected polylines",
            ));
        }
        let changed = plans.len();
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        if postselected && changed > 0 {
            doc.release_command_selection_on_history_replay(selected.iter().copied())?;
            doc.clear_selection();
        }
        if input.copy.unwrap_or(false) {
            for (_, geometry) in plans {
                doc.add_geometry(geometry)?;
            }
            if postselected && changed > 0 {
                doc.select_command_results(selected)?;
            }
        } else {
            doc.replace_object_geometries(plans)?;
        }
        self.0.set(Some(tolerance));
        Ok(format!(
            "Reduced {changed} polyline(s); removed {removed} vertices"
        ))
    }
}

#[cfg(test)]
mod tests;
