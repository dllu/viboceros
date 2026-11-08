use super::*;
use viboceros_document::{Object, ReplacementHistory};

pub struct Prepared {
    sources: Vec<Object>,
    result: Brep,
    postselected: bool,
    tolerance: Tolerance,
    units: viboceros_geometry::LengthUnitSystem,
}

fn sources(document: &Document, postselected: bool) -> Vec<Object> {
    if postselected {
        document
            .selected_objects()
            .filter(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
            .cloned()
            .collect()
    } else {
        document
            .objects()
            .filter(|o| {
                document.is_selected(o.id()) && ObjectSelectionFilter::Surfaces.accepts_object(o)
            })
            .cloned()
            .collect()
    }
}

pub fn prepare(
    document: &Document,
    options: Options,
    postselected: bool,
) -> Result<Prepared, CommandError> {
    let sources = sources(document, postselected);
    if sources.len() != 2
        || sources
            .iter()
            .any(|o| !document.is_object_selectable(o.id()))
    {
        return Err(CommandError::Usage(
            "Select exactly two editable untrimmed surfaces",
        ));
    }
    let result = merge(
        sources[0].geometry(),
        sources[1].geometry(),
        options,
        document.tolerance(),
    )?;
    Ok(Prepared {
        sources,
        result,
        postselected,
        tolerance: document.tolerance(),
        units: document.units().clone(),
    })
}

impl Prepared {
    pub fn geometry(&self) -> &Brep {
        &self.result
    }
    pub fn is_current(&self, document: &Document) -> bool {
        document.tolerance() == self.tolerance
            && document.units() == &self.units
            && sources(document, self.postselected)
                .iter()
                .map(Object::id)
                .eq(self.sources.iter().map(Object::id))
            && self.sources.iter().all(|old| {
                document.is_object_selectable(old.id())
                    && document.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
    pub fn commit(&self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "MergeSrf", |d| self.apply(d))
    }
    pub(super) fn apply(&self, document: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(document) {
            return Err(CommandError::Usage("MergeSrf prepared inputs changed"));
        }
        if self.postselected {
            document
                .release_command_selection_on_history_replay(self.sources.iter().map(Object::id))?;
            document.clear_selection();
        }
        document.replace_object_geometries_with_history(
            [(self.sources[0].id(), Geometry::Brep(self.result.clone()))],
            ReplacementHistory::EveryReplacement,
        )?;
        document.delete_objects([self.sources[1].id()])?;
        Ok("Merged two surfaces".into())
    }
}
