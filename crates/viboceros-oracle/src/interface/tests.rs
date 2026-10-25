use super::*;

#[test]
fn subd_appearance_requires_explicit_boolean_and_is_recorded() {
    let base = json!({"grid_snap":true,"osnap":false,"smart_track":true,"active_viewport":0,
        "display_modes":["Wireframe","Shaded","Ghosted","Wireframe"],
        "commands":["SubDDisplayToggle","Snap","'_SubDDisplayToggle"]});
    let missing: InterfaceFixture = serde_json::from_value(base.clone()).unwrap();
    assert!(run(&missing).is_err());
    for value in [
        Value::Null,
        json!(0),
        json!("Surface"),
        json!([]),
        json!({}),
    ] {
        let mut fixture = base.clone();
        fixture["subd_control_net"] = value;
        assert!(serde_json::from_value::<InterfaceFixture>(fixture).is_err());
    }
    for flat in [false, true] {
        let mut input = base.clone();
        input["subd_control_net"] = json!(flat);
        let fixture: InterfaceFixture = serde_json::from_value(input).unwrap();
        let (actual, _) = run(&fixture).unwrap();
        assert_eq!(
            actual["states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["subd_control_net"].as_bool().unwrap())
                .collect::<Vec<_>>(),
            [flat, !flat, !flat, flat]
        );
        assert_eq!(actual["states"][2]["grid_snap"], false);
    }
}

#[test]
fn drafting_aid_transitions_match_recorded_rhino_states() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/drafting_aids.json"
    ))
    .unwrap();
    let observations: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/drafting_aids.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 2);
    for (result, observed) in response
        .results
        .iter()
        .zip(observations["results"].as_array().unwrap())
    {
        assert_eq!(result.id, observed["id"]);
        let mut actual = result.value.clone();
        let expected = &observed["value"];
        for (actual_state, observed_state) in actual["states"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(expected["states"].as_array().unwrap())
        {
            let angle = actual_state["ortho_angle_degrees"].as_f64().unwrap();
            let reference = observed_state["ortho_angle_degrees"].as_f64().unwrap();
            assert!((angle - reference).abs() <= 1e-12);
            actual_state["ortho_angle_degrees"] = observed_state["ortho_angle_degrees"].clone();
        }
        assert_eq!(actual, *expected);
    }
}

#[test]
fn ortho_fixture_records_switch_and_angle_transitions() {
    let fixture: InterfaceFixture = serde_json::from_value(json!({
        "grid_snap": false,
        "ortho": false,
        "ortho_snap_to_cplane_z": false,
        "ortho_angle_degrees": 90.0,
        "osnap": false,
        "smart_track": false,
        "active_viewport": 0,
        "display_modes": ["Wireframe", "Wireframe", "Wireframe", "Wireframe"],
        "commands": ["SetOrtho On", "OrthoAngle 45", "Ortho", "OrthoSnapToCPlaneZ Enable", "OrthoSnapToCPlaneZ Toggle"]
    }))
    .unwrap();
    let (result, _) = run(&fixture).unwrap();
    assert_eq!(result["states"][0]["ortho"], false);
    assert_eq!(result["states"][1]["ortho"], true);
    assert_eq!(result["states"][2]["ortho_angle_degrees"], 45.0);
    assert_eq!(result["states"][3]["ortho"], false);
    assert_eq!(result["states"][4]["ortho_snap_to_cplane_z"], true);
    assert_eq!(result["states"][5]["ortho_snap_to_cplane_z"], false);
}

#[test]
fn planar_fixture_records_switch_transitions() {
    let fixture: InterfaceFixture = serde_json::from_value(json!({
        "grid_snap": false,
        "planar": false,
        "osnap": false,
        "smart_track": false,
        "active_viewport": 0,
        "display_modes": ["Wireframe", "Wireframe", "Wireframe", "Wireframe"],
        "commands": ["SetPlanar On", "Planar", "SetPlanar Toggle"]
    }))
    .unwrap();
    let (result, _) = run(&fixture).unwrap();
    assert_eq!(result["states"][0]["planar"], false);
    assert_eq!(result["states"][1]["planar"], true);
    assert_eq!(result["states"][2]["planar"], false);
    assert_eq!(result["states"][3]["planar"], true);
}

