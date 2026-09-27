use super::*;
use crate::CommandRegistry;
use viboceros_document::SelectionMode;
use viboceros_geometry::{Brep, Circle3, NurbsSurface, Vector3};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn uv(report: &str) -> Vec<f64> {
    report
        .split_whitespace()
        .nth(4)
        .unwrap()
        .split(',')
        .map(|x| x.parse().unwrap())
        .collect()
}

#[test]
fn uv_replays_live_rhino_reports_and_created_points() {
    use serde_json::Value;
    use viboceros_geometry::WeightedPoint3;

    let request: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/evaluate-uv-command.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/evaluate-uv-rhino-reference.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = response["results"].as_array().unwrap();
    assert_eq!(operations.len(), 4);
    assert_eq!(operations.len(), results.len());
    let registry = CommandRegistry::with_builtins();
    for (operation, result) in operations.iter().zip(results) {
        assert_eq!(operation["id"], result["id"]);
        assert_eq!(operation["op"], "evaluate_uv_command");
        let definition = &operation["surface"];
        let size = |key| definition[key].as_u64().unwrap() as usize;
        let surface = NurbsSurface::try_new_rational(
            size("degree_u"),
            size("degree_v"),
            size("control_point_count_u"),
            size("control_point_count_v"),
            definition["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|control| {
                    WeightedPoint3::try_new(
                        point(serde_json::from_value(control["point"].clone()).unwrap()),
                        control["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(definition["knots_u"].clone()).unwrap(),
            serde_json::from_value(definition["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let before = format!("{doc:?}");
        let source_before = format!("{:?}", doc.object(id).unwrap());
        let options = EvaluateUvOptions {
            normalized: operation["normalized"].as_bool().unwrap_or(false),
            create_point: operation["create_point"].as_bool().unwrap_or(false),
        };
        let [x, y, z]: [f64; 3] = serde_json::from_value(operation["point"].clone()).unwrap();
        let report = registry
            .execute(&mut doc, &format!("{} {x},{y},{z}", options.command_line()))
            .unwrap();
        let observation = &result["value"];
        let lines = observation["history"]
            .as_str()
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("UV coordinates of point = "))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1);
        let expected = lines[0]
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let actual = uv(&report);
        assert_eq!(actual.len(), 2);
        assert_eq!(expected.len(), 2);
        // These dyadic fixture values are exact even in Rhino's rounded report.
        for (a, b) in actual.iter().zip(expected) {
            assert!((a - b).abs() < 1e-8, "{}: {report}", result["id"]);
        }
        let expected: Vec<[f64; 3]> =
            serde_json::from_value(observation["created_points"].clone()).unwrap();
        let actual = doc
            .objects()
            .filter(|object| object.id() != id)
            .map(|object| match object.geometry() {
                Geometry::Point(point) => *point,
                _ => panic!("unexpected added geometry"),
            })
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), usize::from(options.create_point));
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            assert!(a.distance_to(point(b)).unwrap() < 1e-8);
        }
        assert_eq!(observation["source_geometry_unchanged"], true);
        assert_eq!(format!("{:?}", doc.object(id).unwrap()), source_before);
        if !options.create_point {
            assert_eq!(format!("{doc:?}"), before);
        }
    }
}

#[test]
fn uv_projection_and_normalization_survive_extreme_independent_parameter_scales() {
    for (u_domain, v_domain) in [(0.0..=1e12, 0.0..=1.0), (0.0..=1e-308, 0.0..=1e308)] {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let surface = NurbsSurface::try_bilinear(
            [[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]].map(point),
        )
        .unwrap()
        .try_reparameterized(u_domain, v_domain)
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
        doc.select_object(id, SelectionMode::Replace).unwrap();
        let report = registry
            .execute(
                &mut doc,
                "EvaluateUVPt Normalized=Yes CreatePoint=Yes 1.37,0.63,3",
            )
            .unwrap();
        for (a, b) in uv(&report).iter().zip([0.3425, 0.315]) {
            assert!((a - b).abs() < 1e-9);
        }
        let Geometry::Point(marker) = doc.objects().last().unwrap().geometry() else {
            panic!("marker expected")
        };
        assert!(marker.distance_to(point([1.37, 0.63, 0.])).unwrap() < 1e-9);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), 1);
    }
}

