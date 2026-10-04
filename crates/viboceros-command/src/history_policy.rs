//! Per-command replay policy, shared by scripts and interactive continuations.
use super::CommandError;
use viboceros_document::{
    ControlPointId, CopyGroupPolicy, Document, DocumentError, HistoryGroup, ObjectId,
    ReplacementHistory,
};
use viboceros_geometry::AffineTransform3;

/// Source order belongs to GetObject for command-first picks, and to the
/// object table for preselection. Explicit sources also survive Mirror Object
/// clearing the live selection while its plane target is being picked.
pub(super) struct TransformSources {
    pub(super) ids: Vec<ObjectId>,
    pub(super) grips: Vec<ControlPointId>,
    pub(super) postselected: bool,
    pub(super) release_on_replay: bool,
}

impl TransformSources {
    /// Whole-object Copy releases preselected sources on replay. The shared
    /// application path applies the separate grip Copy retention policy.
    pub(super) fn release_selection_on_replay(&mut self) {
        self.release_on_replay = true;
    }
}

pub(super) fn transform_arguments<'a>(
    document: &Document,
    arguments: &[&'a str],
    usage: &'static str,
) -> Result<(Vec<&'a str>, TransformSources), CommandError> {
    source_arguments(document, arguments, usage, false)
}

pub(super) fn affine_transform_arguments<'a>(
    document: &Document,
    arguments: &[&'a str],
    usage: &'static str,
) -> Result<(Vec<&'a str>, TransformSources), CommandError> {
    source_arguments(document, arguments, usage, true)
}

fn source_arguments<'a>(
    document: &Document,
    arguments: &[&'a str],
    usage: &'static str,
    allow_grips: bool,
) -> Result<(Vec<&'a str>, TransformSources), CommandError> {
    let mut positional = Vec::new();
    let mut sources = None;
    let mut grips = None;
    let mut grips_postselected = false;
    for argument in arguments {
        if allow_grips
            && let Some((name, value)) = argument.split_once('=')
            && (super::option_name_eq(name, "GripSources")
                || super::option_name_eq(name, "PickedGripSources"))
        {
            if grips.is_some() {
                return Err(CommandError::Usage(usage));
            }
            let picks = value
                .split(',')
                .map(|value| {
                    let (id, index) = value.rsplit_once(':').ok_or(CommandError::Usage(usage))?;
                    let pick = ControlPointId {
                        object: id.parse().map_err(|_| CommandError::Usage(usage))?,
                        index: index.parse().map_err(|_| CommandError::Usage(usage))?,
                    };
                    if document
                        .control_point_locations(pick.object)
                        .is_none_or(|p| pick.index >= p.len())
                    {
                        return Err(CommandError::Usage(usage));
                    }
                    Ok(pick)
                })
                .collect::<Result<std::collections::BTreeSet<_>, CommandError>>()?;
            grips_postselected = super::option_name_eq(name, "PickedGripSources");
            grips = Some(picks.into_iter().collect::<Vec<_>>());
            continue;
        }
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
                grips: Vec::new(),
                postselected,
                release_on_replay: postselected,
            });
        } else {
            positional.push(*argument);
        }
    }
    let mut sources = match sources {
        Some(sources) => sources,
        None => TransformSources {
            ids: if allow_grips {
                document
                    .objects()
                    .filter(|o| document.is_selected(o.id()))
                    .map(|o| o.id())
                    .collect()
            } else {
                transform_source_ids(document)?
            },
            grips: Vec::new(),
            postselected: false,
            release_on_replay: false,
        },
    };
    if allow_grips {
        sources.grips = grips.unwrap_or_else(|| {
            document
                .selected_control_points()
                .map(|(id, _)| id)
                .collect()
        });
        sources.postselected |= grips_postselected;
        sources.release_on_replay |= grips_postselected;
        if sources.ids.is_empty() && sources.grips.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
    }
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
        let owners = selected
            .iter()
            .copied()
            .chain(sources.grips.iter().map(|p| p.object))
            .collect::<std::collections::BTreeSet<_>>();
        return Ok((owners.len(), 0));
    }
    // A grip Copy retains its preselected object peers through Undo/Redo.
    if sources.release_on_replay && (sources.grips.is_empty() || sources.postselected) {
        document.release_command_selection_on_history_replay(selected.iter().copied())?;
    }
    if !sources.grips.is_empty() {
        let owners = selected
            .iter()
            .copied()
            .chain(sources.grips.iter().map(|p| p.object))
            .collect::<std::collections::BTreeSet<_>>();
        let policy = if sources.postselected && owners.len() == 1 {
            CopyGroupPolicy::DefinitionsOnly
        } else {
            CopyGroupPolicy::Preserve
        };
        let (changed, copies) = document.transform_objects_and_grips(
            selected.iter().copied(),
            sources.grips.iter().copied(),
            transform,
            copy,
            policy,
        )?;
        if copy {
            document.select_command_results(selected.iter().copied())?;
        } else if sources.postselected {
            document.select_command_results([])?;
        }
        return Ok((changed.len(), copies.len()));
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
