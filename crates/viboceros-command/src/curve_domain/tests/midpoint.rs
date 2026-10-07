use super::*;
use serde_json::Value;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn midpoint_subcurves_replay_native_radius_clamping_seams_and_markers() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/subcurve_midpoint.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = r["id"].as_str().unwrap().strip_prefix("midpoint_").unwrap();
        let mut d = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
        let input = d.add_layer("input", ColorRgb::BLACK).unwrap();
        let output = d.add_layer("output", ColorRgb::BLACK).unwrap();
        let geometry = Geometry::NurbsCurve(curve(&v["before"][0]["definition"]));
        let attrs = ObjectAttributes::on_layer(input)
            .with_name("source")
            .try_with_user_text("source", "original")
            .unwrap();
        let source = d
            .add_geometry_with_attributes(geometry.clone(), attrs.clone())
            .unwrap();
        d.add_group(Some("source".into()), [source]).unwrap();
        d.set_current_layer(output).unwrap();
        d.clear_history().unwrap();
        d.select_command_results([source]).unwrap();
        let mut command = if case.starts_with("point_") || case == "quadratic_point" {
            let end = match case {
                "point_forward" => Point3::try_new(3., 4.5, 0.).unwrap(),
                "point_backward" => Point3::try_new(1., 1.5, 0.).unwrap(),
                _ => point(&v["confirmation"]),
            };
            let center = point(&v["start"]);
            format!(
                "SubCrv {},{},{} {},{},{}",
                center.x(),
                center.y(),
                center.z(),
                end.x(),
                end.y(),
                end.z()
            )
        } else {
            format!(
                "SubCrv Numeric={},{},{}",
                v["start_parameter"],
                if case == "zero" {
                    "0"
                } else {
                    v["length_token"].as_str().unwrap()
                },
                v["start_parameter"]
            )
        };
        command.push_str(&format!(
            " FromMidpoint=Yes Copy={} Mode={}",
            if v["copy"] == true { "Yes" } else { "No" },
            if case.starts_with("mark_") {
                "MarkEnds"
            } else {
                "Shorten"
            }
        ));
        registry.execute(&mut d, &command).unwrap();
        assert_eq!(
            d.objects().len(),
            v["after"].as_array().unwrap().len(),
            "{case}"
        );
        for (actual, expected) in d.objects().zip(v["after"].as_array().unwrap()) {
            assert_eq!(
                d.is_selected(actual.id()),
                expected["selected"].as_bool().unwrap(),
                "{case}"
            );
            if expected.get("point").is_some() {
                let Geometry::Point(p) = actual.geometry() else {
                    panic!()
                };
                assert!(
                    p.distance_to(point(&expected["point"])).unwrap() < 1e-6,
                    "{case}"
                );
                assert_eq!(actual.attributes(), &ObjectAttributes::on_layer(output));
                assert!(actual.group_ids().is_empty());
            } else {
                assert_eq!(actual.attributes(), &attrs);
                let c = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                for sample in expected["samples"].as_array().unwrap() {
                    let p = point(sample);
                    let t = c.closest_parameter(p, d.tolerance()).unwrap();
                    assert!(
                        c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-6,
                        "{case}"
                    );
                }
                assert!(
                    c.evaluate(*c.domain().start())
                        .unwrap()
                        .distance_to(point(&expected["samples"][0]))
                        .unwrap()
                        < 1e-6,
                    "{case} start"
                );
                assert!(
                    c.evaluate(*c.domain().end())
                        .unwrap()
                        .distance_to(point(&expected["samples"][32]))
                        .unwrap()
                        < 1e-6,
                    "{case} end"
                );
            }
        }
        if v["copy"] == true || case.starts_with("mark_") || v["success"] == false {
            assert_eq!(d.object(source).unwrap().geometry(), &geometry);
        }
        if v["success"] == false {
            assert!(!d.can_undo());
            continue;
        }
        let count = d.objects().len();
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().len(), 1);
        assert_eq!(d.selected_object_count(), 0);
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.objects().len(), count);
        assert_eq!(
            d.selected_object_count(),
            v["redo"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count()
        );
    }
}

#[test]
fn midpoint_option_and_radius_failures_are_atomic() {
    let mut d = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut d, "Line 0,0 4,6").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_command_results([id]).unwrap();
    for options in [
        "FromMidpoint=Maybe",
        "FromMidpoint=Yes FromMidpoint=No",
        "FromMidpoint=Yes Mode=Unknown",
    ] {
        let before = format!("{d:?}");
        assert!(
            registry
                .execute(&mut d, &format!("SubCrv Numeric=3,2,3 {options}"))
                .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
    let before = format!("{d:?}");
    assert!(
        registry
            .execute(&mut d, "SubCrv Numeric=3,20,3 FromMidpoint=Yes")
            .is_err()
    );
    assert_eq!(format!("{d:?}"), before);
}
