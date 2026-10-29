//! Actual InsertPoint control identities and custom face data, not just shape.
use super::{custom, support};
#[path = "subd_insert_identity.rs"]
mod identity;
use serde_json::{Value, json};
use viboceros_geometry::{
    LineSegment, Point3, PolygonEdgePoint, SubD, SubDMergeComponent, Tolerance, Vector3,
};

fn object(rows: &Value) -> &Value {
    rows.as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap()
}
pub(super) fn compare(
    actual: &SubD,
    expected: &SubD,
    rows: &Value,
    source: &SubD,
    path: &str,
    differences: &mut Vec<String>,
) {
    let expected_value = support::actual_value(expected);
    support::differences_between(
        &expected_value,
        &support::native_value(&object(rows)["subd"]),
        path,
        differences,
    );
    let mut actual_value = support::actual_value(actual);
    let mut expected_value = expected_value;
    for (value, subd) in [(&mut actual_value, actual), (&mut expected_value, expected)] {
        for (row, sharpness) in value["edges"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(subd.edge_sharpness())
        {
            row.as_array_mut().unwrap().push(json!(sharpness));
        }
    }
    let normalized = identity::normalize(actual_value, &mut expected_value, source);
    support::differences_between(&normalized, &expected_value, path, differences);
    support::compare_limit_mesh(
        actual,
        &object(rows)["subd"]["surface_mesh"],
        path,
        differences,
    );
    if custom::value(actual) != custom::value(expected) {
        differences.push(format!(
            "{path}: properties differ: actual {} native {}",
            custom::value(actual),
            custom::value(expected)
        ));
    }
}
pub(super) fn pick(source: &SubD, point: &Value) -> PolygonEdgePoint {
    let p = Point3::try_from(serde_json::from_value::<[f64; 3]>(point.clone()).unwrap()).unwrap();
    source
        .edges()
        .iter()
        .enumerate()
        .map(|(edge, &[a, b])| {
            let line = LineSegment::try_new(
                source.control_points()[a as usize],
                source.control_points()[b as usize],
                Tolerance::MESH_VALIDATION,
            )
            .unwrap();
            let parameter = line
                .closest_parameter(p, Tolerance::MESH_VALIDATION)
                .unwrap();
            let distance = line.point_at(parameter).unwrap().distance_to(p).unwrap();
            (distance, PolygonEdgePoint { edge, parameter })
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}
pub(super) fn audit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (family, count) in [
        ("subd_insert_custom", 22),
        ("subd_insert_order", 16),
        ("subd_insert_order_repeat", 16),
    ] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let cases: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "fixtures/{}_recipes.json",
                family.replace("_repeat", "")
            )))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(record["results"].as_array().unwrap().len(), count);
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let c = &cases[name];
            let initial = support::read(&v["initial_3dm"]);
            let mut source = initial;
            let mut differences = Vec::new();
            if let Some(faces) = c["merge"].as_array() {
                source = source
                    .merge_face_components(
                        &faces
                            .iter()
                            .map(|i| SubDMergeComponent::Face(i.as_u64().unwrap() as usize))
                            .collect::<Vec<_>>(),
                    )
                    .unwrap()
                    .subd;
            }
            let native_source = support::read(&v["source_3dm"]);
            compare(
                &source,
                &native_source,
                &v["before"],
                &source,
                &format!("{name}/source"),
                &mut differences,
            );
            let mut picks = Vec::new();
            let mut canceled = false;
            let mut rejected = Vec::new();
            let clicks = v["trace"]["clicks"].as_array().unwrap();
            for (i, a) in c["actions"].as_array().unwrap().iter().enumerate() {
                if a.get("click").is_some() {
                    let click = clicks.iter().find(|x| x["action"] == i).unwrap();
                    let mut trial = picks.clone();
                    trial.push(pick(&source, &click["projected_point"]));
                    if source
                        .try_insert_edge_path(
                            &trial,
                            Some(
                                Vector3::try_new(0., 0., 1.)
                                    .unwrap()
                                    .normalized_nonzero()
                                    .unwrap(),
                            ),
                            Tolerance::DEFAULT,
                        )
                        .is_ok()
                    {
                        picks = trial;
                    } else {
                        rejected.push(i);
                    }
                } else if a == "Undo" {
                    picks.pop();
                } else if a == "Cancel" {
                    canceled = true;
                }
            }
            assert_eq!(
                rejected,
                if [
                    "merged-disjoint",
                    "disjoint-reverse",
                    "disjoint-return",
                    "disjoint-reject-undo"
                ]
                .contains(&name)
                {
                    vec![1]
                } else {
                    vec![]
                },
                "{name}"
            );
            let actual = if canceled {
                source.clone()
            } else {
                source
                    .try_insert_edge_path(
                        &picks,
                        Some(
                            Vector3::try_new(0., 0., 1.)
                                .unwrap()
                                .normalized_nonzero()
                                .unwrap(),
                        ),
                        Tolerance::DEFAULT,
                    )
                    .unwrap()
            };
            let native = support::read(&v["after_3dm"]);
            compare(
                &actual,
                &native,
                &v["after"],
                &source,
                &format!("{name}/after"),
                &mut differences,
            );
            if !differences.is_empty() {
                failures.push((name.to_owned(), differences.clone()));
            }
            report.push(json!({"id":name,"differences":differences,"picks":picks.iter().map(|p|json!([p.edge,p.parameter])).collect::<Vec<_>>(),
                "actual":support::actual_value(&actual),"native":support::actual_value(&native),"actual_properties":custom::value(&actual),"native_properties":custom::value(&native)}));
        }
    }
    if let Ok(path) = std::env::var("VIBOCEROS_SUBD_INSERT_CUSTOM_AUDIT_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "SubD InsertPoint differs: {failures:?}"
    );
}
