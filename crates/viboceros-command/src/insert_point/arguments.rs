//! Script coordinates and explicit edge/parameter tokens share the staged session.
use super::*;
pub const USAGE: &str = "InsertPoint (point|Edge=index Parameter=number) [Undo ...] [Enter|Cancel]";
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InsertAction {
    Point(Point3),
    Edge(PolygonEdgePoint),
    Undo,
    Finish,
    Cancel,
}

pub fn parse_actions(args: &[&str]) -> Result<Vec<InsertAction>, CommandError> {
    if args.is_empty() {
        return Err(CommandError::Usage(USAGE));
    }
    let mut actions = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let token = args[i].trim_start_matches('_');
        if token.eq_ignore_ascii_case("Undo") {
            actions.push(InsertAction::Undo);
            i += 1;
            continue;
        }
        if token.eq_ignore_ascii_case("Cancel") || token.eq_ignore_ascii_case("Enter") {
            if i + 1 != args.len() {
                return Err(CommandError::Usage(USAGE));
            }
            actions.push(if token.eq_ignore_ascii_case("Cancel") {
                InsertAction::Cancel
            } else {
                InsertAction::Finish
            });
            break;
        }
        if token
            .split_once('=')
            .is_some_and(|(k, _)| k.eq_ignore_ascii_case("Edge"))
            || token.eq_ignore_ascii_case("Edge")
        {
            let edge = option(args, &mut i, "Edge")?
                .parse::<usize>()
                .map_err(|_| CommandError::Usage(USAGE))?;
            let parameter = option(args, &mut i, "Parameter")?
                .parse::<Real>()
                .map_err(|_| CommandError::Usage(USAGE))?;
            actions.push(InsertAction::Edge(PolygonEdgePoint { edge, parameter }));
        } else {
            let (point, used) = parse_point(&args[i..])?;
            i += used;
            actions.push(InsertAction::Point(point));
        }
    }
    Ok(actions)
}

pub(super) fn run(document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
    let actions = parse_actions(args)?;
    let ids = document.selected_object_ids().collect::<Vec<_>>();
    let [object] = ids.as_slice() else {
        return Err(CommandError::InsertPointUnavailable);
    };
    let mut session = InsertSession::new(document, *object)?;
    for action in actions {
        match action {
            InsertAction::Point(point) => {
                let pick = session.pick_point(point)?;
                session.insert(document, pick, None)?;
            }
            InsertAction::Edge(pick) => session.insert(document, pick, None)?,
            InsertAction::Undo => {
                session.undo_point(document)?;
            }
            InsertAction::Finish => break,
            InsertAction::Cancel => {
                document.clear_selection();
                return Ok("InsertPoint cancelled".into());
            }
        }
    }
    session.apply(document)
}
fn option<'a>(args: &[&'a str], index: &mut usize, name: &str) -> Result<&'a str, CommandError> {
    let token = args
        .get(*index)
        .ok_or(CommandError::Usage(USAGE))?
        .trim_start_matches('_');
    if let Some((key, value)) = token.split_once('=') {
        if !key.eq_ignore_ascii_case(name) {
            return Err(CommandError::Usage(USAGE));
        }
        *index += 1;
        Ok(value)
    } else {
        if !token.eq_ignore_ascii_case(name) {
            return Err(CommandError::Usage(USAGE));
        }
        let value = *args.get(*index + 1).ok_or(CommandError::Usage(USAGE))?;
        *index += 2;
        Ok(value)
    }
}
