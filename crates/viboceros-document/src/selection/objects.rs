//! Ordered immutable object access without repeated full-table searches.
use super::*;

// Keep tiny selections allocation-free. This is an implementation heuristic,
// not a selection limit or a claim about the optimal crossover on every host.
const DIRECT_LIMIT: usize = 16;

pub(super) struct SelectedObjects<'a> {
    ids: std::slice::Iter<'a, ObjectId>,
    lookup: Lookup<'a>,
    started: bool,
}

enum Lookup<'a> {
    Direct(&'a Document),
    Indexed(BTreeMap<ObjectId, &'a Object>),
}

impl<'a> SelectedObjects<'a> {
    pub(super) fn new(document: &'a Document) -> Self {
        Self {
            ids: document.selection_order.iter(),
            lookup: Lookup::Direct(document),
            started: false,
        }
    }
}

impl<'a> Iterator for SelectedObjects<'a> {
    type Item = &'a Object;

    fn next(&mut self) -> Option<Self::Item> {
        // A command often inspects only the first selected object. Keep that
        // access lazy even for large selections; build only if iteration continues.
        if self.started
            && self.ids.len() >= DIRECT_LIMIT
            && let Lookup::Direct(document) = self.lookup
        {
            self.lookup = Lookup::Indexed(
                document
                    .objects
                    .iter()
                    .filter(|o| document.selection.contains(&o.id))
                    .map(|o| (o.id, o))
                    .collect(),
            );
        }
        self.started = true;
        for id in self.ids.by_ref() {
            let object = match &self.lookup {
                Lookup::Direct(document) => document.object(*id),
                Lookup::Indexed(index) => index.get(id).copied(),
            };
            if object.is_some() {
                return object;
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.ids.len()))
    }
}

impl std::iter::FusedIterator for SelectedObjects<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_indexed_paths_match_independent_order_after_selection_edits() {
        for count in [0, 1, 16, 17, 32, 65] {
            let mut document = Document::default();
            let ids = (0..80)
                .map(|i| {
                    document
                        .add_geometry(Geometry::Point(
                            Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                        ))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let selected = ids.iter().rev().take(count).copied().collect::<Vec<_>>();
            for id in &selected {
                document
                    .select_objects_direct([*id], SelectionMode::Add)
                    .unwrap();
            }
            let before = format!("{document:?}");
            let mut iterator = SelectedObjects::new(&document);
            assert!(matches!(iterator.lookup, Lookup::Direct(_)));
            let actual = iterator.by_ref().map(|o| o.id).collect::<Vec<_>>();
            assert_eq!(
                matches!(iterator.lookup, Lookup::Direct(_)),
                count <= DIRECT_LIMIT
            );
            assert_eq!(actual, selected);
            assert!(iterator.next().is_none());
            assert_eq!(iterator.size_hint(), (0, Some(0)));
            drop(iterator);
            assert_eq!(format!("{document:?}"), before);
            for id in selected.iter().step_by(3) {
                document
                    .select_objects_direct([*id], SelectionMode::Remove)
                    .unwrap();
            }
            let expected = selected
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 3 != 0)
                .map(|(_, id)| *id)
                .collect::<Vec<_>>();
            assert_eq!(
                document
                    .selected_objects()
                    .map(|o| o.id)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn grouped_hidden_members_and_replayed_objects_remain_accessible_in_action_order() {
        let mut document = Document::default();
        let ids = (0..24)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document.add_group(None, ids.iter().copied()).unwrap();
        document.set_objects_visibility([ids[1]], false).unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            document
                .selected_objects()
                .map(|o| o.id)
                .collect::<Vec<_>>(),
            ids
        );
        document.delete_objects([ids[1], ids[4]]).unwrap();
        document.undo().unwrap();
        let expected = document.selected_object_ids().collect::<Vec<_>>();
        assert_eq!(
            document
                .selected_objects()
                .map(|o| o.id)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(document.selected_objects().any(|o| o.id == ids[1]));
    }

    #[test]
    fn first_object_access_does_not_allocate_the_large_selection_index() {
        let mut document = Document::default();
        let ids = (0..32)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let mut iterator = SelectedObjects::new(&document);
        assert_eq!(iterator.next().unwrap().id, ids[0]);
        assert!(matches!(iterator.lookup, Lookup::Direct(_)));
        assert_eq!(iterator.next().unwrap().id, ids[1]);
        assert!(matches!(iterator.lookup, Lookup::Indexed(_)));
        assert_eq!(iterator.map(|o| o.id).collect::<Vec<_>>(), ids[2..]);
    }
}
