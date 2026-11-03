use viboceros_document::Geometry;
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/inset.rs"]
mod support;
#[test]
fn subd_inset_controls_topology_tags_and_limit_points_match_native() {
    for family in ["inset_faces", "inset_extra_faces"] {
        for (op, native) in common::cases_for(family) {
            let Some(indices) = support::indices(&op) else {
                continue;
            };
            let id = op["id"].as_str().unwrap();
            let Geometry::SubD(s) = support::source(&native) else {
                panic!()
            };
            let archive = s.archive().to_vec();
            support::compare(
                &support::subd_value(&s),
                &support::expected_subd(&native["before"][0]["subd"]),
                &format!("{id}:before"),
            );
            let result = s.try_inset_faces(
                &indices,
                support::effective_distance(&op, &native, family),
                op["mode"] == "Group",
            );
            assert_eq!(s.archive(), archive);
            if native["after"][0]["subd"]["valid"] == false {
                assert!(result.is_err(), "{id}: reject collapsed native cage");
                continue;
            }
            support::compare(
                &support::subd_value(&result.unwrap_or_else(|e| panic!("{id}: {e}"))),
                &support::expected_subd(&native["after"][0]["subd"]),
                id,
            );
        }
    }
}
#[test]
fn mesh_inset_faces_vertices_normals_and_effective_channels_match_native() {
    for family in [
        "inset_mesh_faces",
        "inset_mesh_confirmed",
        "inset_mesh_primed",
        "inset_mesh_ready",
    ] {
        for (op, native) in common::cases_for(family) {
            let id = op["id"].as_str().unwrap();
            let Geometry::Mesh(s) = support::source(&native) else {
                panic!()
            };
            support::compare(
                &support::mesh_value(&s),
                &support::expected_mesh(&native["before"][0]["mesh"], false),
                &format!("{family}:{id}:before"),
            );
            let out = s
                .try_inset_faces(
                    &support::indices(&op).unwrap(),
                    support::effective_distance(&op, &native, family),
                )
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            support::compare(
                &support::mesh_value(&out),
                &support::expected_mesh(&native["after"][0]["mesh"], true),
                &format!("{family}:{id}"),
            );
        }
    }
}
#[test]
fn inset_geometry_metadata_selection_and_history_match_native() {
    use viboceros_command::inset::Plan;
    for family in ["inset_faces", "inset_extra_faces", "inset_mesh_ready"] {
        for (op, native) in common::cases_for(family) {
            let Some(indices) = support::indices(&op) else {
                continue;
            };
            if native["after"][0]["subd"]["valid"] == false {
                continue;
            }
            let label = op["id"].as_str().unwrap();
            let (mut doc, id) = support::fixture(&native);
            support::compare(
                &support::snapshot(&doc, id),
                &support::expected_rows(&native["before"], false),
                label,
            );
            let plan = Plan::prepare(
                &doc,
                indices.into_iter().map(|f| (id, f)),
                support::options(&op, support::effective_distance(&op, &native, family)),
            )
            .unwrap_or_else(|e| panic!("{label}: {e}"));
            plan.commit(&mut doc).unwrap();
            support::compare(
                &support::snapshot(&doc, id),
                &support::expected_rows(&native["after"], true),
                label,
            );
            if doc.can_undo() {
                doc.undo().unwrap();
            }
            support::compare(
                &support::snapshot(&doc, id),
                &support::expected_rows(&native["undo"], false),
                label,
            );
            if doc.can_redo() {
                doc.redo().unwrap();
            }
            support::compare(
                &support::snapshot(&doc, id),
                &support::expected_rows(&native["redo"], true),
                label,
            );
        }
    }
}

#[test]
fn refined_subd_and_mesh_insets_roundtrip_native_3dm() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inset.3dm");
    for family in ["inset_faces", "inset_mesh_ready"] {
        let (op, native) = common::cases_for(family)
            .into_iter()
            .find(|(op, _)| op["target"] == "bent_both" && op["distance"] == 0.3)
            .unwrap();
        let (mut doc, id) = support::fixture(&native);
        let geometry = match doc.object(id).unwrap().geometry() {
            Geometry::SubD(s) => {
                let source = s.subdivided(1).unwrap();
                let edge = source
                    .edge_face_counts()
                    .iter()
                    .position(|&n| n == 2)
                    .unwrap();
                let source = source
                    .try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
                    .unwrap()
                    .0;
                Geometry::SubD(source.try_inset_faces(&[0, 1], 0.1, true).unwrap())
            }
            Geometry::Mesh(m) => Geometry::Mesh(
                m.try_inset_faces(&support::indices(&op).unwrap(), 0.3)
                    .unwrap(),
            ),
            _ => panic!(),
        };
        doc.replace_object_geometries([(id, geometry)]).unwrap();
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        support::compare(
            &support::snapshot(&restored, id),
            &support::snapshot(&doc, id),
            "inset native roundtrip",
        );
        if let (Geometry::SubD(a), Geometry::SubD(b)) = (
            doc.object(id).unwrap().geometry(),
            restored.object(id).unwrap().geometry(),
        ) {
            assert_eq!(a.edge_sharpness(), b.edge_sharpness());
        }
    }
}