#[test]
fn mesh_snap_switch_states_match_rhino_without_changing_other_settings() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/mesh_snap_switches.json"
    ))
    .unwrap();
    let observations: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/mesh_snap_switches.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 2);
    for (result, observed) in response
        .results
        .iter()
        .zip(observations["results"].as_array().unwrap())
    {
        assert_eq!(result.id, observed["id"]);
        assert_eq!(result.value, observed["value"]);
    }
}

#[test]
fn permanent_interface_fixtures_cover_initial_switch_states_and_every_viewport() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/interface_commands.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    let mut initials = BTreeSet::new();
    let mut viewports = BTreeSet::new();
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::InterfaceCommands { fixture, .. } = operation else {
            panic!("interface fixture")
        };
        initials.insert((fixture.grid_snap, fixture.osnap, fixture.smart_track));
        viewports.insert(fixture.active_viewport);
        assert_eq!(result.elapsed_ns, 0);
        assert_eq!(
            result.value["states"].as_array().unwrap().len(),
            fixture.commands.len() + 1
        );
        assert_eq!(result.value["states"][0]["osnap"], fixture.osnap);
        assert_eq!(
            result.value["states"][0]["active_viewport"],
            fixture.active_viewport
        );
    }
    assert_eq!(initials.len(), 8);
    assert_eq!(viewports.len(), 4);
}

#[test]
fn interface_probes_reject_invalid_state_and_unrestricted_macros() {
    let mut fixture = InterfaceFixture {
        subd_control_net: None,
        snap_to_meshes: None,
        grid_snap: true,
        ortho: None,
        ortho_snap_to_cplane_z: None,
        planar: None,
        ortho_angle_degrees: None,
        osnap: true,
        smart_track: false,
        active_viewport: 0,
        display_modes: [
            "Wireframe".into(),
            "Shaded".into(),
            "Ghosted".into(),
            "Wireframe".into(),
        ],
        commands: vec!["Snap".into()],
    };
    for command in [
        "Delete",
        "Snap _Delete",
        "SetDisplayMode Viewport=All",
        "Help",
        "SetSnap On\n_Delete",
        "SnapToMeshes Toggle", // An explicit initial mesh setting is required.
    ] {
        fixture.commands = vec![command.into()];
        assert!(run(&fixture).is_err());
    }
    fixture.commands = vec!["Snap".into(); 129];
    assert!(run(&fixture).is_err());
    fixture.commands = vec![];
    assert!(run(&fixture).is_err());
    fixture.commands = vec!["Snap".into()];
    fixture.active_viewport = 4;
    assert!(run(&fixture).is_err());
    fixture.active_viewport = 0;
    fixture.display_modes[2] = "Rendered".into();
    assert!(run(&fixture).is_err());
}

#[test]
fn optional_mesh_setting_requires_an_explicit_boolean_when_present() {
    let base = json!({
        "grid_snap":false, "osnap":true, "smart_track":false, "active_viewport":0,
        "display_modes":["Wireframe", "Wireframe", "Wireframe", "Wireframe"],
        "commands":["Snap"]
    });
    let absent: InterfaceFixture = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(absent.snap_to_meshes, None);
    for value in [Value::Null, json!(0), json!("Enable"), json!([]), json!({})] {
        let mut input = base.clone();
        input["snap_to_meshes"] = value;
        assert!(serde_json::from_value::<InterfaceFixture>(input).is_err());
    }
    for enabled in [false, true] {
        let mut input = base.clone();
        input["snap_to_meshes"] = json!(enabled);
        let fixture: InterfaceFixture = serde_json::from_value(input).unwrap();
        assert_eq!(fixture.snap_to_meshes, Some(enabled));
    }
}
