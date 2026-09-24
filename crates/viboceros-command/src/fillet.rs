//! Joins two selected open curves with a tangent circular fillet.

use super::*;
use viboceros_geometry::{Curve3, try_fillet_curves_joined, try_fillet_curves_parts};

const USAGE: &str = "Fillet radius [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [Trim=Yes|No]";

pub(super) struct FilletCommand;

struct FilletOptions {
    radius: Real,
    picks: [Option<Point3>; 2],
    join: bool,
    trim: bool,
}

impl Command for FilletCommand {
    fn name(&self) -> &'static str {
        "Fillet"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let FilletOptions {
            radius,
            picks,
            join,
            trim,
        } = parse(arguments)?;
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
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let outputs = if join && trim {
            let joined = try_fillet_curves_joined(
                first,
                picks[0],
                second,
                picks[1],
                radius,
                document.tolerance(),
            )?;
            let outputs = document.copy_object_pieces_into_source_groups([(
                *first_id,
                Geometry::PolyCurve(joined),
            )])?;
            document.delete_objects([*first_id, *second_id])?;
            outputs
        } else {
            let parts = try_fillet_curves_parts(
                first,
                picks[0],
                second,
                picks[1],
                radius,
                trim,
                document.tolerance(),
            )?;
            let pieces = parts.into_iter().enumerate().map(|(index, curve)| {
                (
                    if index == 1 && trim {
                        *second_id
                    } else {
                        *first_id
                    },
                    Geometry::from(curve),
                )
            });
            let outputs = document.copy_object_pieces_into_source_groups(pieces)?;
            if trim {
                document.delete_objects([*first_id, *second_id])?;
            }
            outputs
        };
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

fn parse(arguments: &[&str]) -> Result<FilletOptions, CommandError> {
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
    let mut join = None;
    let mut trim = None;
    for argument in &arguments[1..] {
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if option_name_eq(name, "Join") {
            if join
                .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                .is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
        if option_name_eq(name, "Trim") {
            if trim
                .replace(parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?)
                .is_some()
            {
                return Err(CommandError::Usage(USAGE));
            }
            continue;
        }
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
    Ok(FilletOptions {
        radius,
        picks,
        join: join.unwrap_or(true),
        trim: trim.unwrap_or(true),
    })
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

    #[test]
    fn join_and_trim_options_control_sources_and_arc() {
        let registry = CommandRegistry::with_builtins();
        for (command, expected_count, originals_retained, arc_count) in [
            ("Fillet 0.5 Join=No Trim=Yes", 3, false, 1),
            ("Fillet 0.5 Join=No Trim=No", 3, true, 1),
            ("Fillet 0.5 Join=Yes Trim=No", 3, true, 1),
            ("Fillet 0 Join=No Trim=Yes", 2, false, 0),
        ] {
            let mut document = Document::default();
            registry.execute(&mut document, "Line 0,0,0 4,0,0").unwrap();
            registry.execute(&mut document, "Line 4,0,0 4,4,0").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            let originals = document
                .objects()
                .map(|object| object.id())
                .collect::<Vec<_>>();
            let before = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, command).unwrap();
            assert_eq!(document.objects().count(), expected_count, "{command}");
            assert_eq!(
                originals.iter().all(|id| document.object(*id).is_some()),
                originals_retained,
                "{command}"
            );
            assert_eq!(
                document
                    .objects()
                    .filter(|object| matches!(object.geometry(), Geometry::Arc(_)))
                    .count(),
                arc_count,
                "{command}"
            );
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
}
