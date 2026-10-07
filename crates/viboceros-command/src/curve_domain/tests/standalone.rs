use super::*;
use serde_json::Value;

fn p(v: &Value) -> Point3 {
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
                WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn standalone_subcurve_replays_native_geometry_groups_selection_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/standalone_subcurve.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = r["id"].as_str().unwrap().strip_prefix("subcurve_").unwrap();
        if case == "no_confirmation" {
            continue;
        }
        let mut d = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
        let input = d
            .add_layer(v["before"][0]["layer"].as_str().unwrap(), ColorRgb::BLACK)
            .unwrap();
        let source = d
            .add_geometry_with_attributes(
                Geometry::NurbsCurve(curve(&v["before"][0]["definition"])),
                ObjectAttributes::on_layer(input)
                    .with_name("source")
                    .try_with_user_text("source", "original")
                    .unwrap(),
            )
            .unwrap();
        let group = d.add_group(Some("original".into()), [source]).unwrap();
        let output = d
            .add_layer(format!("subcurve_{case}_output"), ColorRgb::BLACK)
            .unwrap();
        d.set_current_layer(output).unwrap();
        d.clear_history().unwrap();
        d.select_command_results([source]).unwrap();
        let copy = if v["copy"] == true { "Yes" } else { "No" };
        let command = if case.starts_with("point_") || case.starts_with("closed_point_") {
            let end = match case {
                "point_forward" => Point3::try_new(3., 4.5, 0.).unwrap(),
                "point_backward" => Point3::try_new(1., 1.5, 0.).unwrap(),
                _ => p(&v["confirmation"]),
            };
            format!(
                "SubCrv {} {} Copy={copy}",
                format_point(p(&v["start"])),
                format_point(end)
            )
        } else {
            let c = d.object(source).unwrap().geometry().curve_ref().unwrap();
            let confirmation = c
                .closest_parameter(p(&v["confirmation"]), d.tolerance())
                .unwrap();
            let length = if case == "zero" {
                "0"
            } else {
                v["length_token"].as_str().unwrap()
            };
            format!(
                "SubCrv Numeric={},{length},{confirmation} Copy={copy}",
                v["start_parameter"]
            )
        };
        registry.execute(&mut d, &command).unwrap();
        assert_eq!(
            d.objects().len(),
            v["after"].as_array().unwrap().len(),
            "{case}"
        );
        for (actual, expected) in d.objects().zip(v["after"].as_array().unwrap()) {
            assert_eq!(actual.attributes().name(), Some("source"));
            assert_eq!(actual.attributes().layer_id(), input);
            assert_eq!(actual.group_ids(), &[group]);
            assert_eq!(
                d.is_selected(actual.id()),
                expected["selected"].as_bool().unwrap(),
                "{case}"
            );
            let c = actual.geometry().curve_ref().unwrap().to_nurbs().unwrap();
            for station in expected["samples"].as_array().unwrap() {
                let point = p(station);
                let t = c.closest_parameter(point, d.tolerance()).unwrap();
                assert!(
                    c.evaluate(t).unwrap().distance_to(point).unwrap() < 1e-6,
                    "{case}"
                );
            }
            assert!(
                c.evaluate(*c.domain().start())
                    .unwrap()
                    .distance_to(p(&expected["samples"][0]))
                    .unwrap()
                    < 1e-6,
                "{case} start"
            );
            assert!(
                c.evaluate(*c.domain().end())
                    .unwrap()
                    .distance_to(p(&expected["samples"][32]))
                    .unwrap()
                    < 1e-6,
                "{case} end"
            );
        }
        let count = d.objects().len();
        if case == "zero" {
            assert!(!d.can_undo());
            continue;
        }
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

fn format_point(p: Point3) -> String {
    format!("{},{},{}", p.x(), p.y(), p.z())
}

#[test]
fn malformed_numeric_subcurves_do_not_modify_geometry_or_history() {
    let mut d = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut d, "Line 0,0 4,6").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_command_results([id]).unwrap();
    for value in ["0,20,1", "0,NaN,1", "-1,2,1", "0,2", "0,2,1,1"] {
        let before = format!("{d:?}");
        assert!(
            registry
                .execute(&mut d, &format!("SubCrv Numeric={value}"))
                .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
}
