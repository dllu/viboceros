use super::*;
use serde_json::{Value, json};

#[test]
fn split_disjoint_mesh_matches_live_rhino_picking_observations() {
    assert_recorded_decomposition(
        "SplitDisjointMesh",
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_split_picking.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_split_picking.json"),
    );
}

#[test]
fn explode_mesh_matches_live_rhino_picking_observations() {
    assert_recorded_decomposition(
        "Explode",
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_explode_picking.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_explode_picking.json"),
    );
}

fn assert_recorded_decomposition(command: &str, request: &str, response: &str) {
    let request: Value = serde_json::from_str(request).unwrap();
    let response: Value = serde_json::from_str(response).unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = response["results"].as_array().unwrap();
    assert_eq!(operations.len(), results.len());
    let registry = CommandRegistry::with_builtins();
    for (operation, result) in operations.iter().zip(results) {
        assert_eq!(operation["id"], result["id"]);
        assert_eq!(
            operation["op"],
            if command == "Explode" {
                "mesh_explode_picking"
            } else {
                "mesh_split_picking"
            }
        );
        let mut document = Document::default();
        let ids = [0, 1, 2].map(|source| {
            let vertices = [[0., 0.], [2., 0.], [0., 2.], [0., 4.], [2., 4.], [0., 6.]]
                .map(|[x, y]| Point3::try_new(f64::from(source) * 5. + x, y, 0.).unwrap())
                .to_vec();
            let mesh =
                TriangleMesh::try_new(vertices, vec![[0, 1, 2], [3, 4, 5]], document.tolerance())
                    .unwrap();
            let attributes = ObjectAttributes::on_layer(document.current_layer_id())
                .with_name(format!("source-{source}"));
            document
                .add_geometry_with_attributes(Geometry::Mesh(mesh), attributes)
                .unwrap()
        });
        let groups = operation["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|members| {
                document
                    .add_group(
                        None,
                        members
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|index| ids[index.as_u64().unwrap() as usize]),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        if operation["reverse_bridge"].as_bool().unwrap_or(false) {
            let reversed = document
                .object(ids[1])
                .unwrap()
                .group_ids()
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>();
            document
                .set_object_group_memberships(ids[1], reversed)
                .unwrap();
        }
        for mode in ["hidden", "locked"] {
            if let Some(indices) = operation.get(mode) {
                let restricted = indices
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|index| ids[index.as_u64().unwrap() as usize])
                    .collect::<Vec<_>>();
                if mode == "hidden" {
                    document.set_objects_visibility(restricted, false).unwrap();
                } else {
                    document.set_objects_locked(restricted, true).unwrap();
                }
            }
        }
        if let Some(mode) = operation["layer_mode"].as_str() {
            let layer = document
                .add_layer("Restricted source", ColorRgb::BLACK)
                .unwrap();
            document.set_objects_layer([ids[1]], layer).unwrap();
            document.set_layer_locked(layer, mode == "locked").unwrap();
            document
                .set_layer_visibility(layer, mode != "hidden")
                .unwrap();
        }
        document
            .select_object(
                ids[operation["seed"].as_u64().unwrap() as usize],
                SelectionMode::Replace,
            )
            .unwrap();
        let selected = ids
            .iter()
            .enumerate()
            .filter_map(|(index, id)| document.is_selected(*id).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(json!(selected), result["value"]["selected"]);
        let before_objects = document.objects().cloned().collect::<Vec<_>>();
        let before_groups = document.groups().cloned().collect::<Vec<_>>();
        let before_selection = if command == "Explode" {
            BTreeSet::new()
        } else {
            document.selected_object_ids().collect::<BTreeSet<_>>()
        };
        registry.execute(&mut document, command).unwrap();
        let mut actual = document.objects().map(|object| {
            let Geometry::Mesh(mesh) = object.geometry() else { panic!("mesh expected") };
            let attributes = object.attributes();
            let mode = if !attributes.is_visible() { "Hidden" } else if attributes.is_locked() { "Locked" } else { "Normal" };
            json!({
                "source": attributes.name().unwrap(),
                "original_identity": ids.contains(&object.id()),
                "selected": document.is_selected(object.id()),
                "mode": mode,
                "layer_visible": document.layer(attributes.layer_id()).unwrap().is_visible(),
                "layer_locked": document.layer(attributes.layer_id()).unwrap().is_locked(),
                "groups": object.group_ids().iter().map(|group| groups.iter().position(|candidate| candidate == group).unwrap()).collect::<Vec<_>>(),
                "faces": mesh.face_count(),
                "vertices": mesh.vertices().iter().map(|p| [p.x(), p.y(), p.z()]).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>();
        let mut expected = result["value"]["outputs"].as_array().unwrap().clone();
        // JSON permits integral coordinates with or without a decimal point.
        // Compare geometry as exact f64 values, without an epsilon or rounding.
        for record in &mut expected {
            for vertex in record["vertices"].as_array_mut().unwrap() {
                for coordinate in vertex.as_array_mut().unwrap() {
                    *coordinate = json!(coordinate.as_f64().unwrap());
                }
            }
        }
        // The probe compares records, not document table order or UUID bytes.
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{}", operation["id"]);
        // Native history invariants are separate from the recorded Rhino
        // output comparison: that probe does not measure Rhino undo/redo.
        let after_objects = document.objects().cloned().collect::<Vec<_>>();
        let after_groups = document.groups().cloned().collect::<Vec<_>>();
        let after_selection = document.selected_object_ids().collect::<BTreeSet<_>>();
        for _ in 0..2 {
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(
                document.objects().cloned().collect::<Vec<_>>(),
                before_objects,
                "{}",
                operation["id"]
            );
            assert_eq!(
                document.groups().cloned().collect::<Vec<_>>(),
                before_groups,
                "{}",
                operation["id"]
            );
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                before_selection,
                "{}",
                operation["id"]
            );
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(
                document.objects().cloned().collect::<Vec<_>>(),
                after_objects,
                "{}",
                operation["id"]
            );
            assert_eq!(
                document.groups().cloned().collect::<Vec<_>>(),
                after_groups,
                "{}",
                operation["id"]
            );
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                after_selection,
                "{}",
                operation["id"]
            );
        }
    }
}
