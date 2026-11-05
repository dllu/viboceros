#[path = "support/observation.rs"]
mod observation;
#[path = "support/revolve_subd.rs"]
mod subd_profile_comparison;
#[path = "support/extrude_curve_subd.rs"]
mod topology;

#[test]
fn subd_straight_extrusions_match_native_control_topology_limit_meshes_and_history() {
    family("extrude_curve_subd_pilot", 6);
    family("extrude_curve_subd_holdouts", 44);
    family("extrude_curve_subd_confirmations", 12);
}
fn family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut query: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    query["operations"]
        .as_array_mut()
        .unwrap()
        .retain(|op| op["op"] == "extrude_curve_subd");
    let request = serde_json::from_value(query).unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    assert_eq!(actual["results"].as_array().unwrap().len(), count);
    let native = observation::read(&base, family);
    for row in actual["results"].as_array().unwrap() {
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
fn extrusion_surface_fits_match_independent_public_sdk() {
    use viboceros_geometry::{
        NurbsSurface, Point3, SubD, SubDFromSurfaceMethod, Tolerance, WeightedPoint3,
    };
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let native = observation::read(&base, "extrude_curve_subd_sdk");
    let rows = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), 10);
    for row in rows {
        let d = &row["value"]["original"]["surface"];
        let degree: [usize; 2] = serde_json::from_value(d["degree"].clone()).unwrap();
        let count: [usize; 2] = serde_json::from_value(d["control_count"].clone()).unwrap();
        let controls: Vec<viboceros_oracle::ControlPoint> =
            serde_json::from_value(d["control_points"].clone()).unwrap();
        let surface = NurbsSurface::try_new_rational(
            degree[0],
            degree[1],
            count[0],
            count[1],
            controls
                .into_iter()
                .map(|c| {
                    WeightedPoint3::try_new(Point3::try_from(c.point).unwrap(), c.weight).unwrap()
                })
                .collect(),
            serde_json::from_value(d["knots_u"].clone()).unwrap(),
            serde_json::from_value(d["knots_v"].clone()).unwrap(),
        )
        .unwrap();
        let subd = SubD::try_from_surface(
            &surface,
            SubDFromSurfaceMethod::Location,
            true,
            Tolerance::DEFAULT,
        )
        .unwrap();
        topology::compare(
            &viboceros_oracle::extrude_curve_subd::subd_record(&subd).unwrap(),
            &row["value"]["original"]["subd"],
            &row["id"].to_string(),
            1e-12,
        );
    }
}
