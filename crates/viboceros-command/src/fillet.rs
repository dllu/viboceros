//! Joins two selected open curves with a tangent circular fillet.

use super::*;
use viboceros_geometry::{Curve3, try_fillet_curves_joined};

const USAGE: &str = "Fillet radius [Pick1=x,y,z] [Pick2=x,y,z]";

pub(super) struct FilletCommand;

impl Command for FilletCommand {
    fn name(&self) -> &'static str {
        "Fillet"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let (radius, picks) = parse(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                Ok((
                    object.id(),
                    object
                        .geometry()
                        .curve_ref()
                        .ok_or(CommandError::FilletRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::FilletRequiresTwoCurves);
        };
        let (default_first, default_second) = nearest_ends(first, second)?;
        let joined = try_fillet_curves_joined(
            first,
            picks[0].unwrap_or(default_first),
            second,
            picks[1].unwrap_or(default_second),
            radius,
            document.tolerance(),
        )?;
        let outputs = document
            .copy_object_pieces_into_source_groups([(*first_id, Geometry::PolyCurve(joined))])?;
        document.delete_objects([*first_id, *second_id])?;
        document.select_objects_direct(outputs, SelectionMode::Replace)?;
        Ok("Filleted two curves".to_string())
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        parse(arguments)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            menus: vec![],
            choices: vec![],
            options: vec![],
        }))
    }
}

fn parse(arguments: &[&str]) -> Result<(Real, [Option<Point3>; 2]), CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(USAGE));
    };
    let radius = if let Some((name, value)) = first.split_once('=') {
        if !option_name_eq(name, "Radius") {
            return Err(CommandError::Usage(USAGE));
        }
        parse_finite_real(value)?
    } else {
        parse_finite_real(first)?
    };
    let mut picks = [None, None];
    for argument in &arguments[1..] {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        let index = if option_name_eq(name, "Pick1") {
            0
        } else if option_name_eq(name, "Pick2") {
            1
        } else {
            return Err(CommandError::Usage(USAGE));
        };
        if picks[index].is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let (pick, consumed) = parse_point(&[value])?;
        if consumed != 1 {
            return Err(CommandError::Usage(USAGE));
        }
        picks[index] = Some(pick);
    }
    Ok((radius, picks))
}

fn nearest_ends(first: &Curve3, second: &Curve3) -> Result<(Point3, Point3), CommandError> {
    let first_ref = first.as_ref();
    let second_ref = second.as_ref();
    let first_ends = [
        first_ref.evaluate(*first_ref.domain().start())?,
        first_ref.evaluate(*first_ref.domain().end())?,
    ];
    let second_ends = [
        second_ref.evaluate(*second_ref.domain().start())?,
        second_ref.evaluate(*second_ref.domain().end())?,
    ];
    let mut best = (Real::INFINITY, first_ends[0], second_ends[0]);
    for first_end in first_ends {
        for second_end in second_ends {
            let distance = first_end.distance_to(second_end)?;
            if distance < best.0 {
                best = (distance, first_end, second_end);
            }
        }
    }
    Ok((best.1, best.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_selected_lines_and_restores_sources_with_undo() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Line 0,0,0 4,0,0").unwrap();
        registry.execute(&mut document, "Line 4,0,0 4,4,0").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut document, "Fillet 0.5").unwrap();
        assert_eq!(document.objects().count(), 1);
        assert!(matches!(
            document.objects().next().unwrap().geometry(),
            Geometry::PolyCurve(_)
        ));
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    }
}
