//! Provisional direction changes: Enter accepts once, Escape discards everything.
use super::*;
use viboceros_document::GeometrySnapshot;
#[cfg(test)]
mod tests;

#[derive(Debug)]
struct Source {
    id: ObjectId,
    original: GeometrySnapshot,
    staged: GeometrySnapshot,
}

#[derive(Debug)]
pub struct DirectionSession {
    sources: Vec<Source>,
    tolerance: Tolerance,
    postselected: bool,
}

impl DirectionSession {
    pub fn new(document: &Document, postselected: bool) -> Result<Self, CommandError> {
        let sources = document
            .selected_objects()
            .filter(|o| {
                o.geometry().curve_ref().is_some()
                    || matches!(
                        o.geometry(),
                        Geometry::Brep(_)
                            | Geometry::NurbsSurface(_)
                            | Geometry::Mesh(_)
                            | Geometry::SubD(_)
                    )
            })
            .map(|o| Source {
                id: o.id(),
                original: o.geometry_snapshot().clone(),
                staged: o.geometry_snapshot().clone(),
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        Ok(Self {
            sources,
            tolerance: document.tolerance(),
            postselected,
        })
    }

    pub fn geometries(&self) -> impl Iterator<Item = (ObjectId, &Geometry)> {
        self.sources.iter().map(|s| (s.id, &*s.staged))
    }

    pub fn is_current(&self, document: &Document) -> bool {
        self.tolerance == document.tolerance()
            && self.sources.iter().all(|s| {
                document.is_object_selectable(s.id)
                    && document
                        .object(s.id)
                        .is_some_and(|o| s.original.shares_storage_with(o.geometry_snapshot()))
            })
    }

    /// Prepare the entire batch before changing even the provisional state.
    /// Targeting one object never expands its groups or admits a new object.
    pub fn apply(
        &mut self,
        document: &Document,
        edit: DirectionEdit,
        target: Option<ObjectId>,
    ) -> Result<usize, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::Usage(
                "Dir source changed; restart direction inspection",
            ));
        }
        if target.is_some_and(|id| !self.sources.iter().any(|s| s.id == id)) {
            return Err(CommandError::Usage("click an inspected Dir object"));
        }
        let options = Options { edit, faces: None };
        let mut replacements = Vec::new();
        for (index, source) in self
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| target.is_none_or(|id| s.id == id))
        {
            // SubDs only admit normal reversal. The second Flip restores the
            // original storage directly: kernel archive caches are not a
            // semantic equality witness and must not consume pending Redo.
            if edit == DirectionEdit::Flip
                && matches!(&*source.original, Geometry::SubD(_))
                && !source.original.shares_storage_with(&source.staged)
            {
                replacements.push((index, source.original.clone()));
                continue;
            }
            if let Some(geometry) = edited_geometry(&source.staged, &options, self.tolerance)? {
                // Natural surfaces need an independent face sense after the
                // first edit. Returning to their original chart restores the
                // source representation too, so a double reversal is a no-op.
                let unchanged = if let (Geometry::NurbsSurface(surface), Geometry::Brep(brep)) =
                    (&*source.original, &geometry)
                {
                    Brep::try_surface_face(surface.clone(), self.tolerance)? == *brep
                } else {
                    *source.original == geometry
                };
                replacements.push((
                    index,
                    if unchanged {
                        source.original.clone()
                    } else {
                        geometry.into()
                    },
                ));
            }
        }
        let count = replacements.len();
        for (index, geometry) in replacements {
            self.sources[index].staged = geometry;
        }
        Ok(count)
    }

    pub fn cancel(self, document: &mut Document) -> Result<(), CommandError> {
        self.release_picks(
            document,
            self.sources.len() > 1
                || self
                    .sources
                    .iter()
                    .any(|s| matches!(&*s.original, Geometry::SubD(_))),
        )
    }

    fn release_picks(&self, document: &mut Document, force: bool) -> Result<(), CommandError> {
        if self.postselected || force {
            let sources = self.sources.iter().map(|s| s.id).collect::<BTreeSet<_>>();
            let retain = document
                .selected_object_ids()
                .filter(|id| !sources.contains(id))
                .collect::<Vec<_>>();
            document.set_transient_selection(retain)?;
        }
        Ok(())
    }

    pub fn accept(self, document: &mut Document) -> Result<usize, CommandError> {
        self.accept_with_history(document, true)
    }

    pub(super) fn accept_in_transaction(
        self,
        document: &mut Document,
    ) -> Result<usize, CommandError> {
        self.accept_with_history(document, false)
    }

    fn accept_with_history(
        self,
        document: &mut Document,
        own_transaction: bool,
    ) -> Result<usize, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::Usage(
                "Dir source changed; restart direction inspection",
            ));
        }
        let replacements = self
            .sources
            .iter()
            .filter(|s| *s.original != *s.staged)
            .map(|s| (s.id, (*s.staged).clone()))
            .collect::<Vec<_>>();
        let count = replacements.len();
        if count > 0 {
            let selected = document.selected_object_ids().collect::<Vec<_>>();
            let changed = replacements
                .iter()
                .map(|(id, _)| *id)
                .collect::<BTreeSet<_>>();
            if own_transaction {
                document.begin_transaction("Dir")?;
            }
            let result = (|| -> Result<(), CommandError> {
                // Unlike a single B-rep, a SubD retains preselection on Undo.
                if self.postselected
                    || (self.sources.len() == 1
                        && !matches!(&*self.sources[0].original, Geometry::SubD(_)))
                {
                    document.select_command_results(
                        selected.iter().copied().filter(|id| !changed.contains(id)),
                    )?;
                }
                document.replace_object_geometries(replacements)?;
                if !self.postselected && self.sources.len() == 1 {
                    document.select_command_results(selected)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                if own_transaction {
                    document.rollback_transaction()?;
                }
                return Err(error);
            }
            if own_transaction {
                document.commit_transaction()?;
            }
        }
        self.release_picks(document, false)?;
        Ok(count)
    }
}
