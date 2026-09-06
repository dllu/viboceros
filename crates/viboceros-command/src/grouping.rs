//! Group commands share ordered, transactional document membership edits.
use super::*;

pub(super) struct GroupCommand;

impl Command for GroupCommand {
    fn name(&self) -> &'static str {
        "Group"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let all = arguments
            .first()
            .is_some_and(|argument| argument.eq_ignore_ascii_case("all"));
        let name_arguments = if all { &arguments[1..] } else { arguments };
        let name = if name_arguments.is_empty() {
            document.next_unused_group_name()
        } else {
            name_arguments.join(" ")
        };
        let members: Vec<_> = if all {
            document
                .objects()
                .filter(|object| document.is_object_selectable(object.id()))
                .map(|object| object.id())
                .collect()
        } else {
            document.selected_object_ids().collect()
        };
        let member_count = members.len();
        let id = document.add_group(Some(name.clone()), members)?;
        Ok(format!(
            "Created group '{name}' {id} with {member_count} object(s)"
        ))
    }
}

pub(super) struct UngroupCommand;

impl Command for UngroupCommand {
    fn name(&self) -> &'static str {
        "Ungroup"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return ungroup_selected(document, false);
        }
        if arguments.len() == 1 && arguments[0].eq_ignore_ascii_case("all") {
            let groups: Vec<_> = document.groups().map(|group| group.id()).collect();
            for group in &groups {
                document.remove_group(*group)?;
            }
            return Ok(format!("Removed {} group(s)", groups.len()));
        }

        let name = arguments.join(" ");
        let id = document
            .group_by_name(&name)
            .map(|group| group.id())
            .ok_or_else(|| CommandError::NamedGroupNotFound(name.clone()))?;
        let members = document.remove_group(id)?;
        Ok(format!("Removed group '{name}' ({members} object(s))"))
    }
}

pub(super) struct UngroupAllCommand;

impl Command for UngroupAllCommand {
    fn name(&self) -> &'static str {
        "UngroupAll"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "UngroupAll")?;
        ungroup_selected(document, true)
    }
}

fn ungroup_selected(document: &mut Document, all: bool) -> Result<String, CommandError> {
    let selected = selected_ids(document)?;
    let mut changed = 0;
    for id in selected {
        let mut memberships = document
            .object(id)
            .expect("selected object")
            .group_ids()
            .to_vec();
        if all {
            memberships.clear();
        } else {
            memberships.pop();
        }
        changed += usize::from(document.set_object_group_memberships(id, memberships)?);
    }
    Ok(format!(
        "Ungrouped {changed} object(s){}",
        if all {
            " at every level"
        } else {
            " at the top level"
        }
    ))
}
