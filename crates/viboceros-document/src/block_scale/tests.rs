use super::*;
fn matrix(rows: [[f64; 3]; 3], t: [f64; 3]) -> AffineTransform3 {
    AffineTransform3::try_new(rows, Vector3::try_from(t).unwrap()).unwrap()
}
fn fixture() -> (Document, BlockDefinitionId, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let source = doc
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    let (definition, peer) = doc
        .create_block_from_objects("part", Point3::try_new(0., 0., 0.).unwrap(), [source])
        .unwrap();
    let placement = matrix(
        [[0., -2., 0.], [3., 0., 0.], [0., 0., -4.]],
        [10., 20., 30.],
    );
    let root = doc
        .add_block_instance(BlockReference::try_new(definition, placement).unwrap())
        .unwrap();
    (doc, definition, peer, root)
}
#[test]
fn frame_scaling_retains_shear_and_reflection_with_exact_insertion() {
    let shear = matrix([[2., 1., 0.], [0., 3., 1.], [0., 0., 4.]], [10., 20., 30.]);
    let reset = reset_placement(shear, BlockScaleResetMode::One).unwrap();
    let rows = reset.linear_rows();
    assert_eq!(rows[0], [1., 0.5, 0.]);
    assert!((rows[1][1] - 3. / 10_f64.sqrt()).abs() < 1e-15);
    assert!((rows[1][2] - 1. / 10_f64.sqrt()).abs() < 1e-15);
    assert!((rows[2][2] - 4. / 17_f64.sqrt()).abs() < 1e-15);
    assert_eq!(reset.translation(), shear.translation());
    let reflected = matrix(
        [[0., -2., 0.], [3., 0., 0.], [0., 0., -4.]],
        [10., 20., 30.],
    );
    assert_eq!(
        reset_placement(reflected, BlockScaleResetMode::One)
            .unwrap()
            .linear_rows(),
        [[0., -1., 0.], [1., 0., 0.], [0., 0., -1.]]
    );
}
#[test]
fn automatic_uses_absolute_pair_tolerance_and_otherwise_the_mean() {
    for (scales, want) in [
        ([2., 2. + 1e-8, 5.], 2.),
        ([2., 2. + 1.55e-8, 5.], 2.),
        ([20., 20. + 2e-8, 50.], 20.),
        ([0.2, 0.2 + 1e-8, 0.5], 0.2),
        ([5., 2., 2.], 2.),
        ([2., 5., 2.], 2.),
        ([2., 3., 4.], 3.),
        ([20., 20. + 1e-7, 50.], 30. + 1e-7 / 3.),
    ] {
        let value = reset_placement(
            matrix(
                [
                    [scales[0], 0., 0.],
                    [0., scales[1], 0.],
                    [0., 0., scales[2]],
                ],
                [0.; 3],
            ),
            BlockScaleResetMode::Automatic,
        )
        .unwrap();
        for i in 0..3 {
            assert!((value.linear_rows()[i][i] - want).abs() < 1e-13);
        }
    }
}
#[test]
fn subnormal_reciprocals_do_not_destroy_finite_normalization() {
    let tiny = f64::from_bits(1);
    let value = reset_placement(
        matrix([[tiny, 0., 0.], [0., tiny, 0.], [0., 0., tiny]], [0.; 3]),
        BlockScaleResetMode::One,
    )
    .unwrap();
    assert_eq!(value, AffineTransform3::identity());
    let mixed = reset_placement(
        matrix(
            [[tiny, 0., 0.], [0., tiny, 0.], [0., 0., f64::MAX]],
            [0.; 3],
        ),
        BlockScaleResetMode::Automatic,
    )
    .unwrap();
    assert_eq!(
        mixed.linear_rows(),
        [[tiny, 0., 0.], [0., tiny, 0.], [0., 0., tiny]]
    );
    let huge = f64::MAX / 2.;
    let value = reset_placement(
        matrix([[huge, 0., 0.], [0., huge, 0.], [0., 0., huge]], [0.; 3]),
        BlockScaleResetMode::Automatic,
    )
    .unwrap();
    assert_eq!(
        value.linear_rows(),
        [[huge, 0., 0.], [0., huge, 0.], [0., 0., huge]]
    );
}
#[test]
fn metadata_groups_protected_peers_and_catalog_survive_history() {
    let (mut doc, _, peer, root) = fixture();
    doc.set_object_geometry_user_text([root], "Shape", Some("keep"))
        .unwrap();
    doc.set_object_user_text([root], "Part", Some("root"))
        .unwrap();
    doc.add_group(None, [peer, root]).unwrap();
    doc.set_objects_locked([peer], true).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let catalog = doc.block_definitions().cloned().collect::<Vec<_>>();
    doc.reset_block_scale([root], BlockScaleResetMode::One)
        .unwrap();
    assert_eq!(
        doc.block_definitions().cloned().collect::<Vec<_>>(),
        catalog
    );
    assert_eq!(doc.object(peer).unwrap(), &before[0]);
    assert_eq!(
        doc.object(root).unwrap().attributes(),
        before[1].attributes()
    );
    assert_eq!(
        doc.object(root).unwrap().geometry_user_text(),
        before[1].geometry_user_text()
    );
    assert_eq!(doc.object(root).unwrap().group_ids(), before[1].group_ids());
    assert_eq!(doc.undo_label(), Some("BlockResetScale"));
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn invalid_batch_rejects_before_edits_and_keeps_redo() {
    let (mut doc, _, peer, root) = fixture();
    doc.set_objects_locked([peer], true).unwrap();
    let extra = doc
        .add_geometry(Geometry::Point(Point3::try_new(7., 0., 0.).unwrap()))
        .unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for ids in [vec![root, peer], vec![root, extra], vec![]] {
        assert!(
            doc.reset_block_scale(ids, BlockScaleResetMode::One)
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    doc.redo().unwrap();
    assert!(doc.object(extra).is_some());
    let before = format!("{doc:?}");
    assert!(
        doc.reset_block_scale([root, extra], BlockScaleResetMode::One)
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn failed_placement_geometry_leaves_every_root_unchanged() {
    let mut doc = Document::default();
    let source = doc
        .add_geometry(Geometry::Point(Point3::try_new(f64::MAX, 0., 0.).unwrap()))
        .unwrap();
    let (definition, _) = doc
        .create_block_from_objects("extreme", Point3::try_new(0., 0., 0.).unwrap(), [source])
        .unwrap();
    let root = doc
        .add_block_instance(
            BlockReference::try_new(
                definition,
                matrix(
                    [[0.5, 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                    [f64::MAX / 2., 0., 0.],
                ),
            )
            .unwrap(),
        )
        .unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.reset_block_scale([root], BlockScaleResetMode::One)
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
