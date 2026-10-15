use super::*;
#[test]
fn loose_recipes_keep_original_bases_and_full_misses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/projection_loose.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 9);
    for result in response.results {
        let curves = result.value["curves"].as_array().unwrap();
        if result.id.ends_with("miss") {
            assert!(curves.is_empty());
        } else {
            assert_eq!(curves.len(), 1, "{}", result.id);
            assert!(curves[0]["definition"]["knots"].is_array());
        }
    }
}
