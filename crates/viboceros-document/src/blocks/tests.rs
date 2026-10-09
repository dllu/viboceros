use super::*;
use crate::{ColorRgb, ObjectColorSource, SelectionMode};
use viboceros_geometry::{LengthUnitSystem, LineSegment, Vector3};

fn point(x: f64, y: f64, z: f64) -> GeometrySnapshot {
    Geometry::Point(Point3::try_new(x, y, z).unwrap()).into()
}

fn leaf(document: &Document, geometry: GeometrySnapshot) -> BlockMember {
    BlockMember::new(
        BlockContent::Geometry(geometry),
        ObjectAttributes::on_layer(document.current_layer_id()),
    )
}

fn reference(id: BlockDefinitionId, transform: AffineTransform3) -> BlockReference {
    BlockReference::try_new(id, transform).unwrap()
}

fn nested(document: &Document, id: BlockDefinitionId, transform: AffineTransform3) -> BlockMember {
    BlockMember::new(
        BlockContent::Reference(reference(id, transform)),
        ObjectAttributes::on_layer(document.current_layer_id()),
    )
}

fn xyz(member: &ResolvedBlockMember) -> [f64; 3] {
    let Geometry::Point(point) = *member.geometry else {
        panic!("expected point")
    };
    point.to_array()
}

