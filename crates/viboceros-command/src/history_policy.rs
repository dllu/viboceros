//! Per-command replay policy, shared by scripts and interactive continuations.
use super::CommandError;
use viboceros_document::{
    CopyGroupPolicy, Document, DocumentError, HistoryGroup, ObjectId, ReplacementHistory,
    SelectionMode,
};
use viboceros_geometry::AffineTransform3;

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
    selected: &[ObjectId],
    transform: AffineTransform3,
    copy: bool,
) -> Result<(usize, usize), DocumentError> {
    if !copy && transform == AffineTransform3::identity() {
        return Ok((selected.len(), 0));
    }
    if copy {
        // A single picked source allocates definitions without copying its
        // memberships. Multiple picked sources recreate their group topology.
        let policy = if selected.len() == 1 {
            CopyGroupPolicy::DefinitionsOnly
        } else {
            CopyGroupPolicy::Preserve
        };
        let copies = document.copy_objects_with_transforms_and_groups(
            selected.iter().copied(),
            &[transform],
            policy,
        )?;
        document.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
        Ok((selected.len(), copies.len()))
    } else {
        let transformed = document.transform_objects_with_history(
            selected.iter().copied(),
            transform,
            ReplacementHistory::EveryReplacement,
        )?;
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
