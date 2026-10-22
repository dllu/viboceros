//! Native conversion evidence shared by the ToSubD command and getter.
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod native;
use serde_json::Value;
use viboceros_geometry::{SubD, SubDFromMeshOptions, Tolerance};

#[test]
fn mesh_conversion_matches_native_controls_tags_limit_stations_and_facets() {
    let observations: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/tosubd.json"
    ))
    .unwrap();
    let mut comparisons = 0;
    for row in observations["results"].as_array().unwrap() {
        let value = &row["value"];
        if value["success"] != true {
            continue;
        }
        for (source, input) in value["before"].as_array().unwrap().iter().enumerate() {
            let Some(mesh) = input.get("mesh") else {
                continue;
            };
            let mesh = mesh_native::mesh(mesh);
            let original = mesh.clone();
            let options = &value["input"]["options"];
            let converted = SubD::try_from_mesh(
                &mesh,
                SubDFromMeshOptions {
                    interpolate: options["UseMesh"] == "Location",
                    corners: options["MeshCorners"] == true,
                    creases: options["MeshCreases"] == true,
                },
                Tolerance::DEFAULT,
            )
            .unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
            assert_eq!(mesh, original);
            let output = value["after_script"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o.get("subd").is_some() && o["name"] == input["name"])
                .unwrap();
            let mut control = output["subd"].clone();
            control.as_object_mut().unwrap().remove("surface_mesh");
            let mut check = native::compare(&converted, &control);
            native::facets::certify_common_triangulation(&converted, &output["subd"], &mut check);
            println!(
                "TOSUBD_MESH_KERNEL {}",
                serde_json::json!({"id":row["id"],"source":source,"source_unchanged":true,"check":check})
            );
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 16);
}

fn surface(v: &Value) -> viboceros_geometry::NurbsSurface {
    use viboceros_geometry::{NurbsSurface, WeightedPoint3};
    let coordinates = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            WeightedPoint3::try_new(
                mesh_native::point(&c["point"]),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let knots = |axis: &str| {
        v[axis]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect()
    };
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        coordinates,
        knots("knots_u"),
        knots("knots_v"),
    )
    .unwrap()
}
#[test]
fn surface_control_nets_match_native_open_closed_rational_and_singular_cases() {
    check_surfaces(viboceros_geometry::SubDFromSurfaceMethod::ControlPoints, 45);
}
#[test]
fn surface_location_fit_matches_native_greville_cages_closed_seams_and_poles() {
    check_surfaces(viboceros_geometry::SubDFromSurfaceMethod::Location, 63);
}
fn check_surfaces(method: viboceros_geometry::SubDFromSurfaceMethod, expected_count: usize) {
    use viboceros_geometry::SubDFromSurfaceMethod;
    let families = [
        include_str!("../../../tools/rhino_oracle/observations/tosubd.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_surface_details.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_surface_edges.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_features.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_poles.json"),
    ];
    let mode = if method == SubDFromSurfaceMethod::Location {
        "Location"
    } else {
        "ControlPoints"
    };
    let prefix = if method == SubDFromSurfaceMethod::Location {
        "TOSUBD_SURFACE_LOCATION_KERNEL"
    } else {
        "TOSUBD_SURFACE_CONTROL_KERNEL"
    };
    let mut count = 0;
    let mut failures = Vec::new();
    for data in families {
        let observations: Value = serde_json::from_str(data).unwrap();
        for row in observations["results"].as_array().unwrap() {
            let value = &row["value"];
            if value["success"] != true || value["input"]["options"]["UseSurface"] != mode {
                continue;
            }
            for (i, source) in value["input"]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if ![
                    "surface",
                    "sphere_surface",
                    "cylinder_surface",
                    "torus_surface",
                    "cone_surface",
                    "hemisphere_surface",
                    "trimmed",
                ]
                .iter()
                .any(|&k| source["kind"] == k)
                {
                    continue;
                }
                if value["before"][i]["brep"]
                    .as_array()
                    .is_some_and(|faces| faces.len() != 1)
                {
                    continue;
                }
                count += 1;
                let checked = std::panic::catch_unwind(|| {
                    let input = &value["before"][i];
                    let definition = input.get("surface").unwrap_or(&input["brep"][0]["surface"]);
                    let input = surface(definition);
                    let before = input.clone();
                    let converted = SubD::try_from_surface(
                        &input,
                        method,
                        value["input"]["options"]["SurfaceCorners"] == true,
                        Tolerance::DEFAULT,
                    );
                    assert_eq!(input, before);
                    if !value["after_script"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|o| o.get("subd").is_some())
                    {
                        assert!(converted.is_err(), "native rejected the control net");
                        println!(
                            "{prefix} {}",
                            serde_json::json!({"id":row["id"],"source":i,"source_unchanged":true,"rejected":true})
                        );
                        return;
                    }
                    let converted = converted.unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
                    let expected = &value["after_script"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|o| {
                            o.get("subd").is_some() && o["name"] == value["before"][i]["name"]
                        })
                        .unwrap()["subd"];
                    let mut controls = expected.clone();
                    controls.as_object_mut().unwrap().remove("surface_mesh");
                    let mut check = native::compare(&converted, &controls);
                    native::facets::certify_common_triangulation(&converted, expected, &mut check);
                    println!(
                        "{prefix} {}",
                        serde_json::json!({"id":row["id"],"source":i,"source_unchanged":true,"check":check})
                    );
                });
                if checked.is_err() {
                    failures.push(row["id"].clone());
                }
            }
        }
    }
    assert_eq!(count, expected_count);
    assert!(failures.is_empty(), "unmatched {mode} cases: {failures:?}");
}

#[test]
fn collapsed_surface_directions_fail_without_panicking_or_changing_the_source() {
    use viboceros_geometry::{NurbsSurface, Point3, SubDFromSurfaceMethod};
    let p = Point3::try_new(0., 0., 0.).unwrap();
    let q = Point3::try_new(0., 1., 0.).unwrap();
    for points in [vec![p, p, p, p], vec![p, p, q, q]] {
        let source = NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            points,
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let before = source.clone();
        assert!(
            SubD::try_from_surface(
                &source,
                SubDFromSurfaceMethod::Location,
                false,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert_eq!(source, before);
    }
}
