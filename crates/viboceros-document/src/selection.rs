//! Ephemeral indices keep group traversal independent of document/history edits.
use super::*;

impl Document {
    pub(super) fn selectable_clusters(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> BTreeSet<ObjectId> {
        let mut ids = ids.into_iter().peekable();
        if ids.peek().is_none() {
            return BTreeSet::new();
        }
        if self.groups.is_empty() {
            return ids.filter(|id| self.is_object_selectable(*id)).collect();
        }
        // Avoid a linear object and layer lookup for every member or seed.
        let layers = self
            .layers
            .iter()
            .filter(|layer| layer.visible && !layer.locked)
            .map(|layer| layer.id)
            .collect::<BTreeSet<_>>();
        let selectable = self
            .objects
            .iter()
            .filter(|object| {
                let attributes = object.attributes();
                attributes.visible && !attributes.locked && layers.contains(&attributes.layer_id)
            })
            .map(|object| object.id)
            .collect::<BTreeSet<_>>();
        let mut connected = ids
            .filter(|id| selectable.contains(id))
            .collect::<BTreeSet<_>>();
        if connected.is_empty() {
            return connected;
        }

        let mut memberships: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();
        for (index, group) in self.groups.iter().enumerate() {
            for member in &group.members {
                memberships.entry(*member).or_default().push(index);
            }
        }
        let mut pending = connected.iter().copied().collect::<Vec<_>>();
        let mut visited_groups = vec![false; self.groups.len()];
        while let Some(id) = pending.pop() {
            if let Some(groups) = memberships.get(&id) {
                for &index in groups {
                    if std::mem::replace(&mut visited_groups[index], true) {
                        continue;
                    }
                    for &member in &self.groups[index].members {
                        if connected.insert(member) {
                            pending.push(member);
                        }
                    }
                }
            }
        }
        // Nonselectable seeds cannot start a traversal, but nonselectable
        // members remain bridges. Filter only after visiting the component.
        connected.retain(|id| selectable.contains(id));
        connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Deliberately independent, slow fixed-point definition of the policy.
    fn reference(document: &Document, seeds: &[ObjectId]) -> BTreeSet<ObjectId> {
        let mut reached = seeds
            .iter()
            .copied()
            .filter(|id| document.is_object_selectable(*id))
            .collect::<BTreeSet<_>>();
        loop {
            let before = reached.len();
            for group in &document.groups {
                if group.members.iter().any(|id| reached.contains(id)) {
                    reached.extend(group.members.iter().copied());
                }
            }
            if before == reached.len() {
                break;
            }
        }
        reached.retain(|id| document.is_object_selectable(*id));
        reached
    }

    #[test]
    fn exhaustive_three_object_hypergraphs_match_fixed_point_policy() {
        let mut document = Document::default();
        let ids = (0..3)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let missing = ObjectId::new();
        // All combinations of the seven nonempty subsets as group definitions.
        for graph in 0..128 {
            document.groups.clear();
            for members in 1..8 {
                if graph & (1 << (members - 1)) != 0 {
                    document.groups.push(Group {
                        id: GroupId::new(),
                        name: None,
                        members: ids
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| members & (1 << i) != 0)
                            .map(|(_, id)| *id)
                            .collect(),
                    });
                }
            }
            for object in &mut document.objects {
                object.group_ids = document
                    .groups
                    .iter()
                    .filter(|group| group.members.contains(&object.id))
                    .map(|group| group.id)
                    .collect();
            }
            for eligibility in 0..8 {
                for (i, object) in document.objects.iter_mut().enumerate() {
                    object.attributes.locked = eligibility & (1 << i) == 0;
                }
                for seeds in 0..8 {
                    let mut seeds = ids
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| seeds & (1 << i) != 0)
                        .map(|(_, id)| *id)
                        .collect::<Vec<_>>();
                    let expected = reference(&document, &seeds);
                    seeds.extend(seeds.clone());
                    seeds.push(missing);
                    assert_eq!(document.selectable_clusters(seeds.clone()), expected);
                    document.groups.reverse();
                    assert_eq!(document.selectable_clusters(seeds), expected);
                }
            }
        }
    }

    #[test]
    fn hidden_objects_and_layers_bridge_but_cannot_seed_selection() {
        let mut document = Document::default();
        let first = document
            .add_geometry(Geometry::Point(Point3::try_new(0.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let bridge = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let last = document
            .add_geometry(Geometry::Point(Point3::try_new(2.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.add_group(None, [first, bridge]).unwrap();
        document.add_group(None, [bridge, last]).unwrap();
        let layer = document
            .add_layer("Bridge", ColorRgb::new(1, 2, 3))
            .unwrap();
        document.objects[1].attributes.layer_id = layer;
        for mode in 0..4 {
            document.objects[1].attributes.visible = mode != 0;
            document.objects[1].attributes.locked = mode == 1;
            document.layers[1].visible = mode != 2;
            document.layers[1].locked = mode == 3;
            assert_eq!(
                document.selectable_clusters([first]),
                BTreeSet::from([first, last])
            );
            assert!(document.selectable_clusters([bridge]).is_empty());
        }
    }

    #[test]
    #[ignore = "manual release-mode traversal benchmark; no timing threshold"]
    fn reverse_chain_traversal_benchmark() {
        let mut document = Document::default();
        let ids = (0..1024)
            .map(|i| {
                document
                    .add_geometry(Geometry::Point(
                        Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                    ))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for pair in ids.windows(2).rev() {
            document.add_group(None, pair.iter().copied()).unwrap();
        }
        let start = std::time::Instant::now();
        let expected = reference(&document, &ids[..1]);
        let reference_time = start.elapsed();
        let start = std::time::Instant::now();
        let actual = document.selectable_clusters([ids[0]]);
        let traversal_time = start.elapsed();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), ids.len());
        assert_eq!(document.selectable_clusters(ids), actual);
        eprintln!(
            "1024-object reverse chain: fixed point {reference_time:?}, traversal {traversal_time:?}"
        );
    }
}
