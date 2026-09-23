//! Select hidden or locked objects without changing their state during picking.
use super::*;

#[cfg(test)]
mod tests;

const SHOW_USAGE: &str = "ShowSelected Ids=<id[,id...]>";
const UNLOCK_USAGE: &str = "UnlockSelected Ids=<id[,id...]>";

pub(super) struct VisibilitySelectionCommand {
    pub unlock: bool,
}

impl VisibilitySelectionCommand {
    fn usage(&self) -> &'static str {
        if self.unlock {
            UNLOCK_USAGE
        } else {
            SHOW_USAGE
        }
    }
}

impl Command for VisibilitySelectionCommand {
    fn name(&self) -> &'static str {
        if self.unlock {
            "UnlockSelected"
        } else {
            "ShowSelected"
        }
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if !arguments.is_empty() {
            parse_ids(arguments, self.usage())?;
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: if self.unlock {
                ObjectSelectionFilter::LockedObjects
            } else {
                ObjectSelectionFilter::HiddenObjects
            },
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let ids = parse_ids(arguments, self.usage())?;
        let mut eligible = Vec::with_capacity(ids.len());
        for id in ids {
            let object = document
                .object(id)
                .ok_or(DocumentError::ObjectNotFound(id))?;
            let attributes = object.attributes();
            if if self.unlock {
                attributes.is_locked()
            } else {
                !attributes.is_visible()
            } {
                eligible.push(id);
            }
        }
        let changed = if self.unlock {
            document.set_objects_locked(eligible, false)?
        } else {
            document.set_objects_visibility(eligible, true)?
        };
        Ok(format!(
            "{} {changed} object(s)",
            if self.unlock { "Unlocked" } else { "Showed" }
        ))
    }
}

fn parse_ids(arguments: &[&str], usage: &'static str) -> Result<BTreeSet<ObjectId>, CommandError> {
    let [argument] = arguments else {
        return Err(CommandError::Usage(usage));
    };
    let (name, values) = argument.split_once('=').ok_or(CommandError::Usage(usage))?;
    if !option_name_eq(name, "Ids") {
        return Err(CommandError::Usage(usage));
    }
    let ids = values
        .split(',')
        .map(|value| value.parse().map_err(|_| CommandError::Usage(usage)))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if ids.is_empty() {
        return Err(CommandError::Usage(usage));
    }
    Ok(ids)
}
