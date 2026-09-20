//! Selected rigid layout units; unseen group members never move implicitly.
use std::collections::BTreeMap;
use viboceros_document::{Document, GroupId, Object, ObjectId};

pub(super) fn selected_units(document: &Document) -> Vec<Vec<&Object>> {
    #[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
    enum Unit {
        Object(ObjectId),
        Group(GroupId),
    }
    let mut slots = BTreeMap::new();
    let mut result = Vec::<Vec<&Object>>::new();
    for object in document.selected_objects() {
        let key = object
            .top_group()
            .map_or(Unit::Object(object.id()), Unit::Group);
        let slot = *slots.entry(key).or_insert_with(|| {
            result.push(Vec::new());
            result.len() - 1
        });
        result[slot].push(object);
    }
    result
}
