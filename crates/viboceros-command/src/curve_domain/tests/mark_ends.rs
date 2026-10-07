use super::*;
use serde_json::Value;
fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn source(v: &Value) -> Geometry {
    Geometry::NurbsCurve(
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
        .unwrap(),
    )
}

#[test]
fn mark_ends_matches_native_points_default_attributes_and_history() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/subcurve_mark_ends.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for recipe in capture["results"].as_array().unwrap() {
        let v = &recipe["value"];
        let case = recipe["id"]
            .as_str()
            .unwrap()
            .strip_prefix("mark_ends_")
            .unwrap();
        if case == "no_confirmation" {
            continue;
        }
        let mut d = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
        let input = d.add_layer("input", ColorRgb::BLACK).unwrap();
        let output = d.add_layer("output", ColorRgb::BLACK).unwrap();
        let geometry = source(&v["before"][0]["definition"]);
        let attrs = ObjectAttributes::on_layer(input)
            .with_name("source")
            .try_with_user_text("source", "original")
            .unwrap();
        let id = d
            .add_geometry_with_attributes(geometry.clone(), attrs.clone())
            .unwrap();
        d.add_group(Some("source".into()), [id]).unwrap();
        d.set_current_layer(output).unwrap();
        d.clear_history().unwrap();
        d.select_command_results([id]).unwrap();
        let command = if case.starts_with("point_") {
            let end = if case == "point_forward" {
                Point3::try_new(3., 4.5, 0.).unwrap()
            } else {
                Point3::try_new(1., 1.5, 0.).unwrap()
            };
            format!(
                "SubCrv {},{},{} {},{},{} Mode=MarkEnds Copy={}",
                p(&v["start"]).x(),
                p(&v["start"]).y(),
                p(&v["start"]).z(),
                end.x(),
                end.y(),
                end.z(),
                if v["copy"] == true { "Yes" } else { "No" }
            )
        } else {
            let confirmation = d
                .object(id)
                .unwrap()
                .geometry()
                .curve_ref()
                .unwrap()
                .closest_parameter(p(&v["confirmation"]), d.tolerance())
                .unwrap();
            format!(
                "SubCrv Numeric={},{},{confirmation} Mode=MarkEnds Copy={}",
                v["start_parameter"],
                if case == "zero" {
                    "0"
                } else {
                    v["length_token"].as_str().unwrap()
                },
                if v["copy"] == true { "Yes" } else { "No" }
            )
        };
        registry.execute(&mut d, &command).unwrap();
        let expected = v["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .collect::<Vec<_>>();
        let actual = d.objects().filter(|o| o.id() != id).collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len(), "{case}");
        for (actual, expected) in actual.iter().zip(expected) {
            let Geometry::Point(point) = actual.geometry() else {
                panic!("expected point")
            };
            assert!(
                point.distance_to(p(&expected["point"])).unwrap() < 1e-6,
                "{case}"
            );
            assert_eq!(actual.attributes(), &ObjectAttributes::on_layer(output));
            assert!(actual.group_ids().is_empty());
        }
        assert_eq!(d.object(id).unwrap().geometry(), &geometry);
        assert_eq!(d.object(id).unwrap().attributes(), &attrs);
        assert_eq!(d.selected_object_count(), 0);
        if case == "zero" {
            assert!(!d.can_undo());
            continue;
        }
        registry.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().len(), 1);
        assert_eq!(d.selected_object_count(), 0);
        registry.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.objects().len(), 3);
        assert_eq!(d.selected_object_count(), 0);
    }
}

#[test]
fn mark_ends_rejects_bad_modes_and_rolls_back_without_markers() {
    let mut d = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut d, "Line 0,0 4,6").unwrap();
    let id = d.objects().next().unwrap().id();
    d.select_command_results([id]).unwrap();
    for options in [
        "Mode=Invalid",
        "Mode=MarkEnds Mode=Shorten",
        "Mode=MarkEnds Copy=Maybe",
    ] {
        let before = format!("{d:?}");
        assert!(
            registry
                .execute(&mut d, &format!("SubCrv 1,1.5 3,4.5 {options}"))
                .is_err()
        );
        assert_eq!(format!("{d:?}"), before);
    }
}
