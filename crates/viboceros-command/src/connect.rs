//! Extends or trims selected curve ends until they meet.

use super::*;
use viboceros_geometry::{try_fillet_curves_joined, try_fillet_curves_parts};

const USAGE: &str = "Connect [Pick1=x,y,z] [Pick2=x,y,z] [Join=Yes|No]";

pub(super) struct ConnectCommand;

struct ConnectOptions {
    picks: [Option<Point3>; 2],
    join: bool,
}

impl Command for ConnectCommand {
    fn name(&self) -> &'static str {
        "Connect"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ConnectOptions { picks, join } = parse(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| {
                Ok((
                    object.id(),
                    object
                        .geometry()
                        .curve_ref()
                        .ok_or(CommandError::ConnectRequiresTwoCurves)?
                        .to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let [(first_id, first), (second_id, second)] = selected.as_slice() else {
            return Err(CommandError::ConnectRequiresTwoCurves);
        };
        let (default_first, default_second) = super::fillet::nearest_ends(first, second)?;
        let picks = [
            picks[0].unwrap_or(default_first),
            picks[1].unwrap_or(default_second),
        ];
        let outputs = if join {
            let joined = try_fillet_curves_joined(
                first,
                picks[0],
                second,
                picks[1],
                0.0,
                document.tolerance(),
            )?;
            document
                .copy_object_pieces_into_source_groups([(*first_id, Geometry::PolyCurve(joined))])?
        } else {
            let parts = try_fillet_curves_parts(
                first,
                picks[0],
                second,
                picks[1],
                0.0,
                true,
                document.tolerance(),
            )?;
            document.copy_object_pieces_into_source_groups(
                [*first_id, *second_id]
                    .into_iter()
                    .zip(parts.into_iter().map(Geometry::from)),
            )?
        };
        document.delete_objects([*first_id, *second_id])?;
        document.select_objects_direct(outputs, SelectionMode::Replace)?;
        Ok("Connected two curves".to_string())
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

fn parse(arguments: &[&str]) -> Result<ConnectOptions, CommandError> {
    let mut picks = [None, None];
    let mut join = None;
    for argument in arguments {
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
    Ok(ConnectOptions {
        picks,
        join: join.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::{CircularArc3, LineSegment};

    #[test]
    fn connects_extended_lines_with_or_without_join_and_undo() {
        let registry = CommandRegistry::with_builtins();
        for (command, expected_count) in [("Connect", 2), ("Connect Join=Yes", 1)] {
            let mut document = Document::default();
            registry.execute(&mut document, "Line 0,0,0 3,0,0").unwrap();
            registry.execute(&mut document, "Line 4,1,0 4,4,0").unwrap();
            registry.execute(&mut document, "SelAll").unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut document, command).unwrap();
            assert_eq!(document.objects().count(), expected_count);
            if expected_count == 2 {
                assert!(
                    document
                        .objects()
                        .all(|object| matches!(object.geometry(), Geometry::Line(_)))
                );
            } else {
                assert!(matches!(
                    document.objects().next().unwrap().geometry(),
                    Geometry::PolyCurve(_)
                ));
            }
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        }
    }

    #[test]
    fn already_meeting_arc_and_line_keep_native_geometry() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let diagonal = 2.0_f64.sqrt() / 2.0;
        let arc = CircularArc3::try_from_three_points(
            p(1., 0.),
            p(diagonal, diagonal),
            p(0., 1.),
            document.tolerance(),
        )
        .unwrap();
        let line = LineSegment::try_new(p(0., 1.), p(-2., 1.), document.tolerance()).unwrap();
        let first = document.add_geometry(Geometry::Arc(arc)).unwrap();
        let second = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_objects_direct([first, second], SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Connect").unwrap();
        assert_eq!(document.objects().count(), 2);
        assert!(
            document
                .objects()
                .any(|object| matches!(object.geometry(), Geometry::Arc(_)))
        );
        assert!(
            document
                .objects()
                .any(|object| matches!(object.geometry(), Geometry::Line(_)))
        );
        assert!(document.object(first).is_none() && document.object(second).is_none());
    }
}
