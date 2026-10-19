//! Conic annotation leaves source geometry and attributes untouched.
use super::*;
use crate::remembered::Remembered;
use viboceros_geometry::ConicFoci;
const USAGE: &str = "MarkFoci [DrawAxis=Yes|No]";
pub(super) struct MarkFociCommand(Remembered<bool>);
impl Default for MarkFociCommand {
    fn default() -> Self {
        Self(Remembered::new(true))
    }
}
impl MarkFociCommand {
    fn option(&self, args: &[&str]) -> Result<bool, CommandError> {
        match args {
            [] => Ok(self.0.get()),
            [word] => {
                let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
                if !option_name_eq(key, "DrawAxis") {
                    return Err(CommandError::Usage(USAGE));
                }
                parse_yes_no(value).ok_or(CommandError::Usage(USAGE))
            }
            [key, value] if option_name_eq(key, "DrawAxis") => {
                parse_yes_no(value).ok_or(CommandError::Usage(USAGE))
            }
            _ => Err(CommandError::Usage(USAGE)),
        }
    }
}
impl Command for MarkFociCommand {
    fn name(&self) -> &'static str {
        "MarkFoci"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![BooleanSelectionOption {
                name: "DrawAxis",
                value: self.option(args)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.0.set(self.option(args)?);
        Ok(())
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
impl MarkFociCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let draw_axis = self.option(args)?;
        let mut output = Vec::new();
        let mut curves = 0;
        for object in doc.selected_objects() {
            if !doc.is_object_selectable(object.id()) {
                continue;
            }
            let Some(curve) = object.geometry().curve_ref() else {
                continue;
            };
            let Some(foci) = curve.to_nurbs()?.conic_foci(doc.tolerance())? else {
                continue;
            };
            curves += 1;
            let (points, axis) = match foci {
                ConicFoci::Circle { center } => (vec![center], None),
                ConicFoci::Ellipse { foci } | ConicFoci::Hyperbola { foci } => {
                    (foci.to_vec(), Some(foci))
                }
                ConicFoci::Parabola { vertex, focus } => (vec![focus], Some([vertex, focus])),
            };
            for point in points {
                output.push(Geometry::Point(point));
            }
            if draw_axis && let Some([start, end]) = axis {
                // Native axis domains are their model-space length.
                output.push(Geometry::Line(LineSegment::try_new(
                    start,
                    end,
                    Tolerance::NUMERICAL_VALIDATION,
                )?));
            }
        }
        let count = output.len();
        if postselected && !output.is_empty() {
            let selected = doc.selected_object_ids().collect::<Vec<_>>();
            doc.release_command_selection_on_history_replay(selected)?;
        }
        for geometry in output {
            doc.add_geometry(geometry)?;
        }
        self.0.set(draw_axis);
        Ok(format!(
            "Marked {curves} conic curve(s), adding {count} object(s)"
        ))
    }
}
