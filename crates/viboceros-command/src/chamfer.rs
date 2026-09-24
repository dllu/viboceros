//! Bevels selected straight curve ends by two distances.

use super::*;
use viboceros_geometry::{try_chamfer_curves_joined, try_chamfer_curves_parts};

const USAGE: &str =
    "Chamfer distance1 distance2 [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No] [Trim=Yes|No]";

pub(super) struct ChamferCommand;

struct ChamferOptions {
    distances: [Real; 2],
    picks: [Option<Point3>; 2],
    join: bool,
    trim: bool,
}

impl Command for ChamferCommand {
    fn name(&self) -> &'static str {
        "Chamfer"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ChamferOptions {
            distances,
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
                        .ok_or(CommandError::ChamferRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::ChamferRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let outputs = if join && trim {
            let joined = try_chamfer_curves_joined(
                first,
                picks[0],
                second,
                picks[1],
                distances[0],
                distances[1],
                document.tolerance(),
            )?;
            let outputs = document.copy_object_pieces_into_source_groups([(
                *first_id,
                Geometry::PolyCurve(joined),
            )])?;
            document.delete_objects([*first_id, *second_id])?;
            outputs
        } else {
            let parts = try_chamfer_curves_parts(
                first,
                picks[0],
                second,
                picks[1],
                distances,
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
        Ok("Chamfered two curves".to_string())
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

fn parse(arguments: &[&str]) -> Result<ChamferOptions, CommandError> {
    if arguments.len() < 2 {
        return Err(CommandError::Usage(USAGE));
    }
    let distances = [
        parse_finite_real(arguments[0])?,
        parse_finite_real(arguments[1])?,
    ];
    let mut picks = [None, None];
    let mut join = None;
    let mut trim = None;
    for argument in &arguments[2..] {
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
    Ok(ChamferOptions {
        distances,
        picks,
        join: join.unwrap_or(true),
        trim: trim.unwrap_or(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chamfer_options_and_undo() {
        let registry = CommandRegistry::with_builtins();
        for (command, count, sources_retained) in [
            ("Chamfer 0.5 1", 1, false),
            ("Chamfer 0 0", 1, false),
            ("Chamfer 0.5 1 Join=No", 3, false),
            ("Chamfer 0.5 1 Trim=No", 3, true),
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
            assert_eq!(document.objects().count(), count, "{command}");
            assert_eq!(
                originals.iter().all(|id| document.object(*id).is_some()),
                sources_retained
            );
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }
}
