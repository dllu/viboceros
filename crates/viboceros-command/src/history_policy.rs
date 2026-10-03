//! Per-command replay policy, shared by scripts and interactive continuations.
use super::CommandError;
use viboceros_document::{
    CopyGroupPolicy, Document, DocumentError, HistoryGroup, ObjectId, ReplacementHistory,
};
use viboceros_geometry::AffineTransform3;

/// Source order belongs to GetObject for command-first picks, and to the
/// object table for preselection. Explicit sources also survive Mirror Object
/// clearing the live selection while its plane target is being picked.
pub(super) struct TransformSources {
    pub(super) ids: Vec<ObjectId>,
    pub(super) postselected: bool,
    pub(super) release_on_replay: bool,
}

impl TransformSources {
    /// Copy releases even preselected sources when its history is replayed.
    pub(super) fn release_selection_on_replay(&mut self) {
        self.release_on_replay = true;
    }
}

pub(super) fn transform_arguments<'a>(
    document: &Document,
    arguments: &[&'a str],
    usage: &'static str,
) -> Result<(Vec<&'a str>, TransformSources), CommandError> {
    let mut positional = Vec::new();
    let mut sources = None;
    for argument in arguments {
        if let Some((name, value)) = argument.split_once('=')
            && (super::option_name_eq(name, "Sources")
                || super::option_name_eq(name, "PickedSources"))
        {
            if sources.is_some() {
                return Err(CommandError::Usage(usage));
            }
            let ids = value
                .split(',')
                .map(|id| id.parse::<ObjectId>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CommandError::Usage(usage))?;
            let unique = ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            if ids.is_empty() || unique.len() != ids.len() {
                return Err(CommandError::Usage(usage));
            }
            let postselected = super::option_name_eq(name, "PickedSources");
            let mut remaining = unique;
            for object in document.selectable_objects() {
                remaining.remove(&object.id());
            }
            // A picked group's restricted peers are editable while selected,
            // under the same document policy as ordinary preselection.
            for id in document.selected_object_ids() {
                remaining.remove(&id);
            }
            if !remaining.is_empty() {
                return Err(CommandError::Usage(usage));
            }
            sources = Some(TransformSources {
                ids,
                postselected,
                release_on_replay: postselected,
            });
        } else {
            positional.push(*argument);
        }
    }
    let sources = match sources {
        Some(sources) => sources,
        None => TransformSources {
            ids: transform_source_ids(document)?,
            postselected: false,
            release_on_replay: false,
        },
    };
    Ok((positional, sources))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CommandHistoryPolicy {
    #[default]
    Ordinary,
    /// Native affine commands retain copied group definitions on Undo and
    /// renew creation order when replacing original object geometry.
    TransformedObjects,
}

impl CommandHistoryPolicy {
    pub(super) fn configure(self, group: &mut HistoryGroup) {
        if self == Self::TransformedObjects {
            group.keep_created_group_definitions();
            group.renew_changed_object_order();
        }
    }
}

pub(super) fn apply_transform_with_renewal(
    document: &mut Document,
    sources: &TransformSources,
    transform: AffineTransform3,
    copy: bool,
) -> Result<(usize, usize), DocumentError> {
    let selected = &sources.ids;
    if !copy && transform == AffineTransform3::identity() {
        return Ok((selected.len(), 0));
    }
    if sources.release_on_replay {
        document.release_command_selection_on_history_replay(selected.iter().copied())?;
    }
    if copy {
        // A single picked source allocates definitions without copying its
        // memberships. Multiple picked sources recreate their group topology.
        let policy = if selected.len() == 1 {
            CopyGroupPolicy::DefinitionsOnly
        } else {
            CopyGroupPolicy::Preserve
        };
        let copies = if sources.postselected {
            document.copy_objects_with_transforms_and_groups_in_order(
                selected.iter().copied(),
                &[transform],
                policy,
            )?
        } else {
            document.copy_objects_with_transforms_and_groups(
                selected.iter().copied(),
                &[transform],
                policy,
            )?
        };
        document.select_command_results(selected.iter().copied())?;
        Ok((selected.len(), copies.len()))
    } else {
        let transformed = document.transform_objects_with_history(
            selected.iter().copied(),
            transform,
            ReplacementHistory::EveryReplacement,
        )?;
        document.move_objects_to_end_in_order(selected.iter().copied())?;
        if sources.postselected {
            document.clear_selection();
        }
        Ok((transformed, 0))
    }
}

pub(super) fn transform_source_ids(document: &Document) -> Result<Vec<ObjectId>, CommandError> {
    // Native preselection enumerates the live object table, independently of
    // the order in which the objects were selected.
    let selected = document
        .objects()
        .filter(|object| document.is_selected(object.id()))
        .map(|object| object.id())
        .collect::<Vec<_>>();
    if selected.is_empty() {
        Err(CommandError::NoObjectsSelected)
    } else {
        Ok(selected)
    }
}
