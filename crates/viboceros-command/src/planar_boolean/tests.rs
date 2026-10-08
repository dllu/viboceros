use super::*;
use crate::boolean_union::tests::{Boundary, compare, regions, snapshot, witnesses};
use serde_json::Value;
use viboceros_document::{GroupId, LayerId};
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
fn planar_booleans_compute_finite_area_and_leave_solid_booleans_separate() {
    let registry = CommandRegistry::with_builtins();
    for (name, area) in [
        ("PlanarUnion", 18.),
        ("PlanarDifference", 12.),
        ("PlanarIntersection", 4.),
    ] {
        let mut doc = Document::default();
        registry.execute(&mut doc, "SrfPt 0,0 4,0 4,4 0,4").unwrap();
        registry.execute(&mut doc, "SrfPt 2,1 5,1 5,3 2,3").unwrap();
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.clear_history().unwrap();
        registry
            .execute(&mut doc, &format!("{name} Sources={},{}", ids[0], ids[1]))
            .unwrap();
        assert_eq!(doc.objects().len(), 1);
        let Geometry::Brep(b) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(b.faces().len(), 1);
        assert!((b.area(doc.tolerance()).unwrap() - area).abs() < 1e-9);
        assert!(!b.is_solid());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 1);
    }
}

#[test]
fn planar_command_failures_preserve_geometry_history_and_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0 2,2 2").unwrap();
    registry.execute(&mut doc, "Sphere 0,0 1").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for name in ["PlanarUnion", "PlanarDifference", "PlanarIntersection"] {
        assert!(
            registry
                .execute(&mut doc, &format!("{name} Sources={},{}", ids[0], ids[1]))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}

#[test]
fn planar_booleans_replay_native_geometry_defaults_identity_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_command.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let (mut doc, ids, layers, groups) = setup(v);
        let registry = CommandRegistry::with_builtins();
        let command = format!(
            "{} Sources={}",
            v["command_name"].as_str().unwrap(),
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        if v["pre"] == true {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        let result = registry.execute(&mut doc, &command);
        assert_eq!(
            result.is_ok(),
            v["command"]["success"] == true,
            "{} {result:?}",
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
            let actual = regions(b);
            let expected = serde_json::from_value::<Boundary>(n["face_regions"].clone()).unwrap();
            witnesses(&actual, &expected, "planar Boolean");
            witnesses(&expected, &actual, "planar Boolean");
        }
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
    }
}
