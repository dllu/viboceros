use serde_json::json;
use viboceros_geometry::{CurvatureColorMapping, CurvatureStyle, Tolerance};
use viboceros_oracle::{ProbeRequest, curvature_analysis::Fixture, run_request};
#[path = "support/direction_analysis.rs"]
#[allow(dead_code)]
mod compare;
#[path = "support/curvature_analysis.rs"]
mod support;

#[test]
fn commands_and_history_match_native_analysis_ownership_and_settings() {
    for (op, n) in support::cases() {
        let q:ProbeRequest=serde_json::from_value(json!({"protocol_version":1,"iterations":1,"tolerance":{"absolute":1e-8,"relative":1e-12,"angular":1e-8},"operations":[op]})).unwrap();
        let r = run_request(&q).unwrap();
        compare::compare(
            &r.results[0].value,
            &support::expected(&op, &n),
            op["id"].as_str().unwrap(),
        );
    }
}
#[test]
fn native_curvature_color_samples_agree_within_one_quantization_step() {
    let mut count = 0;
    for (op, n) in support::cases() {
        for t in n["traces"].as_array().unwrap() {
            let s = &t["settings"];
            let style = CurvatureStyle::ALL
                .into_iter()
                .find(|k| k.name() == s["style"].as_str().unwrap())
                .unwrap();
            let index = CurvatureStyle::ALL
                .iter()
                .position(|k| *k == style)
                .unwrap();
            let range: [f64; 2] = serde_json::from_value(s["ranges"][index].clone()).unwrap();
            let mapping = CurvatureColorMapping::try_new(style, range).unwrap();
            for m in t["meshes"].as_array().unwrap() {
                for sample in m["samples"].as_array().unwrap() {
                    let k: [f64; 2] = serde_json::from_value(sample["principal"].clone()).unwrap();
                    let color: [u8; 4] = serde_json::from_value(sample["color"].clone()).unwrap();
                    let actual = mapping.color(k).unwrap();
                    assert!(
                        actual
                            .into_iter()
                            .zip(color)
                            .all(|(a, b)| a.abs_diff(b) <= 1),
                        "{} {k:?} {actual:?} {color:?}",
                        op["id"]
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 11536);
}
#[test]
fn regular_surface_samples_match_native_principal_curvatures() {
    let mut count = 0;
    let mut singular = 0;
    for (op, n) in support::cases() {
        let fixture: Fixture = serde_json::from_value(op.clone()).unwrap();
        let source = fixture.source(Tolerance::DEFAULT).unwrap();
        let viboceros_document::Geometry::Brep(b) = source else {
            continue;
        };
        for t in n["traces"].as_array().unwrap() {
            for (face, m) in b.faces().iter().zip(t["meshes"].as_array().unwrap()) {
                for sample in m["samples"].as_array().unwrap() {
                    let [u, v]: [f64; 2] = serde_json::from_value(sample["uv"].clone()).unwrap();
                    let expected: [f64; 2] =
                        serde_json::from_value(sample["principal"].clone()).unwrap();
                    let Ok(actual) = face.surface().curvature_at(u, v) else {
                        singular += 1;
                        continue;
                    };
                    let mut actual = actual.principal;
                    let mut expected = expected;
                    actual.sort_by(|a, b| a.total_cmp(b));
                    expected.sort_by(|a, b| a.total_cmp(b));
                    for (a, b) in actual.into_iter().zip(expected) {
                        assert!(
                            (a - b).abs() <= 1e-7 * (1. + b.abs()),
                            "{} uv={u},{v} {a} vs {b}",
                            op["id"]
                        );
                    }
                    count += 1;
                }
            }
        }
    }
    assert!(count > 11000);
    assert!(singular < 50);
}