#[test]
fn shared_nested_placements_preserve_order_metadata_paths_and_source_snapshots() {
    let mut document = Document::default();
    let geometry = point(1., 2., 3.);
    let attrs = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("leaf")
        .with_color_source(ObjectColorSource::Parent)
        .with_locked(true)
        .try_with_user_text("material", "steel")
        .unwrap();
    let member = BlockMember::new(BlockContent::Geometry(geometry.clone()), attrs.clone())
        .try_with_geometry_user_text("tag", "original")
        .unwrap();
    let child = BlockDefinition::new("child", vec![member]);
    let child_transform = AffineTransform3::try_new(
        [[2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(1., 2., 3.).unwrap(),
    )
    .unwrap();
    let root = BlockDefinition::new(
        "root",
        vec![
            nested(&document, child.id(), child_transform),
            nested(&document, child.id(), AffineTransform3::identity()),
        ],
    );
    // A forward reference is valid in an atomic batch.
    document
        .set_block_definitions(vec![root.clone(), child.clone()])
        .unwrap();
    let placement = AffineTransform3::try_new(
        [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        Vector3::try_new(10., 20., 30.).unwrap(),
    )
    .unwrap();
    let before = format!("{document:?}");
    let resolved = document
        .resolve_block(reference(root.id(), placement))
        .unwrap();
    assert_eq!(resolved.len(), 2);
    assert_eq!(xyz(&resolved[0]), [2., 23., 45.]);
    assert_eq!(xyz(&resolved[1]), [8., 21., 33.]);
    assert_eq!(resolved[0].attributes, attrs);
    assert_eq!(resolved[0].geometry_user_text["tag"], "original");
    assert_eq!(
        resolved[0].path,
        vec![
            BlockMemberLocation {
                definition: root.id(),
                member_index: 0
            },
            BlockMemberLocation {
                definition: child.id(),
                member_index: 0
            },
        ]
    );
    assert_eq!(resolved[1].path[0].member_index, 1);
    assert_eq!(format!("{document:?}"), before);
    let BlockContent::Geometry(stored) = child.members()[0].content() else {
        panic!()
    };
    assert!(stored.shares_storage_with(&geometry));
    let direct = document
        .resolve_block(reference(child.id(), AffineTransform3::identity()))
        .unwrap();
    assert!(direct[0].geometry.shares_storage_with(&geometry));
}

#[test]
fn editing_a_shared_definition_updates_all_uses_and_replays_exact_snapshots() {
    let mut document = Document::default();
    let original = point(1., 0., 0.);
    let child = document
        .add_block_definition("child", vec![leaf(&document, original.clone())])
        .unwrap();
    let root = document
        .add_block_definition(
            "root",
            vec![
                nested(&document, child, AffineTransform3::identity()),
                nested(
                    &document,
                    child,
                    AffineTransform3::from_translation(Vector3::try_new(10., 0., 0.).unwrap()),
                ),
            ],
        )
        .unwrap();
    let root_ref = reference(root, AffineTransform3::identity());
    let changed = point(2., 0., 0.);
    document
        .replace_block_definition_members(child, vec![leaf(&document, changed.clone())])
        .unwrap();
    assert_eq!(
        document
            .resolve_block(root_ref)
            .unwrap()
            .iter()
            .map(xyz)
            .collect::<Vec<_>>(),
        [[2., 0., 0.], [12., 0., 0.]]
    );
    document.undo().unwrap();
    let resolved = document.resolve_block(root_ref).unwrap();
    assert_eq!(xyz(&resolved[0]), [1., 0., 0.]);
    assert!(resolved[0].geometry.shares_storage_with(&original));
    // Equal replacement and unknown deletion leave the complete redo state intact.
    let before = format!("{document:?}");
    assert!(
        !document
            .replace_block_definition_members(child, vec![leaf(&document, original)])
            .unwrap()
    );
    assert!(
        document
            .remove_block_definition(BlockDefinitionId::new())
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
    document.redo().unwrap();
    assert!(
        document.resolve_block(root_ref).unwrap()[0]
            .geometry
            .shares_storage_with(&changed)
    );
}

#[test]
fn catalog_transactions_rollback_and_commit_as_one_step_without_touching_selection() {
    let mut document = Document::default();
    let object = document
        .add_geometry(Geometry::Point(Point3::try_new(4., 5., 6.).unwrap()))
        .unwrap();
    document
        .select_object(object, SelectionMode::Replace)
        .unwrap();
    let selection = document.selection_order.clone();
    let last = document.last_changed_objects.clone();
    document.begin_transaction("Blocks").unwrap();
    let child = document
        .add_block_definition("child", vec![leaf(&document, point(1., 0., 0.))])
        .unwrap();
    document
        .add_block_definition(
            "root",
            vec![nested(&document, child, AffineTransform3::identity())],
        )
        .unwrap();
    document.rollback_transaction().unwrap();
    assert_eq!(document.block_definitions().len(), 0);
    assert_eq!(document.selection_order, selection);
    assert_eq!(document.last_changed_objects, last);
    document.begin_transaction("Blocks").unwrap();
    document.add_block_definition("one", vec![]).unwrap();
    document.add_block_definition("two", vec![]).unwrap();
    document.commit_transaction().unwrap();
    assert_eq!(document.undo_label(), Some("Blocks"));
    document.undo().unwrap();
    assert_eq!(document.block_definitions().len(), 0);
    document.redo().unwrap();
    assert_eq!(document.block_definitions().len(), 2);
    assert_eq!(document.last_changed_objects, last);
}

#[test]
fn malformed_graphs_names_layers_and_live_definition_deletion_fail_atomically() {
    let mut document = Document::default();
    let child = BlockDefinition::new("child", vec![leaf(&document, point(1., 0., 0.))]);
    let root = BlockDefinition::new(
        "root",
        vec![nested(&document, child.id(), AffineTransform3::identity())],
    );
    document
        .set_block_definitions(vec![root.clone(), child.clone()])
        .unwrap();
    for definitions in [
        vec![child.clone(), child.clone()],
        vec![child.clone(), BlockDefinition::new(" CHILD ", vec![])],
        vec![BlockDefinition::new(" \t ", vec![])],
        vec![BlockDefinition::new("bad\0name", vec![])],
        vec![root.clone()],
        vec![
            root.clone(),
            child.clone().with_members(vec![nested(
                &document,
                root.id(),
                AffineTransform3::identity(),
            )]),
        ],
        vec![child.clone().with_members(vec![nested(
            &document,
            child.id(),
            AffineTransform3::identity(),
        )])],
        vec![BlockDefinition::new(
            "bad layer",
            vec![BlockMember::new(
                BlockContent::Geometry(point(1., 0., 0.)),
                ObjectAttributes::on_layer(crate::LayerId::new()),
            )],
        )],
    ] {
        let before = format!("{document:?}");
        assert!(document.set_block_definitions(definitions).is_err());
        assert_eq!(format!("{document:?}"), before);
    }
    let before = format!("{document:?}");
    assert!(document.remove_block_definition(child.id()).is_err());
    assert_eq!(format!("{document:?}"), before);
    document.remove_block_definition(root.id()).unwrap();
    document.remove_block_definition(child.id()).unwrap();
    document.undo().unwrap();
    document.undo().unwrap();
    assert_eq!(
        document.block_definitions().cloned().collect::<Vec<_>>(),
        [root, child]
    );
}

#[test]
fn definition_member_layers_remain_live_until_catalog_removal() {
    let mut document = Document::default();
    let layer = document.add_layer("members", ColorRgb::BLACK).unwrap();
    let id = document
        .add_block_definition(
            "block",
            vec![BlockMember::new(
                BlockContent::Geometry(point(1., 0., 0.)),
                ObjectAttributes::on_layer(layer),
            )],
        )
        .unwrap();
    assert_eq!(
        document.delete_layer(layer),
        Err(DocumentError::LayerNotEmpty(layer))
    );
    document.remove_block_definition(id).unwrap();
    document.delete_layer(layer).unwrap();
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(document.layer(layer).is_some());
    assert!(document.block_definition(id).is_some());
}

#[test]
fn nesting_depth_limits_work_with_cached_shared_subgraphs() {
    let mut document = Document::default();
    let mut definitions = vec![BlockDefinition::new("level0", vec![])];
    for level in 1..MAX_DEPTH {
        let previous = definitions.last().unwrap().id();
        definitions.push(BlockDefinition::new(
            format!("level{level}"),
            vec![nested(&document, previous, AffineTransform3::identity())],
        ));
    }
    // Child-first order exercises memoized heights, not just recursion depth.
    document.set_block_definitions(definitions.clone()).unwrap();
    let root = definitions.last().unwrap().id();
    assert!(
        document
            .resolve_block(reference(root, AffineTransform3::identity()))
            .unwrap()
            .is_empty()
    );
    definitions.push(BlockDefinition::new(
        "too deep",
        vec![nested(&document, root, AffineTransform3::identity())],
    ));
    let before = format!("{document:?}");
    assert!(document.set_block_definitions(definitions.clone()).is_err());
    definitions.reverse();
    assert!(document.set_block_definitions(definitions).is_err());
    assert_eq!(format!("{document:?}"), before);
}

fn branching(
    document: &Document,
    leaf_members: Vec<BlockMember>,
    levels: usize,
) -> Vec<BlockDefinition> {
    let mut definitions = vec![BlockDefinition::new("level0", leaf_members)];
    for level in 1..levels {
        let id = definitions.last().unwrap().id();
        let member = nested(document, id, AffineTransform3::identity());
        definitions.push(BlockDefinition::new(
            format!("level{level}"),
            vec![member.clone(), member],
        ));
    }
    definitions
}

#[test]
fn empty_branching_dags_and_excessive_output_are_bounded_without_mutation() {
    for (leaves, levels, expected) in [
        (false, 21, "placement visit limit exceeded"),
        (true, 18, "placed geometry limit exceeded"),
    ] {
        let mut document = Document::default();
        let members = if leaves {
            vec![leaf(&document, point(1., 0., 0.))]
        } else {
            vec![]
        };
        let definitions = branching(&document, members, levels);
        let root = definitions.last().unwrap().id();
        document.set_block_definitions(definitions).unwrap();
        let before = format!("{document:?}");
        assert_eq!(
            document.resolve_block(reference(root, AffineTransform3::identity())),
            Err(invalid(expected))
        );
        assert_eq!(format!("{document:?}"), before);
    }
}

#[test]
fn catalog_definition_and_member_limits_are_atomic() {
    let mut document = Document::default();
    let before = format!("{document:?}");
    assert!(
        document
            .set_block_definitions(
                (0..=MAX_DEFINITIONS)
                    .map(|i| BlockDefinition::new(format!("d{i}"), vec![]))
                    .collect()
            )
            .is_err()
    );
    assert!(
        document
            .add_block_definition(
                "too many members",
                vec![leaf(&document, point(0., 0., 0.)); MAX_MEMBERS + 1]
            )
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn reflected_and_short_geometry_placements_validate_without_model_feature_thresholds() {
    let mut document = Document::default();
    let origin = Point3::try_new(0., 0., 0.).unwrap();
    let line = Geometry::Line(
        LineSegment::try_new(
            origin,
            Point3::try_new(0.0001, 0., 0.).unwrap(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap(),
    );
    let id = document
        .add_block_definition("short", vec![leaf(&document, line.into())])
        .unwrap();
    let reflected = AffineTransform3::try_new(
        [[-2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let resolved = document.resolve_block(reference(id, reflected)).unwrap();
    let Geometry::Line(line) = &*resolved[0].geometry else {
        panic!()
    };
    assert_eq!(line.end().to_array(), [-0.0002, 0., 0.]);
    let singular = AffineTransform3::try_uniform_scale(origin, 0.).unwrap();
    assert!(BlockReference::try_new(id, singular).is_err());
}

#[test]
fn failed_late_placement_returns_no_partial_geometry_and_keeps_history() {
    let mut document = Document::default();
    let id = document
        .add_block_definition(
            "overflow",
            vec![
                leaf(&document, point(1., 0., 0.)),
                leaf(&document, point(f64::MAX, 0., 0.)),
            ],
        )
        .unwrap();
    let scale =
        AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.).unwrap(), 2.).unwrap();
    let before = format!("{document:?}");
    assert!(document.resolve_block(reference(id, scale)).is_err());
    assert_eq!(format!("{document:?}"), before);
    assert!(
        document
            .resolve_block(reference(
                BlockDefinitionId::new(),
                AffineTransform3::identity()
            ))
            .is_err()
    );
}

#[test]
fn nested_unit_rescaling_changes_geometry_and_translation_once_and_replays_storage() {
    let mut document = Document::default();
    let original = point(1000., 2000., 3000.);
    let child = document
        .add_block_definition("child", vec![leaf(&document, original.clone())])
        .unwrap();
    let child_transform = AffineTransform3::try_new(
        [[2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(100., 200., 300.).unwrap(),
    )
    .unwrap();
    let root = document
        .add_block_definition("root", vec![nested(&document, child, child_transform)])
        .unwrap();
    let placement = reference(root, AffineTransform3::identity());
    let before = document.resolve_block(placement).unwrap();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    let scaled = document.resolve_block(placement).unwrap();
    assert_eq!(xyz(&scaled[0]), [2.1, 6.2, 12.3]);
    let BlockContent::Reference(stored) =
        document.block_definition(root).unwrap().members()[0].content()
    else {
        panic!()
    };
    assert_eq!(
        stored.transform.linear_rows(),
        child_transform.linear_rows()
    );
    assert_eq!(stored.transform.translation().to_array(), [0.1, 0.2, 0.3]);
    document.undo().unwrap();
    assert_eq!(document.resolve_block(placement).unwrap(), before);
    let BlockContent::Geometry(stored) =
        document.block_definition(child).unwrap().members()[0].content()
    else {
        panic!()
    };
    assert!(stored.shares_storage_with(&original));
    document.redo().unwrap();
    assert_eq!(document.resolve_block(placement).unwrap(), scaled);
    let definitions = document.block_definitions.clone();
    document.set_units(LengthUnitSystem::Inches, false).unwrap();
    assert_eq!(document.block_definitions, definitions);
}

#[test]
fn late_definition_or_object_unit_failure_leaves_both_tables_and_redo_intact() {
    for fail_in_definition in [true, false] {
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Point(
                Point3::try_new(if fail_in_definition { 1. } else { f64::MAX }, 0., 0.).unwrap(),
            ))
            .unwrap();
        document
            .add_block_definition("first", vec![leaf(&document, point(1., 0., 0.))])
            .unwrap();
        document
            .add_block_definition(
                "last",
                vec![leaf(
                    &document,
                    point(if fail_in_definition { f64::MAX } else { 1. }, 0., 0.),
                )],
            )
            .unwrap();
        document.add_block_definition("undo me", vec![]).unwrap();
        document.undo().unwrap();
        let before = format!("{document:?}");
        assert!(
            document
                .set_units(
                    LengthUnitSystem::Custom {
                        name: "half mm".into(),
                        meters_per_unit: 0.0005
                    },
                    true
                )
                .is_err()
        );
        assert_eq!(format!("{document:?}"), before);
    }
}
