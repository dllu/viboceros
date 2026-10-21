//! Native reference inputs retain source objects, grip state and grouped history.
use super::subd_native;
use serde_json::{Value, json};
use viboceros_document::{
    ControlPointId, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode,
};
use viboceros_geometry::{LineSegment, NurbsCurve, Point3, Tolerance, WeightedPoint3};
pub struct Scene {
    pub document: Document,
    pub sources: Vec<ObjectId>,
    original: Vec<GeometrySnapshot>,
}
pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for (text, count) in [
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/subd_cylinder_references.json"
            ),
            20,
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/observations/subd_cylinder_reference_degenerate.json"
            ),
            2,
        ),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        let found = data["results"].as_array().unwrap();
        assert_eq!(found.len(), count);
        rows.extend(found.iter().cloned());
    }
    rows
}
pub fn point(p: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap()).unwrap()
}
pub fn setup(value: &Value) -> Scene {
    setup_on_layer(value, "SubDCylinder output")
}
pub fn setup_on_layer(value: &Value, layer_name: &str) -> Scene {
    let input = &value["input"];
    let mut document = Document::default();
    let layer = document
        .add_layer(layer_name, viboceros_document::ColorRgb::new(0, 0, 255))
        .unwrap();
    document.set_current_layer(layer).unwrap();
    let mut sources = Vec::new();
    for c in input["curves"].as_array().into_iter().flatten() {
        let g = if c["type"] == "circle" {
            Geometry::Circle(
                viboceros_geometry::Circle3::try_from_frame(
                    point(&c["center"]),
                    c["radius"].as_f64().unwrap(),
                    viboceros_geometry::Vector3::try_from(point(&c["x_axis"]).to_array())
                        .unwrap()
                        .normalized_nonzero()
                        .unwrap(),
                    viboceros_geometry::Vector3::try_from(point(&c["normal"]).to_array())
                        .unwrap()
                        .normalized_nonzero()
                        .unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            )
        } else {
            Geometry::Line(
                LineSegment::try_new(point(&c["start"]), point(&c["end"]), Tolerance::DEFAULT)
                    .unwrap(),
            )
        };
        sources.push(document.add_geometry(g).unwrap());
    }
    if let Some(points) = input["controls"].as_array() {
        let points = points.iter().map(point).collect::<Vec<_>>();
        let base = NurbsCurve::try_clamped_uniform(2, points.clone()).unwrap();
        let curve = NurbsCurve::try_new_rational(
            2,
            points
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint3::try_new(p, input["weights"][i].as_f64().unwrap_or(1.)).unwrap()
                })
                .collect(),
            base.knots().to_vec(),
        )
        .unwrap();
        let parent = document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        sources.push(parent);
        document.enable_control_points([parent]).unwrap();
        document
            .select_control_points(
                input["selected_grips"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| ControlPointId {
                        object: parent,
                        index: i.as_u64().unwrap() as usize,
                    }),
                SelectionMode::Add,
            )
            .unwrap();
    }
    let points = input["points"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| document.add_geometry(Geometry::Point(point(p))).unwrap())
        .collect::<Vec<_>>();
    sources.extend(&points);
    document
        .select_objects_direct(
            input["selected"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|i| points[i.as_u64().unwrap() as usize]),
            SelectionMode::Add,
        )
        .unwrap();
    document
        .select_objects_direct(
            input["selected_curves"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|i| sources[i.as_u64().unwrap() as usize]),
            SelectionMode::Add,
        )
        .unwrap();
    document
        .set_object_names(
            sources
                .iter()
                .enumerate()
                .map(|(i, &id)| (id, Some(format!("reference {i}")))),
        )
        .unwrap();
    let original = sources
        .iter()
        .map(|&id| document.object(id).unwrap().geometry_snapshot().clone())
        .collect();
    document.clear_history().unwrap();
    Scene {
        document,
        sources,
        original,
    }
}
pub fn actions(value: &Value) -> Vec<Value> {
    let mut actions = value["input"]["actions"].as_array().unwrap().clone();
    for trace in value["input_trace"].as_array().into_iter().flatten() {
        for click in trace["clicks"].as_array().into_iter().flatten() {
            let i = click["action"].as_u64().unwrap() as usize;
            if click["projected_point"].is_array() {
                actions[i]["pick"] = click["projected_point"].clone();
            }
        }
    }
    actions
}
pub fn token(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.into()
    } else if let Some(p) = v.as_array() {
        p.iter()
            .map(|x| x.as_f64().unwrap().to_string())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        v.to_string()
    }
}
#[allow(dead_code)]
pub fn command(scene: &mut Scene, value: &Value) -> (String, bool) {
    command_for("SubDCylinder", scene, value)
}
pub fn command_for(name: &str, scene: &mut Scene, value: &Value) -> (String, bool) {
    let fit = value["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a == "FitPoints");
    let mut words = vec![name.to_owned()];
    let mut postselected = false;
    if fit
        && viboceros_command::circle_fit_points::selected_points(&scene.document)
            .take(3)
            .count()
            < 3
    {
        postselected = true;
        scene.document.clear_selection();
        let ids = scene
            .document
            .objects()
            .filter(|o| matches!(o.geometry(), Geometry::Point(_)))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        scene
            .document
            .select_objects_direct(ids, SelectionMode::Add)
            .unwrap();
        scene.document.select_all_control_points();
    }
    for action in actions(value) {
        if fit && (action == "SelAll" || action == "Enter") {
            continue;
        }
        if action.is_object() {
            words.push(format!(
                "Curve={}",
                scene.sources[action["curve"].as_u64().unwrap() as usize]
            ));
            if let Some(pick) = action.get("pick") {
                words.push(token(pick));
            }
        } else {
            words.push(token(&action));
            if action == "AroundCurve"
                && let Some(index) = value["input"]["selected_curves"]
                    .as_array()
                    .and_then(|v| v.first())
                    .and_then(Value::as_u64)
            {
                words.push(format!("Curve={}", scene.sources[index as usize]));
            }
        }
    }
    (words.join(" "), postselected)
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let targets = value[phase].as_array().unwrap();
    assert_eq!(
        scene.document.objects().count(),
        targets.len(),
        "{}",
        value["input"]
    );
    let mut objects = Vec::new();
    for target in targets {
        let object = if let Some(i) = target["source"].as_u64() {
            let i = i as usize;
            let object = scene.document.object(scene.sources[i]).unwrap();
            assert!(scene.original[i].shares_storage_with(object.geometry_snapshot()));
            object
        } else {
            scene
                .document
                .objects()
                .find(|o| !scene.sources.contains(&o.id()))
                .unwrap()
        };
        assert_eq!(
            scene.document.is_selected(object.id()),
            target["selected"].as_bool().unwrap(),
            "phase={phase}, input={}",
            value["input"]
        );
        assert_eq!(object.attributes().name(), target["name"].as_str());
        assert_eq!(
            scene
                .document
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            target["layer"].as_str().unwrap()
        );
        if let Some(expected) = target.get("subd") {
            let Geometry::SubD(subd) = object.geometry() else {
                panic!("expected SubD")
            };
            objects.push(subd_native::compare(subd, expected));
        } else if let Some(expected) = target.get("grips_on") {
            assert_eq!(
                scene
                    .document
                    .control_point_locations(object.id())
                    .is_some(),
                expected.as_bool().unwrap()
            );
            let grips = scene
                .document
                .control_points()
                .filter(|(id, _, _)| id.object == object.id())
                .collect::<Vec<_>>();
            let expected = target["grips"].as_array().unwrap();
            assert_eq!(grips.len(), expected.len());
            for ((_, p, selected), q) in grips.into_iter().zip(expected) {
                assert_eq!(selected, q["selected"].as_bool().unwrap());
                assert!(p.distance_to(point(&q["point"])).unwrap() < 1e-9);
            }
        }
    }
    json!({"qualified":true,"objects":objects,"attributes_selection_grips_and_source":true})
}
