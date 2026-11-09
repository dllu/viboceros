//! Fit's captured bases qualify tensor controls, not adaptive basis selection.
use serde_json::{Value, json};
use viboceros_geometry::*;
#[path = "../../viboceros-command/tests/support/merge_surfaces.rs"]
#[allow(dead_code)]
mod captured;

fn interpolate(source: &NurbsSurface, degree: [usize; 2], knots: [&[f64]; 2]) -> NurbsSurface {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::interpolate_surface_in_basis(source, degree, knots).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_interpolate_surface_in_basis(source, degree, knots).unwrap()
    }
}

fn fit(
    source: &NurbsSurface,
    degree: [usize; 2],
    knots: [&[f64]; 2],
    limit: f64,
) -> Option<BoundedSurfaceFit> {
    #[cfg(feature = "native-smlib")]
    {
        viboceros_smlib::fit_surface_in_basis(source, degree, knots, limit).unwrap()
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        try_fit_surface_in_basis(source, degree, knots, limit).unwrap()
    }
}

#[test]
fn cartesian_tensor_controls_replay_every_successful_native_fit_in_its_captured_basis() {
    let query: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/rhino_oracle/fixtures/surface_fit.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let native = captured::read("surface_fit");
    let mut matched = 0;
    let mut native_null = 0;
    assert_eq!(query["operations"].as_array().unwrap().len(), 36);
    assert_eq!(native["results"].as_array().unwrap().len(), 36);
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let value = &row["value"];
        assert_eq!(value["before"], value["after"]);
        let source = captured::surface(&value["before"]);
        // These are native fitting failures. They are retained, not replaced
        // with a fabricated output or treated as supported algorithm matches.
        if value["surface"].is_null() {
            native_null += 1;
            println!(
                "SURFACE_FIT_BASIS {}",
                json!({"id":op["id"],"native_null":true})
            );
            continue;
        }
        let expected = captured::surface(&value["surface"]);
        let degree = [
            op["degree_u"].as_u64().unwrap() as usize,
            op["degree_v"].as_u64().unwrap() as usize,
        ];
        assert_eq!(degree, [expected.degree_u(), expected.degree_v()]);
        let before = source.clone();
        let actual = interpolate(&source, degree, [expected.knots_u(), expected.knots_v()]);
        assert_eq!(source, before);
        assert_eq!(
            (
                actual.control_point_count_u(),
                actual.control_point_count_v()
            ),
            (
                expected.control_point_count_u(),
                expected.control_point_count_v()
            )
        );
        assert!(actual.control_points().iter().all(|c| c.weight() == 1.));
        let bound = actual
            .parameter_surface_deviation_bound(&expected, 1e-9)
            .unwrap()
            .unwrap_or_else(|| panic!("{}: complete native fit basis comparison", op["id"]));
        let requested = op["fit_tolerance"].as_f64().unwrap();
        let source_bound = fit(
            &source,
            degree,
            [expected.knots_u(), expected.knots_v()],
            requested,
        )
        .map(|bounded| {
            assert_eq!(bounded.surface, actual);
            bounded.maximum_deviation_bound
        });
        let brep = Brep::try_rectangular_surface_face_with_surface_edge_domains(
            actual.clone(),
            actual.domain_u(),
            actual.domain_v(),
            false,
            captured::model_tolerance(),
        )
        .unwrap();
        let archive = captured::archive(&brep);
        let archive_bound = actual
            .parameter_surface_deviation_bound(archive.faces()[0].surface(), 1e-9)
            .unwrap()
            .unwrap();
        println!(
            "SURFACE_FIT_BASIS {}",
            json!({"id":op["id"],"native_null":false,"maximum_native_bound":bound,"maximum_archive_bound":archive_bound,"requested_fit_tolerance":requested,"source_parameter_bound":source_bound})
        );
        matched += 1;
    }
    assert_eq!((matched, native_null), (23, 13));
    println!(
        "SURFACE_FIT_BASIS_TOTAL {}",
        json!({"matched_bases":matched,"native_null":native_null,"adaptive_fit_qualified":false})
    );
}
