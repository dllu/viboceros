//! Whole-extrusion and direct public surface-SDK conversion comparisons.
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod coordinates;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod native;
use serde_json::Value;
use viboceros_geometry::{NurbsSurface, SubD, SubDFromSurfaceMethod, Tolerance, WeightedPoint3};

fn observations() -> Vec<Value> {
    [
        include_str!("../../../tools/rhino_oracle/observations/tosubd_extrusions.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_pipes.json"),
        include_str!("../../../tools/rhino_oracle/observations/tosubd_conversion_edges.json"),
    ]
    .into_iter()
    .map(|text| serde_json::from_str(text).unwrap())
    .collect()
}
fn method(value: &Value) -> SubDFromSurfaceMethod {
    if value["input"]["options"]["UseSurface"] == "Location" {
        SubDFromSurfaceMethod::Location
    } else {
        SubDFromSurfaceMethod::ControlPoints
    }
}
fn check(subd: &SubD, expected: &Value) -> Value {
    let mut control = expected.clone();
    control.as_object_mut().unwrap().remove("surface_mesh");
    let mut result = native::compare(subd, &control);
    native::facets::certify_common_triangulation(subd, expected, &mut result);
    result
}
#[test]
fn extrusion_conversion_matches_native_profiles_caps_transposition_tags_and_facets() {
    let families = observations();
    let mut sources = Vec::new();
    for data in &families {
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            if !v["source_files"].as_array().unwrap().is_empty()
                && row["id"] != "tosubd_extrusion_tube_00"
            {
                let id = row["id"].as_str().unwrap();
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tools/rhino_oracle/fixtures/extrusions/{id}.3dm"
                ));
                let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
                assert_eq!(model.unsupported_object_count(), 0, "{id}");
                let viboceros_io::ThreeDmGeometry::Brep(b) = &model.objects[0].geometry else {
                    panic!("extrusion source missing")
                };
                sources.push((v["input"]["sources"][0].clone(), b.clone()));
            }
        }
    }
    let mut failures = Vec::new();
    let mut comparisons = 0;
    for data in &families {
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            if row["id"].as_str().unwrap().contains("tube_")
                || row["id"] == "tosubd_pipe_profile_frame_diagnostic"
                || v["input"]["sources"][0]["kind"] != "extrusion"
            {
                continue;
            }
            comparisons += 1;
            let checked = std::panic::catch_unwind(|| {
                let source = &sources
                    .iter()
                    .find(|(definition, _)| definition == &v["input"]["sources"][0])
                    .unwrap()
                    .1;
                let before = source.clone();
                let converted = SubD::try_from_extrusion(
                    source.extrusion().unwrap(),
                    method(v),
                    v["input"]["options"]["SurfaceCorners"] == true,
                    Tolerance::DEFAULT,
                )
                .unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
                assert_eq!(source, &before);
                let expected = &v["after_script"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|o| o.get("subd").is_some())
                    .unwrap()["subd"];
                let checked = check(&converted, expected);
                println!(
                    "TOSUBD_EXTRUSION_KERNEL {}",
                    serde_json::json!({"id":row["id"],"check":checked,"source_unchanged":true})
                );
            });
            if checked.is_err() {
                failures.push(row["id"].clone());
            }
        }
    }
    assert_eq!(comparisons, 54);
    assert!(
        failures.is_empty(),
        "unmatched extrusion cases: {failures:?}"
    );
}
fn surface(v: &Value) -> NurbsSurface {
    let controls = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                coordinates::point(&p["point"]),
                p["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let knots = |name: &str| {
        v[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    };
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls,
        knots("knots_u"),
        knots("knots_v"),
    )
    .unwrap()
}
#[test]
fn public_surface_sdk_distinguishes_linear_and_quadratic_kink_creases() {
    let data = observations().pop().unwrap();
    let mut count = 0;
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        for sdk in v["sdk_inputs"].as_array().unwrap() {
            let source = surface(&sdk["surface"]);
            let before = source.clone();
            let subd = SubD::try_from_surface(
                &source,
                method(v),
                v["input"]["options"]["SurfaceCorners"] == true,
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert_eq!(source, before);
            let checked = check(&subd, &sdk["subd"]);
            println!(
                "TOSUBD_SURFACE_SDK_KERNEL {}",
                serde_json::json!({"id":row["id"],"check":checked,"source_unchanged":true})
            );
            count += 1;
        }
    }
    assert_eq!(count, 8);
}
