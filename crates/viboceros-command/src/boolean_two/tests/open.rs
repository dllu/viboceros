use super::*;
use viboceros_document::{ColorRgb, GroupId, LayerId, ObjectAttributes};
use viboceros_geometry::{NurbsSurface, Point3};
fn setup(v: &Value) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    let mut doc = Document::default();
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    for (i, shape) in v["shapes"].as_array().unwrap().iter().enumerate() {
        let b = if shape["kind"] == "box" {
            crate::boolean_union::tests::box_brep(
                serde_json::from_value(shape["bounds"].clone()).unwrap(),
            )
        } else {
            let points = serde_json::from_value::<[[f64; 3]; 4]>(shape["points"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            let b = Brep::try_surface_face(
                NurbsSurface::try_bilinear(points).unwrap(),
                doc.tolerance(),
            )
            .unwrap();
            if shape["reverse"] == true {
                b.reversed()
            } else {
                b
            }
        };
        let layer = doc
            .add_layer(format!("Source {i}"), ColorRgb::BLACK)
            .unwrap();
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{i}"))
            .with_object_color(ColorRgb::new(20 + i as u8, 40, 60))
            .try_with_user_text("Code", format!("attribute-{i}"))
            .unwrap();
        let id = doc
            .add_geometry_with_attributes(Geometry::Brep(b), attrs)
            .unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{i}")))
            .unwrap();
        ids.push(id);
        layers.push(layer);
    }
    let mut groups = ids
        .iter()
        .map(|&id| doc.add_group(None, [id]).unwrap())
        .collect::<Vec<_>>();
    groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
    doc.clear_history().unwrap();
    (doc, ids, layers, groups)
}
#[test]
fn boolean_two_open_replays_native_physical_boundaries_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_two_open.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        if v["cancel"] == true {
            continue;
        }
        let (mut doc, ids, layers, groups) = setup(v);
        let registry = CommandRegistry::with_builtins();
        let input = format!(
            "Boolean2Objects Sources={},{} Mode={} DeleteInput={}",
            ids[0],
            ids[1],
            Mode::ALL[v["cycles"].as_u64().unwrap() as usize % 5].name(),
            if v["delete"] == true { "Yes" } else { "No" }
        );
        let result = registry.execute(&mut doc, &input);
        assert_eq!(
            result.is_ok(),
            v["command"]["success"] == true,
            "{}: {result:?}",
            v["case"]
        );
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["command"]["after_script"],
            v["case"].as_str().unwrap(),
        );
        for (o, n) in doc
            .objects()
            .zip(v["command"]["after_script"].as_array().unwrap())
        {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            let a = regions(b);
            let expected = serde_json::from_value::<Boundary>(n["face_regions"].clone()).unwrap();
            witnesses(&a, &expected, "open Boolean2Objects");
            witnesses(&expected, &a, "open Boolean2Objects");
        }
        if result.is_ok() {
            registry.execute(&mut doc, "Undo").unwrap();
            compare(
                &snapshot(&doc, &ids, &layers, &groups),
                &v["undo"]["after_script"],
                "undo",
            );
            registry.execute(&mut doc, "Redo").unwrap();
            compare(
                &snapshot(&doc, &ids, &layers, &groups),
                &v["redo"]["after_script"],
                "redo",
            );
        } else {
            assert!(!doc.can_undo());
        }
    }
}
