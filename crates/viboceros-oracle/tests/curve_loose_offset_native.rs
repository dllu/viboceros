//! Public loose Offset controls differ from Inset's rational edit-point policy.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/interpolation_plane.rs"]
#[allow(dead_code)]
mod captured;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod surfaces;

#[test]
fn polynomial_loose_offsets_reproduce_complete_native_curves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut outputs = 0;
    for family in ["curve_loose_offset", "curve_loose_offset_types"] {
        let q: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("../../tools/rhino_oracle/fixtures/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        let native = captured::read(family);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let value = &row["value"];
            assert_eq!(value["success"], true);
            assert!(
                !value["history"]
                    .as_str()
                    .unwrap()
                    .contains("Unknown command")
            );
            let source = captured::curve(&value["before"][0]["curve"]);
            let original = source.clone();
            let targets = value["created"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["source"] == false)
                .collect::<Vec<_>>();
            let curve = Curve3::NurbsCurve(source.clone());
            let side =
                Point3::try_from(serde_json::from_value::<[f64; 3]>(op["side"].clone()).unwrap())
                    .unwrap();
            let normal = Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap();
            let tolerance = Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap();
            let sign = if op["both_sides"] == true {
                1.
            } else {
                curve.offset_side(side, normal, tolerance).unwrap()
            };
            let distances = [
                sign * op["distance"].as_f64().unwrap(),
                -op["distance"].as_f64().unwrap(),
            ];
            assert_eq!(targets.len(), if op["both_sides"] == true { 2 } else { 1 });
            let mut used = vec![false; targets.len()];
            for &distance in &distances[..targets.len()] {
                #[cfg(feature = "native-smlib")]
                let solver =
                    Some(viboceros_smlib::curve_edit_point_interpolation as CurveEditPointSolver);
                #[cfg(not(feature = "native-smlib"))]
                let solver = None;
                let actual = source
                    .try_loose_polynomial_offset(distance, normal, tolerance, solver)
                    .unwrap();
                assert!(actual.control_points().iter().all(|p| p.weight() == 1.));
                let matched = targets
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !used[*i])
                    .find_map(|(i, target)| {
                        let expected = captured::curve(&target["curve"]);
                        actual
                            .parameter_curve_deviation_bound(&expected, 1e-9)
                            .unwrap()
                            .map(|bound| (i, expected, bound))
                    });
                let (index, expected, bound) =
                    matched.unwrap_or_else(|| panic!("{}: {distance}", op["id"]));
                used[index] = true;
                println!(
                    "CURVE_LOOSE_OFFSET {}",
                    json!({"id":op["id"],"distance":distance,"native_bound":bound,"native_index":index,
                "controls":actual.control_points().len(),"degree":actual.degree(),"native_controls":expected.control_points().len()})
                );
                outputs += 1;
            }
            assert_eq!(source, original);
        }
    }
    assert_eq!(outputs, 29);
}

#[test]
fn polynomial_offset_matches_the_complete_outer_isocurve_of_a_native_loose_ribbon() {
    let data = captured::read("ribbon_options");
    let row = data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "smooth-loose")
        .unwrap();
    let value = &row["value"];
    assert_eq!(value["success"], true);
    assert!(
        !value["history"]
            .as_str()
            .unwrap()
            .contains("Unknown command")
    );
    let source = captured::curve(&value["before"][0]["curve"]);
    let output = value["created"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["source"] == false)
        .unwrap();
    let s = surfaces::surface(&output["surfaces"][0]);
    let boundary = s.isocurve_v(*s.domain_u().end()).unwrap();
    #[cfg(feature = "native-smlib")]
    let solver = Some(viboceros_smlib::curve_edit_point_interpolation as CurveEditPointSolver);
    #[cfg(not(feature = "native-smlib"))]
    let solver = None;
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let actual = source
        .try_loose_polynomial_offset(0.5, normal, Tolerance::DEFAULT, solver)
        .unwrap();
    let bound = actual
        .parameter_curve_deviation_bound(&boundary, 1e-9)
        .unwrap()
        .unwrap();
    println!(
        "RIBBON_LOOSE_BOUNDARY {}",
        json!({"id":"smooth-loose","native_bound":bound,"complete_ribbon_command":false})
    );
}

#[test]
fn json_offset_keeps_the_source_and_returns_reusable_curve_definitions_without_a_distance_certificate()
 {
    use viboceros_oracle::{ProbeRequest, run_request};
    let source = json!({"degree":1,"knots":[0.,0.,1.,1.],"control_points":[
        {"point":[0.,0.,0.],"weight":1.},{"point":[2.,0.,0.],"weight":1.}]});
    let call = |curve: &Value, distance: f64, normal: Option<[f64; 3]>| {
        let mut op = json!({"op":"curve_loose_offset_geometry","id":"loose","curve":curve,"distance":distance});
        if let Some(normal) = normal {
            op["normal"] = json!(normal);
        }
        let request = serde_json::from_value::<ProbeRequest>(
            json!({"protocol_version":1,"iterations":1,"operations":[op]}),
        )
        .unwrap();
        run_request(&request).map(|mut r| r.results.remove(0).value)
    };
    let first = call(&source, -0.5, None).unwrap();
    assert_eq!(first["distance_certified"], false);
    assert_eq!(first["before"], first["after"]);
    assert_eq!(
        first["curve"]["control_points"][0]["point"],
        json!([0., -0.5, 0.])
    );
    let second = call(&first["curve"], 0.5, Some([0., 0., 1.])).unwrap();
    assert_eq!(
        captured::curve(&second["curve"]),
        captured::curve(&first["before"])
    );
    assert!(call(&source, 0., None).is_err());
    assert!(call(&source, 0.5, Some([0., 0., 0.])).is_err());
}
