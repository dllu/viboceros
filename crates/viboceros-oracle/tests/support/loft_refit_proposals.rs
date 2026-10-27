//! Compare proposed sections through the established common-basis Loft path.
use serde_json::{Value, json};
use viboceros_geometry::{GeometryError, NurbsCurve};
use viboceros_oracle::{ProbeRequest, run_request};
#[allow(dead_code)]
#[path = "../../../viboceros-geometry/tests/support/loft_rebuild.rs"]
mod comparison;
pub use comparison::difference;

pub fn read(family: &str, folder: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    serde_json::from_slice(&std::fs::read(root.join(format!("{folder}/{family}.json"))).unwrap())
        .unwrap()
}

fn surfaces(value: &Value) -> &Value {
    if value.is_array() {
        value
    } else if value.get("surfaces").is_some() {
        &value["surfaces"]
    } else {
        &value["outputs"][0]["surfaces"]
    }
}

/// This exercises geometry proposals, not command admission: the proposed
/// definitions replace the inputs and further fitting is explicitly disabled.
pub fn compare(
    family: &str,
    count: usize,
    expected: (usize, usize, usize),
    label: &str,
    propose: fn(&NurbsCurve, f64, f64) -> Result<Option<NurbsCurve>, GeometryError>,
) {
    let query = read(family, "fixtures");
    let native = read(family, "observations");
    let operations = query["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!((operations.len(), rows.len()), (count, count));
    let (mut matched, mut differing, mut declined, mut maximum) = (0, 0, 0, 0_f64);
    for (op, row) in operations.iter().zip(rows) {
        assert_eq!(op["id"], row["id"]);
        let mut proposed = Vec::new();
        let sources = op["curves"].as_array().unwrap();
        for definition in sources {
            let source = comparison::definitions::curve(definition);
            let before = source.clone();
            let result = propose(
                &source,
                op["refit_tolerance"].as_f64().unwrap(),
                op["refit_angle_tolerance"].as_f64().unwrap_or(0.),
            )
            .unwrap();
            assert_eq!(source, before);
            let Some(fit) = result else {
                break;
            };
            proposed.push(json!({"degree":fit.degree(), "knots":fit.knots(),
                "control_points":fit.control_points().iter().map(|c|
                    json!({"point":c.point().to_array(),"weight":c.weight()})).collect::<Vec<_>>()}));
        }
        if proposed.len() != sources.len() {
            declined += 1;
            println!("{label}_DECLINED {}", op["id"]);
            continue;
        }
        let mut operation = op.clone();
        operation["curves"] = json!(proposed);
        operation["refit_tolerance"] = json!(0.);
        let mut request = query.clone();
        request["operations"] = json!([operation]);
        let actual =
            run_request(&serde_json::from_value::<ProbeRequest>(request).unwrap()).unwrap();
        let value = &actual.results[0].value;
        if value.get("before").is_some() {
            assert_eq!(value["before"], value["after"]);
        }
        match difference(surfaces(value), surfaces(&row["value"])) {
            Ok(d) if d <= 1e-9 => {
                matched += 1;
                maximum = maximum.max(d);
            }
            difference => {
                differing += 1;
                println!("{label}_COUNTEREXAMPLE {} {difference:?}", op["id"]);
            }
        }
    }
    println!(
        "{label} {family} matched={matched} differing={differing} maximum={maximum} declined={declined}"
    );
    assert_eq!((matched, differing, declined), expected);
}
