use super::*;

fn fixture() -> ProjectedObjectSnapFixture {
    serde_json::from_value(json!({
        "sources":[{"type":"line","start":[0,0,0],"end":[10,0,0]}],
        "camera":{"world_to_screen":[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],
                  "location":[0,0,-10],"direction":[0,0,1]},
        "cursor":[4,0.5],"capture_radius":1,"modes":["Near"],"snap_to_meshes":false
    }))
    .unwrap()
}

#[test]
fn calibrated_probe_reports_source_mode_target_and_non_admission() {
    let mut fixture = fixture();
    assert_eq!(
        run(&fixture, Tolerance::DEFAULT).unwrap(),
        (json!({"kind":"Near","point":[4.,0.,0.],"source":0}), 0)
    );
    fixture.sources.insert(
        0,
        SnapSource::Line {
            start: [0., 20., 0.],
            end: [10., 20., 0.],
        },
    );
    assert_eq!(run(&fixture, Tolerance::DEFAULT).unwrap().0["source"], 1);
    fixture.modes = vec![SnapMode::Mid];
    assert_eq!(
        run(&fixture, Tolerance::DEFAULT).unwrap().0,
        json!({"kind":"Midpoint","point":[5.,0.,0.],"source":1})
    );
    fixture.modes.clear();
    assert_eq!(
        run(&fixture, Tolerance::DEFAULT).unwrap().0,
        json!({"kind":"None","point":null,"source":null})
    );
    fixture.modes = vec![SnapMode::Near];
    fixture.camera.direction = [0., 0., -1.];
    assert_eq!(run(&fixture, Tolerance::DEFAULT).unwrap().0["kind"], "None");
}

#[test]
fn invalid_camera_modes_and_source_bounds_fail_instead_of_becoming_misses() {
    let mut cases = Vec::new();
    let base = fixture();
    let mut f = base.clone();
    f.sources.clear();
    cases.push(f);
    let mut f = base.clone();
    f.sources = vec![f.sources[0].clone(); 17];
    cases.push(f);
    let mut f = base.clone();
    f.camera.direction = [0.; 3];
    cases.push(f);
    let mut f = base.clone();
    f.camera.location[0] = f64::NAN;
    cases.push(f);
    let mut f = base.clone();
    f.camera.world_to_screen[3] = [0.; 4];
    cases.push(f);
    let mut f = base.clone();
    f.camera.world_to_screen[0][1] = f64::INFINITY;
    cases.push(f);
    let mut f = base.clone();
    f.cursor[0] = f64::NAN;
    cases.push(f);
    let mut f = base.clone();
    f.modes = vec![SnapMode::Near; 2];
    cases.push(f);
    for radius in [0., 65., f64::NAN, f64::INFINITY] {
        let mut f = base.clone();
        f.capture_radius = radius;
        cases.push(f);
    }
    let mut f = base.clone();
    f.sources = vec![SnapSource::Mesh {
        vertices: vec![[0.; 3]; 4097],
        faces: vec![vec![0, 1, 2]],
    }];
    cases.push(f);
    let mut f = base;
    f.sources = vec![SnapSource::Mesh {
        vertices: vec![[0.; 3]; 3],
        faces: vec![vec![0, 1, 7]],
    }];
    cases.push(f);
    for f in cases {
        assert!(run(&f, Tolerance::DEFAULT).is_err(), "{f:?}");
    }
}

#[test]
fn protocol_replays_all_retained_point_inputs_without_observed_targets() {
    let input: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/point_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/point_snaps.json"
    ))
    .unwrap();
    let targets: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/point_snap_targets.json"
    ))
    .unwrap();
    let operations: Vec<_> = input["operations"].as_array().unwrap().iter().zip(observed["results"].as_array().unwrap()).map(|(op,row)| {
        let f = &row["value"]["frame"];
        json!({"op":"projected_object_snap","id":op["id"],"sources":op["sources"],
            "camera":{"world_to_screen":f["world_to_screen"],"location":f["camera_location"],"direction":f["camera_direction"]},
            "cursor":f["click_client"],"capture_radius":op.get("capture_radius").unwrap_or(&json!(12)),
            "modes":op["persistent_snaps"],"snap_to_meshes":op["snap_to_meshes"]})
    }).collect();
    let mut request: ProbeRequest = serde_json::from_value(
        json!({"protocol_version":1,"iterations":1,"operations":operations}),
    )
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 101);
    for row in response.results {
        let target = &targets[&row.id];
        if target.is_null() {
            assert_eq!(row.value, json!({"kind":"None","point":null,"source":null}));
        } else {
            assert_eq!(row.value["kind"], target["kind"]);
            assert_eq!(row.value["source"], 0);
            let p = |v: &Value| {
                Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
            };
            assert!(
                p(&row.value["point"])
                    .distance_to(p(&target["point"]))
                    .unwrap()
                    < 1e-9,
                "{}",
                row.id
            );
        }
    }
    request.iterations = 2;
    assert!(run_request(&request).is_err());
}
