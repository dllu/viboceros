//! Source validation remains available when the initial distance has no preview.
use super::*;
use viboceros_document::{GeometrySnapshot, GroupId};
#[derive(Clone, Debug)]
struct Source {
    id: ObjectId,
    geometry: GeometrySnapshot,
    attributes: ObjectAttributes,
    groups: Vec<GroupId>,
    text: BTreeMap<String, String>,
}
#[derive(Clone, Debug)]
pub struct Witness {
    sources: Vec<Source>,
    selected: Vec<ObjectId>,
    tolerance: Tolerance,
}
impl Witness {
    pub fn capture(
        doc: &Document,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Self, CommandError> {
        let mut unique = BTreeSet::new();
        for (i, id) in ids.into_iter().enumerate() {
            if i >= 1_000_000 {
                return Err(too_many_span_outputs("Inset"));
            }
            unique.insert(id);
            if unique.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("Inset"));
            }
        }
        if unique.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let sources = unique
            .into_iter()
            .map(|id| {
                let object = doc
                    .object(id)
                    .filter(|_| doc.is_object_selectable(id))
                    .ok_or(CommandError::InsetStale)?;
                Ok(Source {
                    id,
                    geometry: object.geometry_snapshot().clone(),
                    attributes: object.attributes().clone(),
                    groups: object.group_ids().to_vec(),
                    text: object.geometry_user_text().clone(),
                })
            })
            .collect::<Result<_, CommandError>>()?;
        Ok(Self {
            sources,
            selected: doc.selected_objects().map(|o| o.id()).collect(),
            tolerance: doc.tolerance(),
        })
    }
    pub(super) fn len(&self) -> usize {
        self.sources.len()
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc
                .selected_objects()
                .map(|o| o.id())
                .eq(self.selected.iter().copied())
            && self.sources.iter().all(|s| {
                doc.is_object_selectable(s.id)
                    && doc.object(s.id).is_some_and(|o| {
                        o.geometry_snapshot() == &s.geometry
                            && o.attributes() == &s.attributes
                            && o.group_ids() == s.groups
                            && o.geometry_user_text() == &s.text
                    })
            })
    }
}