#[test]
fn uv_preferences_are_shared_with_prompts_but_not_document_history_or_other_registries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry
        .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,2,0 0,2,0")
        .unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let choices = |input| {
        registry
            .object_selection_prompt(input)
            .unwrap()
            .unwrap()
            .command_line()
    };
    registry
        .execute(
            &mut doc,
            "EvaluateUVPt Normalized=Yes CreatePoint=Yes 1,1,3",
        )
        .unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let yes = "EvaluateUVPt Normalized=Yes CreatePoint=Yes";
    assert_eq!(choices("EvaluateUVPt"), yes);
    // Describing a hypothetical invocation must not accept its choices.
    assert_eq!(
        choices("EvaluateUVPt Normalized=No"),
        "EvaluateUVPt Normalized=No CreatePoint=Yes"
    );
    assert_eq!(choices("EvaluateUVPt"), yes);
    let before = format!("{doc:?}");
    for input in [
        "EvaluateUVPt Normalized=No CreatePoint=Maybe 1,1,3",
        "EvaluateUVPt Normalized=No NaN,0,0",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(choices("EvaluateUVPt"), yes);
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .accept_object_selection_input("EvaluateUVPt CreatePoint=No")
        .unwrap();
    let report = registry.execute(&mut doc, "EvaluateUVPt 1,1,3").unwrap();
    assert_eq!(uv(&report), vec![0.25, 0.5]);
    assert_eq!(format!("{doc:?}"), before);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 2);
    assert_eq!(
        choices("EvaluateUVPt"),
        "EvaluateUVPt Normalized=Yes CreatePoint=No"
    );
    let fresh = CommandRegistry::with_builtins();
    assert_eq!(
        fresh
            .object_selection_prompt("EvaluateUVPt")
            .unwrap()
            .unwrap()
            .command_line(),
        "EvaluateUVPt Normalized=No CreatePoint=No"
    );
}

#[test]
fn uv_reports_native_and_normalized_parameters_with_undoable_projected_markers() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let surface = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [4., 0., 0.], [4., 2., 0.], [0., 2., 0.]].map(point),
    )
    .unwrap()
    .try_reparameterized(-2.0..=6.0, 10.0..=14.0)
    .unwrap();
    let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    for (options, expected) in [
        ("Normalized=No", [0., 12.]),
        ("Normalized=Yes", [0.25, 0.5]),
    ] {
        let report = registry
            .execute(&mut doc, &format!("EvaluateUVPt {options} 1,1,3"))
            .unwrap();
        for (a, b) in uv(&report).iter().zip(expected) {
            assert!((a - b).abs() < 1e-8, "{report}");
        }
        assert_eq!(format!("{doc:?}"), before);
    }
    registry
        .execute(&mut doc, "EvaluateUVPt CreatePoint=Yes 1,1,3")
        .unwrap();
    let Geometry::Point(marker) = doc.objects().last().unwrap().geometry() else {
        panic!("point marker expected")
    };
    assert!(marker.distance_to(point([1., 1., 0.])).unwrap() < 1e-8);
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), vec![id]);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().count(), 1);
    let before = format!("{doc:?}");
    for input in [
        "EvaluateUVPt",
        "EvaluateUVPt CreatePoint=Maybe 0,0,0",
        "EvaluateUVPt Normalized=Yes Normalized=No 0,0,0",
        "EvaluateUVPt CreatePoint=Yes NaN,0,0",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().count(), 2);
}

