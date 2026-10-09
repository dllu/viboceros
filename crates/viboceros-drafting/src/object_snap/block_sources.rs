//! Read-only member records with cache identities distinct from root ownership.
use super::*;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};
use viboceros_document::{GeometrySnapshot, Object};

#[derive(Debug, Default)]
pub(super) struct BlockSources {
    entries: HashMap<ObjectId, CachedBlock>,
    #[cfg(test)]
    pub(super) builds: usize,
}

#[derive(Debug)]
struct CachedBlock {
    source: GeometrySnapshot,
    insertions: Vec<Rc<Object>>,
    members: Vec<Rc<Object>>,
}

pub(super) enum QueryObject<'a> {
    Document(&'a Object),
    Derived(Rc<Object>),
}

impl QueryObject<'_> {
    pub(super) fn object(&self) -> &Object {
        match self {
            Self::Document(object) => object,
            Self::Derived(object) => object,
        }
    }
}

pub(super) struct SnapSource<'a> {
    pub(super) object: QueryObject<'a>,
    pub(super) owner: ObjectId,
    pub(super) visible: bool,
}

pub(super) struct QuerySources<'a> {
    ordinary: Option<&'a Document>,
    visible_layers: HashSet<viboceros_document::LayerId>,
    expanded: Vec<SnapSource<'a>>,
}

pub(super) struct SnapSourceRef<'a> {
    pub(super) object: &'a Object,
    pub(super) owner: ObjectId,
    pub(super) visible: bool,
}

impl QuerySources<'_> {
    pub(super) fn iter(&self) -> impl Iterator<Item = SnapSourceRef<'_>> {
        self.ordinary
            .into_iter()
            .flat_map(|document| document.objects())
            .map(|object| SnapSourceRef {
                object,
                owner: object.id(),
                visible: object.attributes().is_visible()
                    && self
                        .visible_layers
                        .contains(&object.attributes().layer_id()),
            })
            .chain(self.expanded.iter().map(|source| SnapSourceRef {
                object: source.object.object(),
                owner: source.owner,
                visible: source.visible,
            }))
    }
}

impl BlockSources {
    pub(super) fn collect<'a>(
        &mut self,
        document: &'a Document,
    ) -> Result<QuerySources<'a>, DraftingError> {
        let visible_layers = document
            .layers()
            .filter(|layer| layer.is_visible())
            .map(|layer| layer.id())
            .collect::<HashSet<_>>();
        if document.block_definitions().len() == 0 {
            self.entries.clear();
            return Ok(QuerySources {
                ordinary: Some(document),
                visible_layers,
                expanded: Vec::new(),
            });
        }
        let mut live = HashSet::new();
        let mut sources = Vec::new();
        for object in document.objects() {
            let root_visible = object.attributes().is_visible()
                && visible_layers.contains(&object.attributes().layer_id());
            let Geometry::BlockInstance(instance) = object.geometry() else {
                sources.push(SnapSource {
                    object: QueryObject::Document(object),
                    owner: object.id(),
                    visible: root_visible,
                });
                continue;
            };
            live.insert(object.id());
            let entry = self.entries.entry(object.id()).or_insert_with(|| {
                #[cfg(test)]
                {
                    self.builds += 1;
                }
                CachedBlock::new(object)
            });
            if !entry.source.shares_storage_with(object.geometry_snapshot()) {
                *entry = CachedBlock::new(object);
                #[cfg(test)]
                {
                    self.builds += 1;
                }
            }
            // Metadata visibility is dynamic; geometry feature data and proxy
            // IDs remain reusable when layers or root attributes change.
            let insertions =
                document.block_instance_insertion_displays(object.attributes(), instance)?;
            let members = document.block_instance_member_displays(object.attributes(), instance)?;
            sources.extend(
                entry
                    .insertions
                    .iter()
                    .zip(insertions)
                    .chain(entry.members.iter().zip(members))
                    .map(|(member, display)| SnapSource {
                        object: QueryObject::Derived(Rc::clone(member)),
                        owner: object.id(),
                        visible: display.visible,
                    }),
            );
        }
        self.entries.retain(|id, _| live.contains(id));
        Ok(QuerySources {
            ordinary: None,
            visible_layers,
            expanded: sources,
        })
    }
}

impl CachedBlock {
    fn new(object: &Object) -> Self {
        let Geometry::BlockInstance(instance) = object.geometry() else {
            unreachable!("matched an instance")
        };
        let layer = object.attributes().layer_id();
        let insertions = instance
            .insertion_points()
            .iter()
            .map(|point| {
                Rc::new(Object::geometry_query_proxy(
                    Geometry::Point(point.point).into(),
                    layer,
                ))
            })
            .collect();
        let members = instance
            .members()
            .iter()
            .map(|member| {
                Rc::new(Object::geometry_query_proxy(
                    member.geometry.clone(),
                    member.attributes.layer_id(),
                ))
            })
            .collect();
        Self {
            source: object.geometry_snapshot().clone(),
            insertions,
            members,
        }
    }
}

#[cfg(test)]
mod tests;
