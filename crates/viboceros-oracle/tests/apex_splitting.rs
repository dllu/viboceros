#[path = "support/apex_splitting/cap_chart.rs"]
mod cap_charts;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/extrude_apex_subd.rs"]
mod subd_apex_comparison;
#[path = "support/extrude_curve_subd.rs"]
mod subd_face_comparison;
#[path = "support/revolve_subd.rs"]
mod subd_profile_comparison;
#[path = "support/apex_splitting.rs"]
mod topology;
#[test]
fn apex_split_policies_match_native_used_definitions_and_history() {
    family("apex_splitting_pilot", 28);
    family("apex_splitting_holdouts", 43);
}
fn family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, family);
    assert_eq!(query["operations"].as_array().unwrap().len(), count);
    for operation in query["operations"].as_array().unwrap() {
        let mut one = query.clone();
        one["operations"] = serde_json::json!([operation]);
        let request = serde_json::from_value(one).unwrap();
        let response = viboceros_oracle::run_request(&request)
            .unwrap_or_else(|e| panic!("{}: {e:?}", operation["id"]));
        let actual = serde_json::to_value(response).unwrap();
        let row = &actual["results"][0];
        let expected = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == row["id"])
            .unwrap();
        for phase in ["before", "after", "undo", "redo"] {
            topology::compare(
                &row["value"][phase],
                &expected["value"][phase],
                &format!("{}/{phase}", row["id"]),
                1e-12,
            );
        }
    }
}

#[test]
fn public_curve_support_frames_match_independent_native_sdk() {
    use viboceros_document::Geometry;
    use viboceros_geometry::{CurveRef, Tolerance};
    use viboceros_oracle::apex_splitting::{Fixture, setup};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("fixtures/apex_splitting_planes.json")).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, "apex_splitting_planes");
    assert_eq!(query["operations"].as_array().unwrap().len(), 3);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        let f:Fixture=serde_json::from_value(serde_json::json!({"curve":op["curve"],"apex":[0.,3.,2.],"solid":true,"delete_input":false,"split_at_tangents":true})).unwrap();
        let (doc, id) = setup(&f, Tolerance::DEFAULT).unwrap();
        let Geometry::NurbsCurve(curve) = doc.object(id).unwrap().geometry() else {
            panic!("curve required")
        };
        let original = curve.clone();
        for (curve, key) in [
            (curve.clone(), "original"),
            (curve.reversed().unwrap(), "reversed"),
        ] {
            let frame = CurveRef::NurbsCurve(&curve)
                .supporting_plane_frame(Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            let actual = serde_json::json!({"origin":frame.origin().to_array(),"x":frame.x_axis().as_vector().to_array(),"y":frame.y_axis().as_vector().to_array(),"z":frame.z_axis().as_vector().to_array()});
            subd_face_comparison::compare(
                &actual,
                &row["value"][key],
                &format!("{}/{key}", op["id"]),
                1e-12,
            );
        }
        assert_eq!(curve, &original);
    }
}
