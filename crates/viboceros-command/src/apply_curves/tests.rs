use super::*;
use serde_json::Value;

fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}
fn floats(value: &Value) -> Vec<Real> {
    serde_json::from_value(value.clone()).unwrap()
}
fn controls(value: &Value) -> Vec<WeightedPoint3> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|c| WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        controls(&value["control_points"]),
        floats(&value["knots"]),
    )
    .unwrap()
}
fn surface(value: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        value["degree"][0].as_u64().unwrap() as usize,
        value["degree"][1].as_u64().unwrap() as usize,
        value["control_count"][0].as_u64().unwrap() as usize,
        value["control_count"][1].as_u64().unwrap() as usize,
        controls(&value["control_points"]),
        floats(&value["knots_u"]),
        floats(&value["knots_v"]),
    )
    .unwrap()
}

#[test]
fn replays_native_mapping_metadata_selection_and_undo_redo() {
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/apply_uv_curves_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for row in capture["results"].as_array().unwrap() {
        let value = &row["value"];
        let mut document = Document::default();
        document.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
        let input_layer = document.current_layer_id();
        let target = document
            .add_geometry(Geometry::NurbsSurface(surface(&value["surface"])))
            .unwrap();
        let mut sources = Vec::new();
        for source in value["inputs"].as_array().unwrap() {
            let geometry = if source["kind"] == "point" {
                Geometry::Point(p(&source["point"]))
            } else {
                Geometry::NurbsCurve(curve(&source["definition"]))
            };
            let attributes = ObjectAttributes::on_layer(input_layer)
                .try_with_user_text("viboceros-source", source["name"].as_str().unwrap())
                .unwrap();
            let id = document
                .add_geometry_with_attributes(geometry, attributes)
                .unwrap();
            document
                .set_object_names([(id, Some(source["name"].as_str().unwrap().to_owned()))])
                .unwrap();
            sources.push(id);
        }
        let group = document
            .add_group(
                Some("source".into()),
                sources.iter().copied().chain([target]),
            )
            .unwrap();
        let current = document.add_layer("output", ColorRgb::BLACK).unwrap();
        document.set_current_layer(current).unwrap();
        document.clear_history().unwrap();
        document
            .select_objects_direct(sources.clone(), SelectionMode::Replace)
            .unwrap();
        let saved = document
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        registry
            .execute(&mut document, &format!("ApplyCurves Surface={target}"))
            .unwrap_or_else(|e| panic!("{}: {e:?}", row["id"]));
        let native = value["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["source"].is_null())
            .collect::<Vec<_>>();
        let outputs = document
            .objects()
            .filter(|o| !sources.contains(&o.id()) && o.id() != target)
            .collect::<Vec<_>>();
        assert_eq!(outputs.len(), native.len(), "{}", row["id"]);
        assert_eq!(
            document.groups().len(),
            value["groups_after"].as_array().unwrap().len()
        );
        for reference in native {
            let name = reference["name"].as_str().unwrap();
            let output = outputs
                .iter()
                .find(|o| o.attributes().name() == Some(name))
                .unwrap();
            assert!(document.is_selected(output.id()));
            assert_eq!(output.attributes().layer_id(), input_layer);
            assert_eq!(
                output
                    .attributes()
                    .user_text()
                    .get("viboceros-source")
                    .unwrap(),
                name
            );
            if let Geometry::Point(point) = output.geometry() {
                assert!(point.distance_to(p(&reference["point"])).unwrap() < 1e-6);
                assert!(output.group_ids().is_empty());
            } else {
                assert!(!output.group_ids().contains(&group));
                assert_eq!(
                    output.group_ids().len(),
                    reference["groups"].as_array().unwrap().len()
                );
                let model = output.geometry().curve_ref().unwrap().to_nurbs().unwrap();
                for sample in reference["samples"].as_array().unwrap() {
                    let native = p(sample);
                    let t = model
                        .closest_parameter(native, document.tolerance())
                        .unwrap();
                    assert!(
                        model.evaluate(t).unwrap().distance_to(native).unwrap() < 1e-6,
                        "{} {name}",
                        row["id"]
                    );
                }
            }
        }
        for (id, geometry, attributes, groups) in &saved {
            let object = document.object(*id).unwrap();
            assert_eq!(object.geometry(), geometry);
            assert_eq!(object.attributes(), attributes);
            assert_eq!(object.group_ids(), groups);
        }
        if !outputs.is_empty() {
            let ids = outputs.iter().map(|o| o.id()).collect::<Vec<_>>();
            registry.execute(&mut document, "Undo").unwrap();
            assert_eq!(document.objects().len(), saved.len());
            assert_eq!(document.selected_object_count(), 0);
            assert_eq!(
                document.groups().len(),
                value["groups_undo"].as_array().unwrap().len()
            );
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(
                document.selected_object_ids().collect::<BTreeSet<_>>(),
                ids.into_iter().collect()
            );
        } else {
            assert_eq!(document.selected_object_count(), sources.len() + 1);
            assert!(!document.can_undo());
            assert_eq!(
                registry.execute(&mut document, "Undo").unwrap(),
                "Nothing to undo"
            );
            assert_eq!(document.selected_object_count(), 0);
            registry.execute(&mut document, "Redo").unwrap();
            assert_eq!(document.selected_object_count(), 0);
        }
    }
}

#[test]
fn failed_curve_certificate_rolls_back_every_output_and_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let target = d
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                point(0., 0.),
                point(1., 0.),
                point(1., 1.),
                point(0., 1.),
            ])
            .unwrap(),
        ))
        .unwrap();
    let rectangle = d
        .add_geometry(Geometry::Polyline(
            Polyline3::try_new(
                vec![
                    point(0., 0.),
                    point(1., 0.),
                    point(1., 1.),
                    point(0., 1.),
                    point(0., 0.),
                ],
                d.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let mixed = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(0.2, 0.2), 1.).unwrap(),
            WeightedPoint3::try_new(point(0.5, 0.5), -0.1).unwrap(),
            WeightedPoint3::try_new(point(0.8, 0.8), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let bad = d.add_geometry(Geometry::NurbsCurve(mixed)).unwrap();
    d.select_objects_direct([rectangle, bad], SelectionMode::Replace)
        .unwrap();
    let before = d
        .objects()
        .map(|o| (o.id(), o.geometry().clone()))
        .collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(
        registry
            .execute(&mut d, &format!("ApplyCrv Surface={target}"))
            .is_err()
    );
    assert_eq!(
        d.objects()
            .map(|o| (o.id(), o.geometry().clone()))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(d.undo_label(), history.as_deref());
    assert_eq!(
        d.selected_object_ids().collect::<BTreeSet<_>>(),
        [rectangle, bad].into_iter().collect()
    );
}

#[test]
fn invalid_arguments_and_unselectable_targets_do_not_edit() {
    let mut d = Document::default();
    let registry = CommandRegistry::with_builtins();
    for input in [
        "ApplyCrv",
        "ApplyCrv Surface=bad",
        "ApplyCrv Target=bad",
        "ApplyCurves Surface=bad extra",
    ] {
        assert!(registry.execute(&mut d, input).is_err());
        assert_eq!(d.objects().len(), 0);
    }
    assert!(registry.recognizes("ApplyCurves"));
}
