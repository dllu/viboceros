use serde_json::Value;
use viboceros_command::{CommandRegistry, open_3dm_with_views};
use viboceros_document::{ObjectId, SelectionMode};
use viboceros_geometry::Point3;
#[path = "support/insert_kink.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "support/simplify_curves.rs"]
mod native_curve_comparison;
use comparison::check;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
#[test]
fn native_insert_kink_geometry_attributes_selection_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_kink.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/insert_kink");
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let r = &v["recipe"];
        let registry = CommandRegistry::with_builtins();
        let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
        let id = v["source_id"]
            .as_str()
            .unwrap()
            .parse::<ObjectId>()
            .unwrap();
        doc.clear_selection();
        if v["before"][0]["selected"] == true {
            doc.select_object(id, SelectionMode::Replace).unwrap();
        }
        doc.clear_history().unwrap();
        let before = doc.object(id).unwrap().clone();
        let mut cmd = String::from("InsertKink ");
        if let Some(range) = r["subcurve"].as_array() {
            let c = before.geometry().curve_ref().unwrap();
            cmd.push_str(&format!(
                "SubCrv={id},{},{} ",
                c.parameter_at(range[0].as_f64().unwrap()).unwrap(),
                c.parameter_at(range[1].as_f64().unwrap()).unwrap()
            ));
        }
        if r["family"].as_str().unwrap().contains("surface")
            || r["family"].as_str().unwrap().starts_with("trimmed")
        {
            cmd.push_str(&format!(
                "Direction={} Symmetrical={} Midpoints={} ",
                r["direction"].as_str().unwrap_or("U"),
                if r["symmetrical"] == true {
                    "Yes"
                } else {
                    "No"
                },
                if r["midpoints"] == true { "Yes" } else { "No" }
            ));
        }
        for p in v["picks"].as_array().unwrap() {
            let p = point(p);
            let [x, y, z] = p.to_array();
            cmd.push_str(&format!("w{x},{y},{z} "));
        }
        if r["cancel"] == true {
            /* An uncommitted Draft is deliberately discarded. */
            let mut draft = viboceros_command::insert_kink::Draft::new(&doc, id, None).unwrap();
            for p in v["picks"].as_array().unwrap() {
                draft.point(point(p), Default::default()).unwrap();
            }
        } else if name == "numeric_invalid" || name == "polysurface_rejected" {
            if name == "numeric_invalid" {
                cmd = String::from("InsertKink 0.35");
            }
            assert!(registry.execute(&mut doc, &cmd).is_err(), "{name}");
        } else {
            if r["preselect"] == false && r["subcurve"].is_null() {
                doc.select_object(id, SelectionMode::Replace).unwrap();
            }
            let result = if r["preselect"] == false {
                registry.execute_postselected(&mut doc, &cmd, Default::default())
            } else {
                registry.execute(&mut doc, &cmd)
            };
            result.unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let mut maximum = check(&doc, &v["after"], id, name);

        if r["subcurve"].is_null() {
            let after = doc.object(id).unwrap();
            assert_eq!(before.attributes(), after.attributes(), "{name}: attrs");
            assert_eq!(before.group_ids(), after.group_ids());
        }
        if !v["undo"].is_null()
            && r["family"] != "trimmed_hole"
            && r["family"] != "trimmed_rectangle"
        {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("kink.3dm");
            registry
                .execute(&mut doc, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut loaded = open_3dm_with_views(path.to_str().unwrap()).unwrap().0;
            loaded
                .select_objects_direct(doc.selected_object_ids(), SelectionMode::Replace)
                .unwrap();
            maximum = maximum.max(check(&loaded, &v["after"], id, name));
        }
        if !v["undo"].is_null() {
            assert_eq!(doc.undo_label(), Some("InsertKink"), "{name}");
            registry.execute(&mut doc, "Undo").unwrap();
            maximum = maximum.max(check(&doc, &v["undo"], id, name));
            registry.execute(&mut doc, "Redo").unwrap();
            maximum = maximum.max(check(&doc, &v["redo"], id, name));
        }
        assert!(maximum <= 1e-8, "{name}: maximum {maximum}");
        eprintln!(
            "INSERT_KINK_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
}

#[test]
fn standalone_nurbs_surfaces_match_native_underlying_surface_definitions() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/insert_kink.json"
    ))
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures/insert_kink");
    let mut count = 0;
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let r = &v["recipe"];
        if !matches!(r["family"].as_str(), Some("surface" | "surface_rational"))
            || r["cancel"] == true
        {
            continue;
        }
        let mut doc = open_3dm_with_views(root.join(format!("{name}.3dm")).to_str().unwrap())
            .unwrap()
            .0;
        let id = v["source_id"]
            .as_str()
            .unwrap()
            .parse::<ObjectId>()
            .unwrap();
        let surface =
            viboceros_command::surface_reference::surface(doc.object(id).unwrap().geometry(), None)
                .unwrap()
                .0
                .clone();
        doc.replace_object_geometries([(id, viboceros_document::Geometry::NurbsSurface(surface))])
            .unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        doc.clear_history().unwrap();
        let mut command = format!(
            "InsertKink Direction={} Symmetrical={} Midpoints={} ",
            r["direction"].as_str().unwrap_or("U"),
            if r["symmetrical"] == true {
                "Yes"
            } else {
                "No"
            },
            if r["midpoints"] == true { "Yes" } else { "No" }
        );
        for p in v["picks"].as_array().unwrap() {
            let [x, y, z] = point(p).to_array();
            command.push_str(&format!("w{x},{y},{z} "));
        }
        CommandRegistry::with_builtins()
            .execute(&mut doc, &command)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let viboceros_document::Geometry::NurbsSurface(actual) = doc.object(id).unwrap().geometry()
        else {
            panic!("{name}: surface representation");
        };
        let maximum = comparison::surface(actual, &v["after"][0]["geometry"]["surfaces"][0]);
        assert!(maximum <= 1e-8, "{name}: {maximum}");
        count += 1;
        eprintln!(
            "INSERT_KINK_SURFACE_AUDIT {}",
            serde_json::json!({"id":name,"maximum_error":maximum})
        );
    }
    assert_eq!(count, 24);
}
