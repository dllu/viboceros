//! Native command evidence for identity, attributes, chronology and history.
use serde_json::Value;
use std::io::Read;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{LineSegment, Point3, SubD, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

fn subd(row: &Value) -> SubD {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.3dm");
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
fn color(row: &Value) -> ColorRgb {
    ColorRgb::new(
        row["color"][0].as_u64().unwrap() as u8,
        row["color"][1].as_u64().unwrap() as u8,
        row["color"][2].as_u64().unwrap() as u8,
    )
}
fn check(doc: &Document, ids: &[ObjectId], expected: &Value, packs: bool) {
    let rows = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len());
    for (object, row) in doc.objects().zip(rows) {
        assert_eq!(
            Some(&object.id()),
            ids.get(row["source"].as_u64().unwrap() as usize)
        );
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        let a = object.attributes();
        assert_eq!(a.name(), row["name"].as_str());
        assert_eq!(a.object_color(), color(row));
        assert_eq!(doc.layer(a.layer_id()).unwrap().name(), "source");
        assert_eq!(
            object.group_ids().len(),
            row["groups"].as_array().unwrap().len()
        );
        for (&group, number) in object
            .group_ids()
            .iter()
            .zip(row["groups"].as_array().unwrap())
        {
            assert_eq!(
                doc.group(group).unwrap().name(),
                Some(format!("group {}", number).as_str())
            );
        }
        if let Geometry::SubD(s) = object.geometry() {
            let native = subd(row);
            assert_eq!(s.control_points(), native.control_points());
            assert_eq!(s.surface_points(), native.surface_points());
            assert_eq!(s.faces(), native.faces());
            assert_eq!(s.edges(), native.edges());
            assert_eq!(s.vertex_tags(), native.vertex_tags());
            assert_eq!(s.edge_tags(), native.edge_tags());
            assert_eq!(s.edge_sharpness(), native.edge_sharpness());
            if packs {
                assert_eq!(
                    s.face_packs()
                        .unwrap()
                        .iter()
                        .map(|p| p.unwrap().id)
                        .collect::<Vec<_>>(),
                    native
                        .face_packs()
                        .unwrap()
                        .iter()
                        .map(|p| p.unwrap().id)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
#[test]
fn actual_pack_commands_preserve_native_identity_selection_and_history() {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(
        include_bytes!("../../../tools/rhino_oracle/observations/subd_pack_command.json.gz")
            .as_slice(),
    )
    .read_to_end(&mut raw)
    .unwrap();
    let capture: Value = serde_json::from_slice(&raw).unwrap();
    let query: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/subd_pack_command.json"
    ))
    .unwrap();
    let commands = CommandRegistry::with_builtins();
    for (case, evidence) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(capture["results"].as_array().unwrap())
    {
        println!("{}", case["id"]);
        let steps = evidence["value"]["steps"].as_array().unwrap();
        let mut doc = Document::default();
        let layer = doc.add_layer("source", ColorRgb::BLACK).unwrap();
        let current = doc
            .add_layer("current", ColorRgb::new(255, 255, 255))
            .unwrap();
        doc.set_current_layer(current).unwrap();
        let mut ids = Vec::new();
        for row in steps[0]["before"].as_array().unwrap() {
            let geometry = match row["kind"].as_str().unwrap() {
                "SubD" => Geometry::SubD(subd(row)),
                "Point" => Geometry::Point(Point3::try_new(20., 0., 0.).unwrap()),
                "LineCurve" => Geometry::Line(
                    LineSegment::try_new(
                        Point3::try_new(30., 0., 0.).unwrap(),
                        Point3::try_new(31., 1., 2.).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                ),
                _ => panic!(),
            };
            ids.push(
                doc.add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(layer)
                        .with_name(row["name"].as_str().unwrap())
                        .with_object_color(color(row)),
                )
                .unwrap(),
            );
        }
        for n in 0..3 {
            let members = steps[0]["before"]
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
        doc.clear_history().unwrap();
        if case["postselect"] != true {
            doc.set_transient_selection(ids.clone()).unwrap();
        }
        for (repeat, step) in steps.iter().enumerate() {
            check(&doc, &ids, &step["before"], false);
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let post = case["postselect"] == true || repeat > 0;
            if post {
                doc.set_transient_selection(
                    ids.iter()
                        .enumerate()
                        .rev()
                        .filter(|(i, _)| *i == 0 || *i == 2)
                        .map(|(_, id)| *id),
                )
                .unwrap();
            }
            if case["cancel"] == true {
                let prompt = commands
                    .object_selection_prompt("PackSubDFaces")
                    .unwrap()
                    .unwrap();
                assert!(prompt.filter.accepts_object(doc.object(ids[0]).unwrap()));
                doc.clear_selection();
                check(&doc, &ids, &step["settled"], false);
                assert!(!doc.can_undo());
                continue;
            }
            if post {
                commands
                    .execute_postselected(&mut doc, "_-PackSubDFaces", CommandContext::default())
                    .unwrap();
            } else {
                commands.execute(&mut doc, "PackSubDFaces").unwrap();
            }
            assert_eq!(doc.undo_label(), Some("PackSubDFaces"));
            check(&doc, &ids, &step["settled"], true);
            let after = doc.objects().cloned().collect::<Vec<_>>();
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            check(&doc, &ids, &step["undo"], false);
            doc.redo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            check(&doc, &ids, &step["redo"], true);
        }
    }
}