#[test]
fn uv_qualified_faces_match_live_rhinocommon_underlying_surfaces() {
    use serde_json::Value;
    use viboceros_geometry::WeightedPoint3;

    let request: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/surface-face-uv-api.json"
    ))
    .unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/surface-face-uv-rhino-reference.json"
    ))
    .unwrap();
    let operation = &request["operations"][0];
    let value = &response["results"][0]["value"];
    let mut parts = Vec::new();
    for definition in operation["surfaces"].as_array().unwrap() {
        let count = |key| definition[key].as_u64().unwrap() as usize;
        let surface = NurbsSurface::try_new_rational(
            count("degree_u"),
            count("degree_v"),
            count("control_point_count_u"),
            count("control_point_count_v"),
            definition["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|control| {
                    WeightedPoint3::try_new(
                        point(serde_json::from_value(control["point"].clone()).unwrap()),
                        control["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(definition["knots_u"].clone()).unwrap(),
            serde_json::from_value(definition["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        parts.push(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap());
    }
    let mut doc = Document::default();
    let brep = Brep::try_combine(parts, doc.tolerance()).unwrap();
    assert_eq!(
        brep.faces().len(),
        value["face_count"].as_u64().unwrap() as usize
    );
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    for (query, expected) in operation["queries"]
        .as_array()
        .unwrap()
        .iter()
        .zip(value["queries"].as_array().unwrap())
    {
        let target = point(serde_json::from_value(query["point"].clone()).unwrap());
        let face = query["face"].as_u64().unwrap() as usize;
        assert_eq!(expected["face"].as_u64().unwrap() as usize, face);
        let native = evaluate_surface_uv_on_face(
            &doc,
            target,
            EvaluateUvOptions {
                normalized: false,
                create_point: true,
            },
            Some(face),
        )
        .unwrap();
        let expected_parameters: [f64; 2] =
            serde_json::from_value(expected["parameters"].clone()).unwrap();
        for (actual, expected) in uv(&native.report).iter().zip(expected_parameters) {
            assert!((actual - expected).abs() < 1e-10);
        }
        let marker = native.marker.unwrap();
        let expected_point: [f64; 3] = serde_json::from_value(expected["point"].clone()).unwrap();
        assert!(marker.distance_to(point(expected_point)).unwrap() < 1e-10);
        assert!(
            (marker.distance_to(target).unwrap() - expected["distance"].as_f64().unwrap()).abs()
                < 1e-10
        );
        let normalized = evaluate_surface_uv_on_face(
            &doc,
            target,
            EvaluateUvOptions {
                normalized: true,
                create_point: false,
            },
            Some(face),
        )
        .unwrap();
        let expected_normalized: [f64; 2] =
            serde_json::from_value(expected["normalized_parameters"].clone()).unwrap();
        for (actual, expected) in uv(&normalized.report).iter().zip(expected_normalized) {
            assert!((actual - expected).abs() < 1e-10);
        }
        assert!(normalized.marker.is_none());
    }
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn uv_uses_the_nearest_component_surfaces_own_domain() {
    let registry = CommandRegistry::with_builtins();
    let parts = [0., 5.]
        .into_iter()
        .map(|z| {
            let surface = NurbsSurface::try_bilinear(
                [[0., 0., z], [4., 0., z], [4., 2., z], [0., 2., z]].map(point),
            )
            .unwrap();
            let surface = if z == 5. {
                surface
                    .try_reparameterized(100.0..=200.0, -2.0..=2.0)
                    .unwrap()
            } else {
                surface
            };
            Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap()
        })
        .collect();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Brep(
            Brep::try_combine(parts, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    let report = registry.execute(&mut doc, "EvaluateUVPt 1,1,4.5").unwrap();
    for (a, b) in uv(&report).iter().zip([125., 0.]) {
        assert!((a - b).abs() < 1e-8, "{report}");
    }
    let report = registry
        .execute(&mut doc, "EvaluateUVPt 1,1,4.5 Face=0")
        .unwrap();
    for (a, b) in uv(&report).iter().zip([0.25, 0.5]) {
        assert!((a - b).abs() < 1e-8, "{report}");
    }
    let report = registry
        .execute(&mut doc, "EvaluateUVPt 1,1,0.5 Face=1")
        .unwrap();
    for (a, b) in uv(&report).iter().zip([125., 0.]) {
        assert!((a - b).abs() < 1e-8, "{report}");
    }
    assert!(matches!(
        registry.execute(&mut doc, "EvaluateUVPt 1,1,0.5 Face=2"),
        Err(CommandError::EvaluateUvFaceIndexOutOfRange { .. })
    ));
    for input in [
        "EvaluateUVPt Face=0",
        "EvaluateUVPt 1,1,0.5 Face=0 Face=1",
        "EvaluateUVPt 1,1,0.5 Face=oops",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
    }
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn uv_evaluation_uses_underlying_surface_inside_a_trim_hole() {
    let registry = CommandRegistry::with_builtins();
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let circle = |radius| {
        Circle3::try_new(point([0.; 3]), radius, normal, Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let brep =
        Brep::try_planar_face_with_holes(&circle(2.), &[circle(0.5)], Tolerance::DEFAULT).unwrap();
    let (face, u, v) = brep
        .closest_underlying_face_parameters(point([0., 0., 1.]), Tolerance::DEFAULT)
        .unwrap();
    assert!(
        !brep.faces()[face]
            .contains_parameters(u, v, Tolerance::DEFAULT)
            .unwrap()
    );
    let expected = brep.faces()[face]
        .surface()
        .normalized_parameters(u, v)
        .unwrap();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = format!("{doc:?}");
    let report = registry
        .execute(&mut doc, "EvaluateUVPt Normalized=Yes 0,0,1")
        .unwrap();
    for (a, b) in uv(&report).iter().zip(expected) {
        assert!((a - b).abs() < 1e-8);
    }
    assert_eq!(format!("{doc:?}"), before);
}
