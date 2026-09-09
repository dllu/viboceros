//! Group commands share ordered, transactional document membership edits.
use super::*;

pub(super) struct AddToGroupCommand;

impl Command for AddToGroupCommand {
    fn name(&self) -> &'static str {
        "AddToGroup"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() {
            return Err(CommandError::Usage("AddToGroup group-name"));
        }
        let name = arguments.join(" ");
        let group = document
            .group_by_name(&name)
            .map(|group| group.id())
            .ok_or_else(|| CommandError::NamedGroupNotFound(name.clone()))?;
        add_selected_to_group(document, group)
    }
}

impl CommandRegistry {
    /// Execute an already-resolved target pick, including unnamed imported groups.
    pub fn execute_add_to_group(
        &self,
        document: &mut Document,
        group: viboceros_document::GroupId,
    ) -> Result<String, CommandError> {
        run_command_transaction(document, "AddToGroup", |document| {
            add_selected_to_group(document, group)
        })
    }
}

fn add_selected_to_group(
    document: &mut Document,
    group: viboceros_document::GroupId,
) -> Result<String, CommandError> {
    let name = document
        .group(group)
        .ok_or(viboceros_document::DocumentError::GroupNotFound(group))?
        .name()
        .map(str::to_owned)
        .unwrap_or_else(|| group.to_string());
    let members = selected_ids(document)?;
    let count = document.add_group_members(group, members)?;
    document.clear_selection();
    Ok(format!("Added {count} object(s) to group '{name}'"))
}

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
    let changed = if all {
        document.clear_object_group_memberships(selected)?
    } else {
        document.pop_object_group_memberships(selected)?
    };
    Ok(format!(
        "Ungrouped {changed} object(s){}",
        if all {
            " at every level"
        } else {
            " at the top level"
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_to_group_named_target_preserves_order_and_history_and_clears_selection() {
        let mut document = Document::default();
        let ids = (0..3)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let target = document
            .add_group(Some("Target assembly".into()), [ids[0]])
            .unwrap();
        let old = document
            .add_group(Some("old".into()), [ids[1], ids[2]])
            .unwrap();
        document
            .select_objects_direct([ids[1]], SelectionMode::Replace)
            .unwrap();
        let objects = document.objects().cloned().collect::<Vec<_>>();
        let groups = document.groups().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut document, "AddToGroup Target assembly")
            .unwrap();
        assert_eq!(document.object(ids[1]).unwrap().group_ids(), [old, target]);
        assert_eq!(document.object(ids[2]).unwrap().group_ids(), [old]);
        assert_eq!(document.selected_object_count(), 0);
        assert_eq!(document.undo_label(), Some("AddToGroup"));
        let after = format!("{:?}", document.objects().cloned().collect::<Vec<_>>());
        document
            .select_objects_direct([ids[1]], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut document, "AddToGroup Target assembly")
            .unwrap();
        document.undo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
        document.redo().unwrap();
        assert_eq!(
            format!("{:?}", document.objects().cloned().collect::<Vec<_>>()),
            after
        );
    }

    #[test]
    fn add_to_group_rejects_missing_targets_or_selection_without_edits() {
        let mut document = Document::default();
        document.add_empty_group(Some("Target".into())).unwrap();
        document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        document.undo().unwrap();
        let registry = CommandRegistry::with_builtins();
        for command in [
            "AddToGroup",
            "AddToGroup missing",
            "AddToGroup target",
            "AddToGroup Target",
        ] {
            let before = format!("{document:?}");
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}
