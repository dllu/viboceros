use super::*;
use serde_json::Value;
use viboceros_document::{ColorRgb, ObjectAttributes, SelectionMode};

fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}
fn floats(v: &Value) -> Vec<Real> {
    serde_json::from_value(v.clone()).unwrap()
}
fn geometry(row: &Value) -> Geometry {
    let d = &row["definition"];
    match row["kind"].as_str().unwrap() {
        "surface" => Geometry::NurbsSurface(
            NurbsSurface::try_new_rational(
                d["degree"][0].as_u64().unwrap() as usize,
                d["degree"][1].as_u64().unwrap() as usize,
                d["control_count"][0].as_u64().unwrap() as usize,
                d["control_count"][1].as_u64().unwrap() as usize,
                controls(&d["control_points"]),
                floats(&d["knots_u"]),
                floats(&d["knots_v"]),
            )
            .unwrap(),
        ),
        "curve" => Geometry::NurbsCurve(
            NurbsCurve::try_new_rational(
                d["degree"].as_u64().unwrap() as usize,
                controls(&d["control_points"]),
                floats(&d["knots"]),
            )
            .unwrap(),
        ),
        "point" => Geometry::Point(point(&row["point"])),
        _ => panic!("unexpected native source"),
    }
}

#[test]
fn native_inline_subcurves_preserve_loci_direction_defaults_groups_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/uv_subcurve_input_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for recipe in capture["results"].as_array().unwrap() {
        let v = &recipe["value"];
        assert_eq!(v["success"], true, "{}", recipe["id"]);
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
        let mut doc = Document::new(tol);
        let mut ids = Vec::new();
        let mut saved = Vec::new();
        for row in v["before"].as_array().unwrap() {
            let layer_name = row["layer"].as_str().unwrap();
            let layer = if let Some(layer) = doc.layer_by_name(layer_name) {
                layer.id()
            } else {
                doc.add_layer(layer_name, ColorRgb::BLACK).unwrap()
            };
            let mut attrs = ObjectAttributes::on_layer(layer);
            if let Some(name) = row["name"].as_str() {
                attrs = attrs.with_name(name);
            }
            if let Some(text) = row["user_text"].as_str() {
                attrs = attrs.try_with_user_text("viboceros-source", text).unwrap();
            }
            let g = geometry(row);
            let id = doc
                .add_geometry_with_attributes(g.clone(), attrs.clone())
                .unwrap();
            ids.push(id);
            saved.push((id, g, attrs));
        }
        for group in v["groups_before"].as_array().unwrap() {
            let index = group["index"].as_u64().unwrap();
            let members = v["before"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    r["groups"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|g| g.as_u64() == Some(index))
                })
                .map(|(i, _)| ids[i]);
            doc.add_group(Some(group["name"].as_str().unwrap().into()), members)
                .unwrap();
        }
        doc.clear_history().unwrap();
        doc.select_objects_direct(ids.iter().skip(2).copied(), SelectionMode::Replace)
            .unwrap();
        let mut command = format!("{} Surface={}", v["command"].as_str().unwrap(), ids[0]);
        for range in v["ranges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|_| !recipe["id"].as_str().unwrap().ends_with("clear"))
        {
            command.push_str(&format!(" SubCrv={},{},{}", ids[1], range[0], range[1]));
        }
        registry.execute(&mut doc, &command).unwrap();
        let native = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .collect::<Vec<_>>();
        let local = doc
            .objects()
            .filter(|o| !ids.contains(&o.id()))
            .collect::<Vec<_>>();
        assert_eq!(native.len(), local.len(), "{}", recipe["id"]);
        for (expected, actual) in native.iter().zip(&local) {
            assert_eq!(
                actual.attributes().name(),
                expected["name"].as_str(),
                "{}",
                recipe["id"]
            );
            assert_eq!(
                doc.layer(actual.attributes().layer_id()).unwrap().name(),
                expected["layer"].as_str().unwrap()
            );
            assert_eq!(
                actual
                    .attributes()
                    .user_text()
                    .get("viboceros-source")
                    .map(String::as_str),
                expected["user_text"].as_str()
            );
            assert_eq!(
                actual.group_ids().len(),
                expected["groups"].as_array().unwrap().len()
            );
            assert!(doc.is_selected(actual.id()));
            if expected["kind"] == "point" {
                let Geometry::Point(p) = actual.geometry() else {
                    panic!("expected point");
                };
                assert!(p.distance_to(point(&expected["point"])).unwrap() < 1e-6);
            } else {
                let curve = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                for sample in expected["samples"].as_array().unwrap() {
                    let p = point(sample);
                    let t = curve.closest_parameter(p, tol).unwrap();
                    assert!(
                        curve.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                        "{} {p:?}",
                        recipe["id"]
                    );
                }
                if expected["name"].is_null() {
                    let samples = expected["samples"].as_array().unwrap();
                    assert!(
                        curve
                            .evaluate(*curve.domain().start())
                            .unwrap()
                            .distance_to(point(&samples[0]))
                            .unwrap()
                            < 1e-6
                    );
                    assert!(
                        curve
                            .evaluate(*curve.domain().end())
                            .unwrap()
                            .distance_to(point(samples.last().unwrap()))
                            .unwrap()
                            < 1e-6
                    );
                }
            }
        }
        let output_ids = local.iter().map(|o| o.id()).collect::<Vec<_>>();
        for (id, g, a) in &saved {
            assert_eq!(doc.object(*id).unwrap().geometry(), g);
            assert_eq!(doc.object(*id).unwrap().attributes(), a);
            let source = ids.iter().position(|i| i == id).unwrap();
            assert_eq!(
                doc.is_selected(*id),
                v["after"][source]["selected"].as_bool().unwrap()
            );
        }
        assert_eq!(
            doc.groups().len(),
            v["groups_after"].as_array().unwrap().len()
        );
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), ids.len());
        assert_eq!(doc.selected_object_count(), 0);
        assert_eq!(
            doc.groups().len(),
            v["groups_undo"].as_array().unwrap().len()
        );
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            output_ids.into_iter().collect()
        );
    }
}

#[test]
fn bad_ephemeral_ranges_roll_back_without_geometry_selection_or_history_changes() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,0 0,6,0")
        .unwrap();
    let surface = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Line 0,0 4,6").unwrap();
    let curve = doc.objects().last().unwrap().id();
    for name in ["ApplyCrv", "CreateUVCrv"] {
        for bad in [
            format!("{curve},0,0"),
            format!("{curve},-1,2"),
            format!("{curve},0,NaN"),
            format!("{surface},0,1"),
            format!("{curve},0"),
            format!("{},0,1", "00000000-0000-0000-0000-000000000001"),
        ] {
            let saved = format!("{doc:?}");
            assert!(
                registry
                    .execute(&mut doc, &format!("{name} Surface={surface} SubCrv={bad}"))
                    .is_err()
            );
            assert_eq!(format!("{doc:?}"), saved);
        }
    }
}
