use super::*;

const NURBS: &str = include_str!("../../../../tools/rhino_oracle/fixtures/nurbs_conversion.json");
const SESSIONS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/nurbs_conversion_sessions.json");

#[test]
fn permanent_nurbs_conversions_preserve_source_identity_or_copy_source_attributes() {
    let request: ProbeRequest = serde_json::from_str(NURBS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 78);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::NurbsConversion { fixture, .. } = operation else {
            panic!()
        };
        let before = result.value["before"]["objects"].as_array().unwrap();
        let after = result.value["after"]["objects"].as_array().unwrap();
        assert_eq!(
            result.value["before"]["groups"].as_array().unwrap().len(),
            3
        );
        assert_eq!(result.value["after"]["groups"].as_array().unwrap().len(), 3);
        assert_eq!(
            after.iter().filter(|o| !o["original"].is_null()).count(),
            before.len()
        );
        if fixture.geometry.delete_input == Some(true) {
            assert_eq!(after.len(), before.len());
        }
        for object in after {
            let source = before.iter().find(|s| s["name"] == object["name"]).unwrap();
            for key in ["name", "groups", "layer", "color", "color_source"] {
                assert_eq!(object[key], source[key], "{} {key}", result.id);
            }
            assert_eq!(
                object["selected"],
                if object["original"].is_null() {
                    json!(false)
                } else {
                    source["selected"].clone()
                }
            );
            if object["kind"] == "curve" {
                assert!(!object["definition"].is_null());
            }
        }
    }
}

#[test]
fn nurbs_sessions_accept_real_changes_but_do_not_accept_noop_options() {
    let request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 6);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::ConversionSession { fixture, .. } = operation else {
            panic!()
        };
        let states = result.value["states"].as_array().unwrap();
        assert_eq!(states.len(), fixture.steps.len());
        let mut delete = None;
        for (step, state) in fixture.steps.iter().zip(states) {
            if step.command != ConversionCommand::ToNURBS {
                continue;
            }
            let changed = state["before"] != state["after"];
            if changed {
                delete = step.conversion.geometry.delete_input.or(delete);
                let before = state["before"]["objects"].as_array().unwrap();
                let after = state["after"]["objects"].as_array().unwrap();
                assert_eq!(after.len() == before.len(), delete.unwrap());
            }
        }
    }
}

#[test]
fn nurbs_sessions_cannot_be_seeded_by_noops_or_unset_mesh_choices() {
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    let source = fixture.steps[0].conversion.geometry.sources[0]
        .geometry(Tolerance::DEFAULT)
        .unwrap();
    let curve = source.nurbs_curve_representation().unwrap().unwrap();
    let nurbs: ObjectSource=serde_json::from_value(json!({"type":"nurbs","degree":curve.degree(),"control_points":curve.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),"knots":curve.knots()})).unwrap();
    fixture.steps[0].conversion.geometry.sources = vec![nurbs];
    assert!(matches!(
        run(fixture, Tolerance::DEFAULT),
        Err(ProbeError::FixtureInvariant(_))
    ));
    let mut request: ProbeRequest = serde_json::from_str(SESSIONS).unwrap();
    let Operation::ConversionSession { fixture, .. } = &mut request.operations[3] else {
        panic!()
    };
    fixture.steps[0].conversion.trim_triangular_faces = None;
    assert!(matches!(
        run(fixture, Tolerance::DEFAULT),
        Err(ProbeError::FixtureInvariant(_))
    ));
}
