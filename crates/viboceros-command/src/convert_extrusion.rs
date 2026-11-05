//! Explicit B-rep conversion of retained lightweight extrusions.
use super::*;
#[cfg(test)]
mod tests;

const USAGE: &str = "ConvertExtrusion [DeleteInput=Yes|No]";
const MAX_SOURCES: usize = 100_000;
pub(super) struct ConvertExtrusionCommand;

fn parse(arguments: &[&str]) -> Result<bool, CommandError> {
    if arguments.is_empty() {
        Ok(true)
    } else {
        parse_delete_input(arguments, USAGE, &["DeleteInput"])
    }
}
impl Command for ConvertExtrusionCommand {
    fn name(&self) -> &'static str {
        "ConvertExtrusion"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Extrusions,
            options: vec![BooleanSelectionOption {
                name: "DeleteInput",
                value: parse(args)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
            workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
        }))
    }
    fn object_selection_confirmation(
        &self,
        _doc: &Document,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(args)
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        parse(args).map(|_| ())
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        convert(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        convert(doc, args, true)
    }
}

fn convert(doc: &mut Document, args: &[&str], postselected: bool) -> Result<String, CommandError> {
    let delete = parse(args)?;
    let selected = doc
        .selected_objects()
        .filter(|object| matches!(object.geometry(),Geometry::Brep(brep) if brep.is_extrusion()))
        .take(MAX_SOURCES + 1)
        .collect::<Vec<_>>();
    if selected.len() > MAX_SOURCES {
        return Err(CommandError::Usage(
            "ConvertExtrusion supports at most 100000 sources",
        ));
    }
    let mut sources = selected
        .into_iter()
        .map(|object| match object.geometry() {
            Geometry::Brep(brep) if brep.is_extrusion() => Ok((
                object.id(),
                Geometry::Brep(brep.try_extrusion_brep_form(doc.tolerance())?),
            )),
            _ => unreachable!("extrusion filter above"),
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let ranks = if postselected {
        doc.selected_object_ids()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect::<BTreeMap<_, _>>()
    } else {
        doc.objects()
            .enumerate()
            .map(|(i, o)| (o.id(), i))
            .collect()
    };
    sources.sort_unstable_by_key(|(id, _)| ranks[id]);
    let count = sources.len();
    if postselected {
        let picks = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(picks)?;
        doc.clear_selection();
    }
    if delete {
        doc.replace_object_geometries_in_order(sources)?;
    } else {
        doc.copy_object_pieces_into_source_groups(sources)?;
    }
    Ok(format!(
        "Converted {count} extrusion object(s) to explicit B-reps{}",
        if delete { "" } else { "; inputs retained" }
    ))
}
