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

fn retained_differences(input: &str, observations: &str) -> Vec<String> {
    retained_differences_with_epsilon(input, observations, 1e-9)
}

fn retained_differences_with_epsilon(
    input: &str,
    observations: &str,
    point_epsilon: f64,
) -> Vec<String> {
    let input: Value = serde_json::from_str(input).unwrap();
    let observed: Value = serde_json::from_str(observations).unwrap();
    let operations = input["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), rows.len());
    let mut differences = Vec::new();
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let value = &row["value"];
        let frame = &value["frame"];
        let fixture: ProjectedObjectSnapFixture = serde_json::from_value(json!({
            "sources":op["sources"],
            "camera":{"world_to_screen":frame["world_to_screen"],"location":frame["camera_location"],"direction":frame["camera_direction"]},
            "cursor":frame["click_client"],"capture_radius":op.get("capture_radius").unwrap_or(&json!(12)),
            "modes":op["persistent_snaps"],"snap_to_meshes":op["snap_to_meshes"]
        })).unwrap();
        let actual = run(&fixture, Tolerance::DEFAULT).unwrap().0;
        // Admission and source/kind must agree even in unresolved target cases.
        assert_eq!(actual["kind"], value["kind"], "{}", op["id"]);
        assert_eq!(actual["source"], value["source"], "{}", op["id"]);
        if value["kind"] == "None" {
            assert!(actual["point"].is_null());
        } else if (0..3).any(|i| {
            (actual["point"][i].as_f64().unwrap() - value["point"][i].as_f64().unwrap()).abs()
                > point_epsilon
        }) {
            differences.push(op["id"].as_str().unwrap().to_owned());
        }
    }
    differences
}

#[test]
fn mesh_vertex_snap_replays_owned_rhino_picks() {
    let fixture = include_str!("../../../../tools/rhino_oracle/fixtures/mesh_vertex_snaps.json");
    let observation =
        include_str!("../../../../tools/rhino_oracle/observations/mesh_vertex_snaps.json");
    let request: Value = serde_json::from_str(fixture).unwrap();
    let observed: Value = serde_json::from_str(observation).unwrap();
    assert_eq!(request["operations"].as_array().unwrap().len(), 6);
    for (op, row) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        if op["persistent_snaps"][0] == "Vertex" {
            assert_eq!(
                row["value"]["component"],
                json!({"type":"MeshVertex","index":0})
            );
            assert_eq!(row["value"]["point"], json!([0., 0., 0.]));
        } else {
            assert_eq!(row["value"]["kind"], "None");
        }
    }
    assert!(retained_differences(fixture, observation).is_empty());
    let mixed_fixture =
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_vertex_mixed_snaps.json");
    let mixed_observation =
        include_str!("../../../../tools/rhino_oracle/observations/mesh_vertex_mixed_snaps.json");
    let mixed: Value = serde_json::from_str(mixed_observation).unwrap();
    assert_eq!(mixed["results"].as_array().unwrap().len(), 3);
    for row in mixed["results"].as_array().unwrap() {
        assert_eq!(row["value"]["kind"], "Vertex");
        assert_eq!(row["value"]["point"], json!([0., 0., 0.]));
    }
    assert!(retained_differences(mixed_fixture, mixed_observation).is_empty());
    let aperture_fixture =
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_vertex_aperture_snaps.json");
    let aperture_observation =
        include_str!("../../../../tools/rhino_oracle/observations/mesh_vertex_aperture_snaps.json");
    let aperture: Value = serde_json::from_str(aperture_observation).unwrap();
    assert_eq!(aperture["results"].as_array().unwrap().len(), 3);
    assert_eq!(aperture["results"][0]["value"]["kind"], "Vertex");
    assert_eq!(aperture["results"][1]["value"]["kind"], "Vertex");
    assert_eq!(aperture["results"][2]["value"]["kind"], "Near");
    assert!(retained_differences(aperture_fixture, aperture_observation).is_empty());
}

