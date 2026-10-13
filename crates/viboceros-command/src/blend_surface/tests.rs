use super::*;

fn sources() -> (CommandRegistry, Document, [ObjectId; 2]) {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 1,0,0 1,2,0 0,2,0")
        .unwrap();
    registry
        .execute(&mut doc, "SrfPt 3,0,1 4,0,1 4,2,1 3,2,1")
        .unwrap();
    let ids = doc
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    doc.clear_history().unwrap();
    (registry, doc, ids)
}

#[test]
fn surface_blend_creates_selected_face_and_preserves_sources_through_undo_redo() {
    let (registry, mut doc, ids) = sources();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(
            &mut doc,
            &format!(
                "BlendSrf Sources={},{} Continuity1=G2 Continuity2=Curvature Bulge1=2 Bulge2=3",
                ids[0], ids[1]
            ),
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 3);
    for (o, id) in before.iter().zip(ids) {
        assert_eq!(doc.object(id).unwrap(), o);
    }
    let Geometry::Brep(b) = doc.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces()[0].surface().degree_v(), 5);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn readonly_preview_and_stale_preview_leave_history_and_geometry_unchanged() {
    let (registry, mut doc, ids) = sources();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let text = registry
        .execute(
            &mut doc,
            &format!("BlendSrf Sources={},{} Preview=Yes", ids[0], ids[1]),
        )
        .unwrap();
    assert!(text.contains("preview only"));
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    let prepared = prepare(&doc, ids, &Options::default()).unwrap();
    let mut scene = doc.clone();
    prepared.commit(&mut scene).unwrap();
    assert_eq!(scene.objects().len(), 3);
    assert_eq!(doc.objects().len(), 2);
    doc.delete_object(ids[1]).unwrap();
    let changed = doc.objects().cloned().collect::<Vec<_>>();
    assert!(prepared.commit(&mut doc).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), changed);
}

#[test]
fn rectangular_trimming_and_joining_accept_atomically_and_retain_first_identity() {
    let (registry, mut doc, ids) = sources();
    doc.set_object_names([(ids[0], Some("first parent".to_string()))])
        .unwrap();
    let group = doc
        .add_group(Some("source group".to_string()), ids)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.clear_history().unwrap();
    registry
        .execute(
            &mut doc,
            &format!(
                "BlendSrf {} 1 {} 3 Continuity1=G2 Continuity2=G2 Trim1=0.2 Trim2=0.3 Join=Yes",
                ids[0], ids[1]
            ),
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    let object = doc.object(ids[0]).unwrap();
    assert_eq!(object.attributes().name(), Some("first parent"));
    assert!(object.group_ids().contains(&group));
    let Geometry::Brep(b) = object.geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 3);
    assert!(b.is_manifold());
    assert!(
        b.edges_shared_by_distinct_faces()
            .iter()
            .filter(|&&x| x)
            .count()
            >= 2
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn invalid_options_and_component_indices_do_not_change_history() {
    let (registry, mut doc, ids) = sources();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for args in [
        "Bulge1=0",
        "Bulge2=-1",
        "Bulge2=nan",
        "Trim1=1",
        "Trim2=-0.1",
        "Continuity1=G3",
        "Reverse2=Maybe",
        "Join=Maybe",
        "Edge1=Up",
        "Bulge1=1 Bulge1=2",
    ] {
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("BlendSrf Sources={},{} {args}", ids[0], ids[1])
                )
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
    assert!(
        registry
            .execute(&mut doc, &format!("BlendSrf {} 100 {} 3", ids[0], ids[1]))
            .is_err()
    );
    assert!(
        registry
            .execute(&mut doc, &format!("BlendSrf Sources={},{}", ids[0], ids[0]))
            .is_err()
    );
}

#[test]
fn surface_blend_brep_survives_opennurbs_roundtrip() {
    let (registry, mut doc, ids) = sources();
    registry
        .execute(
            &mut doc,
            &format!(
                "BlendSrf Sources={},{} Continuity1=G2 Continuity2=G2 Join=Yes",
                ids[0], ids[1]
            ),
        )
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("blend.3dm");
    registry
        .execute(&mut doc, &format!("SaveAs {}", path.display()))
        .unwrap();
    let read = viboceros_io::read_3dm_file_with_model_tolerance(&path).unwrap();
    assert_eq!(read.objects.len(), 1);
    let viboceros_io::ThreeDmGeometry::Brep(b) = &read.objects[0].geometry else {
        panic!()
    };
    assert_eq!(b.faces().len(), 3);
    assert!(b.is_manifold());
}

#[test]
fn supplied_astra_closed_collar_edges_blend_join_and_export() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../viboceros-geometry/src/surface_blend/astra-collars.json"
    ))
    .unwrap();
    let mut doc = Document::default();
    let mut ids = Vec::new();
    for i in [0, 2] {
        let s = &data[i];
        let coordinates: Vec<[Real; 3]> =
            serde_json::from_value(s["control_points_v_rows_u_fastest"].clone()).unwrap();
        let weights: Vec<Real> = serde_json::from_value(s["weights"].clone()).unwrap();
        let surface = NurbsSurface::try_new_rational(
            s["degree_u"].as_u64().unwrap() as usize,
            s["degree_v"].as_u64().unwrap() as usize,
            s["control_count_u"].as_u64().unwrap() as usize,
            s["control_count_v"].as_u64().unwrap() as usize,
            coordinates
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
                .collect(),
            serde_json::from_value(s["knots_u"].clone()).unwrap(),
            serde_json::from_value(s["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        ids.push(
            doc.add_geometry(Geometry::Brep(
                Brep::try_surface_face(surface, doc.tolerance()).unwrap(),
            ))
            .unwrap(),
        );
    }
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc,&format!("BlendSrf Sources={},{} Edge1=East Edge2=West Continuity1=G2 Continuity2=G2 Bulge1={} Bulge2={} Join=Yes",ids[0],ids[1],2. * 5.2 / 0.55,2. * 5.2 / 0.45)).unwrap();
    assert_eq!(doc.objects().len(), 1);
    let Geometry::Brep(b) = doc.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(b.faces().len(), 3);
    assert!(b.is_manifold());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("astra-collar-blend.3dm");
    registry
        .execute(&mut doc, &format!("SaveAs {}", path.display()))
        .unwrap();
    let model = viboceros_io::read_3dm_file_with_model_tolerance(&path).unwrap();
    assert_eq!(model.unsupported_object_count(), 0);
    assert_eq!(model.objects.len(), 1);
    let viboceros_io::ThreeDmGeometry::Brep(restored) = &model.objects[0].geometry else {
        panic!()
    };
    assert_eq!(restored.faces().len(), 3);
    assert!(restored.is_manifold());
}
