//! Compare complete public SDK outputs for equivalent unit-weight storage forms.
use serde_json::Value;
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod nurbs_surface;
#[path = "../../viboceros-geometry/tests/support/profile_rows.rs"]
mod profile_rows;

#[test]
fn rational_storage_does_not_explain_the_fine_refit_differences() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/observations");
    let read = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(root.join(name)).unwrap()).unwrap()
    };
    let original = read("sweep1_refit_stage_reentry.json");
    let polynomial = read("sweep1_refit_polynomial.json");
    let rows = polynomial["results"].as_array().unwrap();
    assert_eq!(rows.len(), 12);
    assert_eq!(original["results"].as_array().unwrap().len(), 12);
    for row in rows {
        let id = row["id"]
            .as_str()
            .unwrap()
            .strip_suffix("-polynomial")
            .unwrap();
        let before = original["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        assert_eq!(row["value"].as_array().unwrap().len(), 1);
        assert_eq!(before["value"].as_array().unwrap().len(), 1);
        let a = nurbs_surface::from_definition(&row["value"][0]["definition"]);
        let b = nurbs_surface::from_definition(&before["value"][0]["definition"]);
        let bound = profile_rows::complete_bound(&a, &b, 1e-7)
            .expect("complete surface images must agree across equivalent storage flags");
        println!(
            "SWEEP_POLYNOMIAL_REFIT_RESULT {}",
            serde_json::json!({
                "id":id,"complete_image_bound":bound,"epsilon":1e-7,
            })
        );
    }
}