#[test]
fn straight_intersection_snaps_replay_owned_rhino_picks() {
    for (fixture, observed) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/intersection_snaps.json"),
            include_str!("../../../../tools/rhino_oracle/observations/intersection_snaps.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/intersection_depth_snaps.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_depth_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_perspective_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_perspective_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_perspective_reverse_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_perspective_reverse_snaps.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/intersection_mixed_snaps.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_mixed_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_competing_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_competing_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_competing_mid_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_competing_mid_snaps.json"
            ),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/intersection_self_snaps.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_self_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_mesh_self_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_mesh_self_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_line_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_line_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_line_detail_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_line_detail_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_arc_ellipse_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_arc_ellipse_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_arc_ellipse_priority_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_arc_ellipse_priority_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_circle_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_circle_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_circle_detail_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_circle_detail_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_circle_overlap_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_circle_overlap_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_circle_seams_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_circle_seams_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_circle_circle_quadrants_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_circle_circle_quadrants_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_conic_pairs_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_conic_pairs_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_nurbs_line_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_nurbs_line_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_nurbs_spans_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_nurbs_spans_snaps.json"
            ),
        ),
    ] {
        let differences = retained_differences(fixture, observed);
        assert!(differences.is_empty(), "{differences:?}");
    }
}

#[test]
fn near_tangent_intersections_replay_owned_rhino_picks() {
    for (fixture, observed) in [
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_near_tangent_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_near_tangent_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_near_tangent_cursor_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_near_tangent_cursor_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_near_tangent_frames_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_near_tangent_frames_snaps.json"
            ),
        ),
        (
            include_str!(
                "../../../../tools/rhino_oracle/fixtures/intersection_near_tangent_vertical_snaps.json"
            ),
            include_str!(
                "../../../../tools/rhino_oracle/observations/intersection_near_tangent_vertical_snaps.json"
            ),
        ),
    ] {
        let differences = retained_differences_with_epsilon(fixture, observed, 5e-8);
        assert!(differences.is_empty(), "{differences:?}");
    }
}

#[test]
fn square_aperture_replays_all_128_admissions_but_preserves_five_mesh_selection_differences() {
    let differences = retained_differences(
        include_str!("../../../../tools/rhino_oracle/fixtures/snap_capture_box.json"),
        include_str!("../../../../tools/rhino_oracle/observations/snap_capture_box.json"),
    );
    let expected: Vec<String> = [
        "box-top-mesh-near--10-10",
        "box-perspective-mesh-near--8--8",
        "box-perspective-mesh-near--10--10",
        "box-perspective-mesh-near--10-10",
        "box-perspective-mesh-near-10--10",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(differences, expected);
}

#[test]
fn all_48_separate_sources_match_and_combined_mesh_selection_differences_remain() {
    let differences = retained_differences(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_snap_sources.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_snap_sources.json"),
    );
    assert_eq!(
        differences,
        [
            "sources-top-combined-0-10",
            "sources-top-combined-1-8",
            "sources-perspective-combined-0--4",
            "sources-perspective-combined-0-0",
            "sources-perspective-combined-1-4",
            "sources-perspective-combined-1-8",
            "sources-perspective-combined-1-10",
        ]
    );
}

#[test]
fn short_wire_and_edge_on_replay_fix_endpoints_without_hiding_selection_mismatches() {
    let endpoints = retained_differences(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_snap_endpoints.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_snap_endpoints.json"),
    );
    assert_eq!(endpoints.len(), 21);
    assert!(endpoints.iter().all(|id| !id.contains("line")));
    let triangles = retained_differences(
        include_str!("../../../../tools/rhino_oracle/fixtures/mesh_snap_edge_on.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_snap_edge_on.json"),
    );
    assert_eq!(triangles.len(), 14);
    assert!(triangles.iter().all(|id| id.ends_with("r16")));
}
