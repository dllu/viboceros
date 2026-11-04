//! Actual edge commands: complete curve images, source attributes and history.
use serde_json::Value;
use std::io::Read;
use viboceros_command::{
    CommandRegistry,
    to_nurbs::{ToNurbsContext, ToNurbsTarget},
};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{LineSegment, MeshFace, Point3, SubD, Tolerance, TriangleMesh};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
mod native {
    pub fn point(v: &serde_json::Value) -> viboceros_geometry::Point3 {
        viboceros_geometry::Point3::try_from(std::array::from_fn(|i| v[i].as_f64().unwrap()))
            .unwrap()
    }
}
#[path = "../../viboceros-geometry/tests/support/curve_full_image.rs"]
mod image;
fn color(v: &Value) -> ColorRgb {
    ColorRgb::new(
        v[0].as_u64().unwrap() as u8,
        v[1].as_u64().unwrap() as u8,
        v[2].as_u64().unwrap() as u8,
    )
}
fn subd(row: &Value) -> SubD {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("s.3dm");
    let bytes = row["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::SubD(s) = &model.objects[0].geometry else {
        panic!()
    };
    s.clone()
}
fn geometry(row: &Value) -> Geometry {
    match row["kind"].as_str().unwrap() {
        "SubD" => Geometry::SubD(subd(row)),
        "Mesh" => {
            let m = &row["mesh"];
            let points = m["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(native::point)
                .collect();
            let faces = m["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| {
                    let f = f
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as u32)
                        .collect::<Vec<_>>();
                    match f.as_slice() {
                        [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                        [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                        _ => panic!(),
                    }
                })
                .collect();
            Geometry::Mesh(TriangleMesh::try_new_faces(points, faces, Tolerance::DEFAULT).unwrap())
        }
        "LineCurve" => Geometry::Line(
            LineSegment::try_new(
                Point3::try_from([20., 0., 0.]).unwrap(),
                Point3::try_from([24., 1., 0.]).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        _ => panic!(),
    }
}
fn check(doc: &Document, ids: &[ObjectId], expected: &Value) -> f64 {
    assert_eq!(doc.objects().len(), expected.as_array().unwrap().len());
    let mut bound = 0_f64;
    for (object, row) in doc.objects().zip(expected.as_array().unwrap()) {
        assert_eq!(
            ids.iter().position(|&i| i == object.id()),
            row["source"].as_u64().map(|v| v as usize)
        );
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        assert_eq!(object.attributes().name(), row["name"].as_str());
        assert_eq!(object.attributes().object_color(), color(&row["color"]));
        assert_eq!(row["color_source"], 1);
        assert_eq!(
            object.attributes().color_source(),
            viboceros_document::ObjectColorSource::Object
        );
        let kind = match object.geometry() {
            Geometry::SubD(_) => "SubD",
            Geometry::Mesh(_) => "Mesh",
            Geometry::Line(_) => "LineCurve",
            Geometry::NurbsCurve(_) => "NurbsCurve",
            other => panic!("unexpected edge-command geometry: {other:?}"),
        };
        assert_eq!(Some(kind), row["kind"].as_str());
        assert_eq!(
            doc.layer(object.attributes().layer_id()).unwrap().name(),
            if row["layer"] == 0 {
                "source"
            } else {
                "current"
            }
        );
        assert_eq!(
            object
                .group_ids()
                .iter()
                .map(|id| doc.group(*id).unwrap().name().unwrap().to_owned())
                .collect::<Vec<_>>(),
            row["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| format!("group {}", i.as_u64().unwrap()))
                .collect::<Vec<_>>()
        );
        if let Geometry::NurbsCurve(curve) = object.geometry() {
            let target = image::curve(&row["curve"]);
            assert_eq!(curve.degree(), target.degree());
            assert_eq!(curve.domain(), target.domain());
            let b = image::curve_bound(curve, &target);
            bound = bound.max(b);
            assert!(b < 1e-11, "curve image bound {b:e}");
        }
    }
    assert_eq!(doc.groups().count(), 3);
    bound
}
#[test]
fn native_nurbs_edge_commands_match_complete_geometry_and_model_policy() {
    let registry = CommandRegistry::with_builtins();
    let mut workflows = 0;
    let mut curves = 0;
    let mut maximum = 0_f64;
    for (fixture, bytes) in [
        (
            include_str!("../../../tools/rhino_oracle/fixtures/nurbs_edge_command_discovery.json"),
            include_bytes!(
                "../../../tools/rhino_oracle/observations/nurbs_edge_command_discovery.json.gz"
            )
            .as_slice(),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/nurbs_edge_command.json"),
            include_bytes!("../../../tools/rhino_oracle/observations/nurbs_edge_command.json.gz")
                .as_slice(),
        ),
        (
            include_str!("../../../tools/rhino_oracle/fixtures/nurbs_edge_nested.json"),
            include_bytes!("../../../tools/rhino_oracle/observations/nurbs_edge_nested.json.gz")
                .as_slice(),
        ),
    ] {
        let query: Value = serde_json::from_str(fixture).unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for (op, row) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(data["results"].as_array().unwrap())
        {
            let v = &row["value"];
            println!("{}", row["id"]);
            workflows += 1;
            let mut doc = Document::default();
            let layer = doc.add_layer("source", ColorRgb::BLACK).unwrap();
            let current = doc.add_layer("current", ColorRgb::BLACK).unwrap();
            doc.set_current_layer(current).unwrap();
            let mut ids = Vec::new();
            for r in v["before"].as_array().unwrap() {
                ids.push(
                    doc.add_geometry_with_attributes(
                        geometry(r),
                        ObjectAttributes::on_layer(layer)
                            .with_name(r["name"].as_str().unwrap())
                            .with_object_color(color(&r["color"])),
                    )
                    .unwrap(),
                );
            }
            for n in 0..3 {
                let members = v["before"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r["groups"].as_array().unwrap().contains(&Value::from(n)))
                    .map(|(i, _)| ids[i])
                    .collect::<Vec<_>>();
                if members.is_empty() {
                    doc.add_empty_group(Some(format!("group {n}"))).unwrap();
                } else {
                    doc.add_group(Some(format!("group {n}")), members).unwrap();
                }
            }
            let selected = v["before"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .filter(|(_, r)| r["selected"] == true)
                .map(|(i, _)| ids[i])
                .collect::<Vec<_>>();
            doc.set_transient_selection(selected).unwrap();
            doc.clear_history().unwrap();
            let flat = op["flat"] == true;
            doc.set_subd_control_net(flat);
            let original = doc.object(ids[0]).unwrap().geometry_snapshot().clone();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let mut targets = v["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| ToNurbsTarget::Edge(ids[0], i.as_u64().unwrap() as usize))
                .collect::<Vec<_>>();
            targets.extend(ids.iter().skip(1).copied().map(ToNurbsTarget::Whole));
            let command = format!(
                "ToNURBS DeleteInputObjects={}",
                if op["delete"] == true { "Yes" } else { "No" }
            );
            let prompt = registry
                .to_nurbs_target_confirmation(&doc, &command, &targets)
                .unwrap()
                .unwrap();
            assert!(prompt.menus.is_empty());
            if op["nested"] == true {
                doc.set_subd_control_net(!flat);
            }
            if v["success"] == false {
                check(&doc, &ids, &v["settled"]);
                assert!(!doc.can_undo());
                continue;
            }
            registry
                .execute_to_nurbs_targets_in_context(
                    &mut doc,
                    &command,
                    &targets,
                    ToNurbsContext {
                        postselected: false,
                        control_net: flat,
                    },
                )
                .unwrap();
            maximum = maximum.max(check(&doc, &ids, &v["settled"]));
            curves += v["settled"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["kind"] == "NurbsCurve")
                .count();
            assert!(
                doc.object(ids[0])
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(&original)
            );
            let after = doc.objects().cloned().collect::<Vec<_>>();
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            check(&doc, &ids, &v["undo"]);
            doc.redo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            check(&doc, &ids, &v["redo"]);
            assert_eq!(
                doc.subd_control_net(),
                if op["nested"] == true { !flat } else { flat }
            );
        }
    }
    println!(
        "NURBS edge native coverage: {workflows} workflows, {curves} curves, maximum image bound {maximum:e}"
    );
    assert_eq!(workflows, 26);
}
