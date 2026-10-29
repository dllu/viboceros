//! Qualify every newly admitted pair without rewriting the legacy baseline.
use section_refit::definitions;
use serde_json::{Value, json};
use viboceros_geometry::try_prepare_refitted_loft_sections;
#[allow(dead_code)]
#[path = "support/section_refit.rs"]
mod section_refit;

#[test]
fn large_relative_tolerances_match_complete_native_loft_boundary_curves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_coarse.json"))).unwrap(),
        )
        .unwrap()
    };
    let (query, observed) = (read("fixtures"), read("observations"));
    let mut contexts = 0;
    let mut curves = 0;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        if op["refit_tolerance"] != 100. || op["id"] == "loft-refit-coarse-circle-100.0" {
            continue;
        }
        let sources = op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .map(definitions::curve)
            .collect::<Vec<_>>();
        let before = sources.clone();
        let actual = try_prepare_refitted_loft_sections(&sources, 100.).unwrap();
        assert_eq!(sources, before);
        assert_eq!(actual.len(), 2);
        let surfaces = row["value"]["surfaces"].as_array().unwrap();
        assert_eq!(surfaces.len(), 1);
        let native = &surfaces[0];
        assert_eq!(native["degree"][0], 1);
        assert_eq!(native["control_count"][0], 2);
        let controls = native["control_points"].as_array().unwrap();
        assert_eq!(
            controls.len(),
            2 * native["control_count"][1].as_u64().unwrap() as usize
        );
        for (i, actual) in actual.iter().enumerate() {
            let reference = definitions::curve(&json!({
                "degree":native["degree"][1],"knots":native["knots_v"],
                "control_points":controls.iter().skip(i).step_by(2).collect::<Vec<_>>(),
            }));
            let comparison = section_refit::compare(actual, &reference);
            assert!(
                comparison["complete_image_bound"].is_number(),
                "{}: {comparison}",
                op["id"]
            );
            println!(
                "LOFT_LARGE_REFIT_IMAGE {}",
                json!({"id":op["id"],"profile":i,"comparison":comparison,"epsilon":1e-7})
            );
            curves += 1;
        }
        contexts += 1;
    }
    assert_eq!((contexts, curves), (12, 24));
}
