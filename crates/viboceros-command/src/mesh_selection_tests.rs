use super::*;

#[test]
fn mesh_staging_retains_action_order_group_peers_and_read_only_failures() {
    let mut document = Document::default();
    let ids = (0..20)
        .map(|i| {
            let x = i as f64;
            let mesh = TriangleMesh::try_new(
                [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]]
                    .map(|p| Point3::try_from(p).unwrap())
                    .to_vec(),
                vec![[0, 1, 2]],
                Tolerance::DEFAULT,
            )
            .unwrap();
            document.add_geometry(Geometry::Mesh(mesh)).unwrap()
        })
        .collect::<Vec<_>>();
    document.add_group(None, [ids[0], ids[1]]).unwrap();
    document.set_objects_visibility([ids[1]], false).unwrap();
    for &id in ids[2..].iter().rev() {
        document.select_object(id, SelectionMode::Add).unwrap();
    }
    document.select_object(ids[0], SelectionMode::Add).unwrap();
    let expected = document.selected_object_ids().collect::<Vec<_>>();
    assert_eq!(expected.len(), 20);
    assert_eq!(expected[0], ids[19]);
    assert!(expected.contains(&ids[1]));
    let before = format!("{document:?}");
    let unsupported = || CommandError::Usage("expected mesh");
    let sources = selected_mesh_face_sources(&document, unsupported).unwrap();
    assert_eq!(
        sources.iter().map(|source| source.id).collect::<Vec<_>>(),
        expected
    );
    let inputs =
        stage_selected_mesh_face_extractions(&document, unsupported, |_| Ok(None)).unwrap();
    assert_eq!(
        inputs.iter().map(|input| input.id).collect::<Vec<_>>(),
        expected
    );
    for (source, input) in sources.iter().zip(&inputs) {
        let object = document.object(source.id).unwrap();
        assert_eq!(Geometry::Mesh(source.mesh.clone()), *object.geometry());
        assert_eq!(source.attributes, *object.attributes());
        assert_eq!(source.group_ids, object.group_ids());
        assert_eq!(input.attributes, source.attributes);
        assert_eq!(input.group_ids, source.group_ids);
        assert!(input.extraction.is_none());
    }
    let mut calls = 0;
    assert!(
        stage_selected_mesh_face_extractions(&document, unsupported, |_| {
            calls += 1;
            if calls == 3 {
                Err(GeometryError::NumericalIntegrationDidNotConverge)
            } else {
                Ok(None)
            }
        })
        .is_err()
    );
    assert_eq!(calls, 3);
    assert_eq!(format!("{document:?}"), before);
}
