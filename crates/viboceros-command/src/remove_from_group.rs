//! Detach selected group members, optionally retaining grouped originals.
use super::*;

const USAGE: &str = "RemoveFromGroup [Copy=Yes|No]";
pub(super) struct RemoveFromGroupCommand;

fn copy_option(arguments: &[&str]) -> Result<bool, CommandError> {
    if arguments.is_empty() {
        return Ok(false);
    }
    let (name, value, consumed) = orient_option(arguments, 0, USAGE)?;
    require_consumed(arguments, consumed, USAGE)?;
    if !option_name_eq(name, "Copy") {
        return Err(CommandError::Usage(USAGE));
    }
    parse_yes_no(value).ok_or(CommandError::Usage(USAGE))
}

impl Command for RemoveFromGroupCommand {
    fn name(&self) -> &'static str {
        "RemoveFromGroup"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let copy = copy_option(arguments)?;
        if document.selected_object_count() == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        let sources = document
            .selected_objects()
            .filter(|object| !object.group_ids().is_empty())
            .map(|object| object.id())
            .collect::<Vec<_>>();
        let count = sources.len();
        if count == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if copy {
            document.copy_objects_with_transforms_and_groups(
                sources,
                &[AffineTransform3::identity()],
                viboceros_document::CopyGroupPolicy::Omit,
            )?;
        } else {
            for id in sources {
                document.set_object_group_memberships(id, [])?;
            }
        }
        Ok(format!(
            "{} {count} object(s) from groups",
            if copy { "Copied" } else { "Removed" }
        ))
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Grouped,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![BooleanSelectionOption {
                name: "Copy",
                value: copy_option(arguments)?,
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_and_copy_preserve_peers_attributes_and_undo() {
        for copy in [false, true] {
            let mut document = Document::default();
            let ids = (0..3)
                .map(|i| {
                    document
                        .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            document.add_group(None, [ids[0], ids[1]]).unwrap();
            document.add_group(None, [ids[1], ids[2]]).unwrap();
            document
                .select_objects_direct([ids[1]], SelectionMode::Replace)
                .unwrap();
            let before = document.objects().cloned().collect::<Vec<_>>();
            let groups = document.groups().cloned().collect::<Vec<_>>();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(
                    &mut document,
                    if copy {
                        "RemoveFromGroup Copy=Yes"
                    } else {
                        "RemoveFromGroup Copy=No"
                    },
                )
                .unwrap();
            assert_eq!(document.selected_object_count(), 1);
            let selected = document.selected_objects().next().unwrap();
            assert!(selected.group_ids().is_empty());
            assert_eq!(selected.geometry(), before[1].geometry());
            assert_eq!(selected.attributes(), before[1].attributes());
            assert_eq!(selected.id() != ids[1], copy);
            assert_eq!(document.object(ids[0]).unwrap(), &before[0]);
            assert_eq!(document.object(ids[2]).unwrap(), &before[2]);
            if copy {
                assert_eq!(document.object(ids[1]).unwrap(), &before[1]);
            }
            let after = document.objects().cloned().collect::<Vec<_>>();
            document.undo().unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
            document.redo().unwrap();
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
        }
    }

    #[test]
    fn invalid_options_and_ungrouped_selection_do_not_lose_redo() {
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
            .unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        document
            .add_geometry(Geometry::Point(Point3::try_new(1., 0., 0.).unwrap()))
            .unwrap();
        document.undo().unwrap();
        let registry = CommandRegistry::with_builtins();
        for command in [
            "RemoveFromGroup",
            "RemoveFromGroup Copy=maybe",
            "RemoveFromGroup Copy=Yes Copy=No",
            "RemoveFromGroup Bogus=Yes",
        ] {
            let before = format!("{document:?}");
            assert!(registry.execute(&mut document, command).is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}
