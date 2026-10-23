use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{MeshFace, MeshNgon, Point3, Tolerance, TriangleMesh, Vector3};

fn mesh(v: &Value, drop_invalid: bool) -> (TriangleMesh, Vec<usize>) {
    let array = |key: &str| v[key].as_array().filter(|a| !a.is_empty());
    let base = TriangleMesh::try_from_face_records(
        serde_json::from_value::<Vec<[f64; 3]>>(v["vertices"].clone())
            .unwrap()
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let indices: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
                match *indices.as_slice() {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => panic!(),
                }
            })
            .collect(),
    )
    .unwrap()
    .try_with_vertex_colors(
        array("colors").map(|a| serde_json::from_value(Value::Array(a.clone())).unwrap()),
    )
    .unwrap()
    .try_with_texture_coordinates(
        array("uv").map(|a| serde_json::from_value(Value::Array(a.clone())).unwrap()),
    )
    .unwrap()
    .try_with_vertex_normals(array("normals").map(|a| {
        a.iter()
            .map(|n| {
                Vector3::try_from(serde_json::from_value::<[f64; 3]>(n.clone()).unwrap()).unwrap()
            })
            .collect()
    }))
    .unwrap();
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for (index, g) in v["ngons"].as_array().unwrap().iter().enumerate() {
        let ngon = MeshNgon::from_parts(
            serde_json::from_value(g["vertices"].clone()).unwrap(),
            serde_json::from_value(g["faces"].clone()).unwrap(),
        );
        if base.clone().try_with_ngons(vec![ngon.clone()]).is_err() {
            assert!(drop_invalid);
            invalid.push(index);
        } else {
            valid.push(ngon);
        }
    }
    (base.try_with_ngons(valid).unwrap(), invalid)
}

#[test]
fn sdk_conversion_and_command_history_match_independent_native_inputs() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_quad_conversion_v3.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for row in data["results"].as_array().unwrap() {
        eprintln!("Quad conversion {}", row["id"]);
        let value = &row["value"];
        assert_eq!(value["valid_before"], true);
        assert_eq!(value["valid_after"], true);
        let (source, before_invalid) = mesh(&value["before"], false);
        assert!(before_invalid.is_empty());
        let (expected, native_invalid) = mesh(&value["after"], true);
        assert_eq!(
            native_invalid,
            if row["id"] == "quad_convert_adjacent_ngons" {
                vec![0]
            } else {
                vec![]
            }
        );
        let angle = value["input"]["angle"].as_f64().unwrap().to_degrees();
        let ratio = value["input"]["ratio"].as_f64().unwrap();
        let bound = if ratio == 0. { f64::MAX } else { 1. / ratio };
        let (converted, pairs) = source
            .quadrangulate_triangles(angle, bound, Tolerance::default())
            .unwrap();
        assert_eq!(converted, expected);
        assert_eq!(pairs, source.face_count() - expected.face_count());
        assert_eq!(value["changed"], pairs > 0);
        for script in [
            format!("QuadrangulateMesh Planarity={angle:.17} Rectangularity={bound:.17}"),
            format!("_-QuadrangulateMesh _Rectangularity {bound:.17} _Planarity {angle:.17}"),
        ] {
            let mut doc = Document::default();
            let id = doc.add_geometry(Geometry::Mesh(source.clone())).unwrap();
            doc.set_object_names([(id, Some("Quad reference".to_owned()))])
                .unwrap();
            doc.set_object_user_text([id], "oracle", row["id"].as_str())
                .unwrap();
            doc.add_group(Some("Quad group".to_owned()), [id]).unwrap();
            registry.execute(&mut doc, "Point 50,50,50").unwrap();
            doc.undo().unwrap();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let undo = doc.undo_label().map(str::to_owned);
            let redo = doc.redo_label().map(str::to_owned);
            let attributes = doc.object(id).unwrap().attributes().clone();
            let groups = doc.object(id).unwrap().group_ids().to_vec();
            registry.execute(&mut doc, &script).unwrap();
            assert_eq!(
                doc.object(id).unwrap().geometry(),
                &Geometry::Mesh(expected.clone())
            );
            assert_eq!(doc.object(id).unwrap().attributes(), &attributes);
            assert_eq!(doc.object(id).unwrap().group_ids(), &groups);
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), vec![id]);
            if pairs > 0 {
                assert_eq!(doc.undo_label(), Some("QuadrangulateMesh"));
                doc.undo().unwrap();
                assert_eq!(
                    doc.object(id).unwrap().geometry(),
                    &Geometry::Mesh(source.clone())
                );
                doc.redo().unwrap();
                assert_eq!(
                    doc.object(id).unwrap().geometry(),
                    &Geometry::Mesh(expected.clone())
                );
            } else {
                assert_eq!(doc.undo_label(), undo.as_deref());
                assert_eq!(doc.redo_label(), redo.as_deref());
            }
        }
        println!(
            "QUAD_CONVERSION {}",
            json!({"id":row["id"],"qualified":true,"faces":expected.face_count(),"vertices":expected.vertices().len(),"ngons":expected.ngons().len(),"pairs":pairs,"changed":pairs>0,"history":true,"metadata":true,"selection":true,"channels":true,"raw_faces":true,"native_invalid_ngons":native_invalid,"native_ngons_correspond":native_invalid.is_empty(),"maximum_error":0.,"command_syntaxes":2})
        );
    }
}

#[test]
fn a_late_invalid_mesh_admits_no_partial_meshes() {
    let source = TriangleMesh::try_new(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        Tolerance::default(),
    )
    .unwrap();
    let mut bad_vertices = source.vertices().to_vec();
    bad_vertices[3] = bad_vertices[0];
    let bad = TriangleMesh::try_from_face_records(bad_vertices, source.faces().to_vec()).unwrap();
    let mut doc = Document::default();
    let first = doc.add_geometry(Geometry::Mesh(source.clone())).unwrap();
    let second = doc.add_geometry(Geometry::Mesh(bad.clone())).unwrap();
    doc.select_objects_direct([first, second], SelectionMode::Replace)
        .unwrap();
    let undo = doc.undo_label().map(str::to_owned);
    assert!(
        CommandRegistry::with_builtins()
            .execute(&mut doc, "QuadrangulateMesh")
            .is_err()
    );
    assert_eq!(
        doc.object(first).unwrap().geometry(),
        &Geometry::Mesh(source)
    );
    assert_eq!(doc.object(second).unwrap().geometry(), &Geometry::Mesh(bad));
    assert_eq!(doc.undo_label(), undo.as_deref());
}
