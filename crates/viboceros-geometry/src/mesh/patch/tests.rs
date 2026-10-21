use super::*;
use serde_json::Value;
fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn holes_winding_scales_and_remote_origins_preserve_the_trimmed_region() {
    for (scale, offset) in [(1e-9, 0.), (1e-6, 0.), (1., 1e9), (1e6, -1e9)] {
        let map = |x: Real, y: Real| p(x.mul_add(scale, offset), y.mul_add(scale, offset), offset);
        let outer = Polyline3::try_new(
            vec![
                map(0., 0.),
                map(6., 0.),
                map(6., 5.),
                map(0., 5.),
                map(0., 0.),
            ],
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let hole = Polyline3::try_new(
            vec![
                map(2., 1.),
                map(4., 1.),
                map(4., 3.),
                map(2., 3.),
                map(2., 1.),
            ],
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        for reversed in [false, true] {
            let mut vertices = hole.vertices().to_vec();
            if reversed {
                vertices.reverse();
            }
            let hole = Polyline3::try_new(vertices, Tolerance::NUMERICAL_VALIDATION).unwrap();
            let mesh = TriangleMesh::try_patch(
                &[],
                std::slice::from_ref(&outer),
                &[hole],
                MeshPatchOptions::default(),
                Tolerance::try_new(1e-6 * scale, 1e-12, 1e-9).unwrap(),
            )
            .unwrap();
            assert!(
                (mesh.area().unwrap() / scale / scale - 26.).abs() < 1e-8,
                "{scale} {offset}"
            );
            let index = MeshProjectionIndex::new(&mesh).unwrap();
            let center = map(3., 2.);
            assert!(
                (index
                    .closest_point(center)
                    .unwrap()
                    .unwrap()
                    .distance_to(center)
                    .unwrap()
                    / scale
                    - 1.)
                    .abs()
                    < 1e-8
            );
        }
    }
}
#[test]
fn conflicting_sheets_open_boundaries_and_excessive_inputs_are_rejected() {
    let points = [
        p(-10., -10., 0.),
        p(10., -10., 0.),
        p(10., 10., 0.),
        p(-10., 10., 0.),
        p(0., 0., -1.),
        p(0., 0., 1.),
    ];
    assert!(matches!(
        TriangleMesh::try_patch(
            &points,
            &[],
            &[],
            MeshPatchOptions::default(),
            Tolerance::DEFAULT
        ),
        Err(GeometryError::MeshPatchProjectionConflict)
    ));
    let guides = [
        Polyline3::try_new(vec![p(1., 1., 0.), p(4., 3., 0.)], Tolerance::DEFAULT).unwrap(),
        Polyline3::try_new(vec![p(1., 3., 0.), p(4., 1., 0.)], Tolerance::DEFAULT).unwrap(),
    ];
    assert!(matches!(
        TriangleMesh::try_patch(
            &vec![p(0., 0., 0.); MAX_MESH_PATCH_POINTS + 1],
            &[],
            &[],
            MeshPatchOptions::default(),
            Tolerance::DEFAULT
        ),
        Err(GeometryError::MeshPatchLimit)
    ));
    assert!(
        TriangleMesh::try_patch(
            &points[..4],
            &[],
            &guides,
            MeshPatchOptions::default(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
}
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(value.clone()).unwrap()).unwrap()
}
fn lines(value: &Value) -> Vec<Polyline3> {
    value
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .map(|line| {
                    Polyline3::try_new(
                        line.as_array().unwrap().iter().map(point).collect(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                    .unwrap()
                })
                .collect()
        })
        .unwrap_or_default()
}
fn mesh(value: &Value) -> TriangleMesh {
    try_mesh(value).unwrap()
}
fn try_mesh(value: &Value) -> Result<TriangleMesh, GeometryError> {
    TriangleMesh::try_new(
        value["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect(),
        serde_json::from_value(value["faces"].clone()).unwrap(),
        Tolerance::MESH_VALIDATION,
    )
}
fn sampled_error(a: &TriangleMesh, b: &TriangleMesh) -> Real {
    let index = MeshProjectionIndex::new(b).unwrap();
    let mut maximum: Real = 0.;
    for indices in a.triangles() {
        let [a, b, c] = indices.map(|i| a.vertices()[i as usize].to_array());
        for i in 0..=8 {
            for j in 0..=8 - i {
                let u = i as Real / 8.;
                let v = j as Real / 8.;
                let point = Point3::try_from(std::array::from_fn::<_, 3, _>(|k| {
                    (b[k] - a[k]).mul_add(u, (c[k] - a[k]).mul_add(v, a[k]))
                }))
                .unwrap();
                maximum = maximum.max(
                    point
                        .distance_to(index.closest_point(point).unwrap().unwrap())
                        .unwrap(),
                );
            }
        }
    }
    maximum
}
#[test]
fn captured_mesh_patch_loci_are_compared_without_loosening_the_model_gate() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    for row in response["results"].as_array().unwrap() {
        let value = &row["value"];
        let input = &value["input"];
        let points = input["points"]
            .as_array()
            .map(|v| v.iter().map(point).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut guides = lines(&input["curves"]);
        if let Some(c) = input["circle"].as_array() {
            let circle = crate::Circle3::try_new(
                Point3::try_new(
                    c[0].as_f64().unwrap(),
                    c[1].as_f64().unwrap(),
                    c[2].as_f64().unwrap(),
                )
                .unwrap(),
                c[3].as_f64().unwrap(),
                crate::Vector3::try_new(0., 0., 1.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            guides.push(
                crate::CurveRef::Circle(&circle)
                    .try_polyline_by_angle(std::f64::consts::PI / 12., MAX_MESH_PATCH_POINTS)
                    .unwrap(),
            );
        }
        let holes = lines(&input["holes"]);
        let surface = input["surface"].as_array().map(|corners| {
            let corners: Vec<_> = corners.iter().map(point).collect();
            let native = &value["surface"];
            NurbsSurface::try_bilinear(corners.try_into().unwrap())
                .unwrap()
                .try_reparameterized(
                    native["domain_u"][0].as_f64().unwrap()
                        ..=native["domain_u"][1].as_f64().unwrap(),
                    native["domain_v"][0].as_f64().unwrap()
                        ..=native["domain_v"][1].as_f64().unwrap(),
                )
                .unwrap()
        });
        let options = MeshPatchOptions {
            trim_back: input["trimback"].as_bool().unwrap_or(true),
            divisions: input["divisions"].as_u64().unwrap_or(1) as usize,
            starting_surface: surface.as_ref(),
            ..Default::default()
        };
        let actual = TriangleMesh::try_patch(&points, &guides, &holes, options, Tolerance::DEFAULT);
        if value["sdk"].is_null() {
            assert!(matches!(actual, Err(GeometryError::EmptyMeshPatch)));
            println!(
                "MESH_PATCH_GEOMETRY {}",
                serde_json::json!({"id":row["id"],"native_failure":true,"qualified":true})
            );
            continue;
        }
        let actual = actual.unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
        let expected = mesh(&value["sdk"]);
        let error = sampled_error(&actual, &expected).max(sampled_error(&expected, &actual));
        println!(
            "MESH_PATCH_GEOMETRY {}",
            serde_json::json!({"id":row["id"],"maximum_bidirectional_sample_error":error,"qualified":error<1e-6})
        );
        if error >= 1e-6 {
            failures.push(row["id"].clone());
        }
    }
    assert!(
        failures.is_empty(),
        "unqualified native mesh patches: {failures:?}"
    );
}

#[test]
fn captured_crossing_guides_compare_the_command_and_sdk_separately() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_CROSSING_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
#[test]
fn captured_untrimmed_crossing_guides_keep_source_vertices_and_input_priority() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_untrimmed.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_UNTRIMMED_GEOMETRY");
    assert!(
        failures.is_empty(),
        "Unqualified untrimmed crossings: {failures:?}"
    );
}
#[test]
fn captured_seeded_crossings_retain_point_regions() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_seeded.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_SEEDED_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
#[test]
fn captured_junctions_and_overlaps_retain_all_comparison_results() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_junctions.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_JUNCTION_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
#[test]
fn captured_sdk_order_audit_preserves_command_comparisons() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_audit.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_AUDIT_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
#[test]
fn captured_point_guide_variations_keep_the_fixed_model_gate() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_points.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_POINTS_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
#[test]
fn repeated_native_crossings_and_direction_variants_remain_visible() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/mesh_patch_crossings_verify.json"
    ))
    .unwrap();
    let failures = crossing_replay(&response, "MESH_PATCH_VERIFY_GEOMETRY");
    assert_known_crossing_gaps(&failures);
}
fn assert_known_crossing_gaps(failures: &[Value]) {
    let known: Vec<String> = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/mesh_patch_crossing_gaps.json"
    ))
    .unwrap();
    let unexpected = failures
        .iter()
        .filter(|id| {
            let id = id.as_str().unwrap();
            !known
                .iter()
                .any(|gap| gap == id.strip_suffix("_sdk_after").unwrap_or(id))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "new MeshPatch compatibility gaps: {unexpected:?}"
    );
}
#[test]
fn explicit_outer_boundaries_remain_hard_constraints() {
    let outer = Polyline3::try_new(
        vec![
            p(-1., -1., 0.),
            p(1., -1., 0.),
            p(1., 1., 0.),
            p(-1., 1., 0.),
            p(-1., -1., 0.),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let guide =
        Polyline3::try_new(vec![p(-2., 0., 0.), p(2., 0., 0.)], Tolerance::DEFAULT).unwrap();
    assert!(matches!(
        TriangleMesh::try_patch(
            &[],
            &[guide],
            &[],
            MeshPatchOptions {
                outer_boundary: Some(&outer),
                ..Default::default()
            },
            Tolerance::DEFAULT
        ),
        Err(GeometryError::MeshPatchCrossingConstraints)
    ));
}
#[test]
fn repeated_long_constraints_exhaust_work_before_admission() {
    let points = (0..60)
        .flat_map(|x| (0..60).map(move |y| p(x as Real, y as Real, 0.)))
        .collect::<Vec<_>>();
    let diagonal =
        Polyline3::try_new(vec![p(0., 0., 0.), p(59., 59., 0.)], Tolerance::DEFAULT).unwrap();
    let guides = vec![diagonal; 35_000];
    assert!(points.len() + 2 * guides.len() < MAX_MESH_PATCH_POINTS);
    for trim_back in [true, false] {
        assert!(matches!(
            TriangleMesh::try_patch(
                &points,
                &guides,
                &[],
                MeshPatchOptions {
                    trim_back,
                    ..Default::default()
                },
                Tolerance::DEFAULT
            ),
            Err(GeometryError::MeshPatchLimit)
        ));
    }
}
fn crossing_replay(response: &Value, prefix: &str) -> Vec<Value> {
    let mut failures = Vec::new();
    for row in response["results"].as_array().unwrap() {
        let value = &row["value"];
        let input = &value["input"];
        let points = input["points"]
            .as_array()
            .map(|v| v.iter().map(point).collect::<Vec<_>>())
            .unwrap_or_default();
        let guides = lines(&input["curves"]);
        let holes = lines(&input["holes"]);
        let native = &value["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["role"] == "output")
            .unwrap()["mesh"];
        let expected = try_mesh(native);
        let sdk = try_mesh(&value["sdk"]);
        let sdk_error = sdk.as_ref().err().map(ToString::to_string);
        let options = MeshPatchOptions {
            trim_back: input["trimback"].as_bool().unwrap_or(true),
            divisions: input["divisions"].as_u64().unwrap_or(1) as usize,
            ..Default::default()
        };
        let actual = TriangleMesh::try_patch(&points, &guides, &holes, options, Tolerance::DEFAULT);
        let expected = match expected {
            Ok(mesh) => mesh,
            Err(error) => {
                // Native zero-area facets are retained as an explicit gap.
                // These fixed scenes must still produce a valid local mesh.
                let actual = actual.unwrap();
                println!(
                    "{prefix} {}",
                    serde_json::json!({"id":row["id"],"qualified":false,"maximum_bidirectional_sample_error":null,"sdk_command_difference":null,"native_mesh_error":error.to_string(),"sdk_mesh_error":sdk_error,"actual_faces":actual.faces().len(),"actual_vertices":actual.vertices().len(),"native_faces":native["faces"].as_array().unwrap().len(),"native_vertices":native["vertices"].as_array().unwrap().len()})
                );
                failures.push(row["id"].clone());
                continue;
            }
        };
        let native_gap = sdk
            .as_ref()
            .ok()
            .map(|sdk| sampled_error(&expected, sdk).max(sampled_error(sdk, &expected)));
        match actual {
            Err(e) => {
                println!(
                    "{prefix} {}",
                    serde_json::json!({"id":row["id"],"qualified":false,"error":e.to_string(),"sdk_command_difference":native_gap,"sdk_mesh_error":sdk_error})
                );
                failures.push(row["id"].clone());
            }
            Ok(actual) => {
                let error =
                    sampled_error(&actual, &expected).max(sampled_error(&expected, &actual));
                let qualified = error < 1e-6
                    && actual.faces().len() == expected.faces().len()
                    && actual.vertices().len() == expected.vertices().len();
                if !qualified && std::env::var_os("VIBOCEROS_MESH_PATCH_DEBUG").is_some() {
                    println!(
                        "MESH_PATCH_DETAIL {}",
                        serde_json::json!({"id":row["id"],"vertices":actual.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":actual.triangles()})
                    );
                }
                println!(
                    "{prefix} {}",
                    serde_json::json!({"id":row["id"],"qualified":qualified,"maximum_bidirectional_sample_error":error,"sdk_command_difference":native_gap,"sdk_mesh_error":sdk_error,"actual_faces":actual.faces().len(),"native_faces":expected.faces().len(),"actual_vertices":actual.vertices().len(),"native_vertices":expected.vertices().len()})
                );
                if !qualified {
                    failures.push(row["id"].clone());
                }
            }
        }
    }
    failures
}
