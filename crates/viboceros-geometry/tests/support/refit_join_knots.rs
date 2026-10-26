//! Single-knot parity does not establish Loft's composite fitting policy.
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};

fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(value["knots"].clone()).unwrap(),
    )
    .unwrap()
}

pub fn compare<E: std::fmt::Debug>(
    mut remove: impl FnMut(&NurbsCurve, f64) -> Result<NurbsCurve, E>,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let read = |folder| -> Value {
        serde_json::from_slice(
            &std::fs::read(root.join(format!("{folder}/loft_refit_join_knots.json"))).unwrap(),
        )
        .unwrap()
    };
    let query = read("fixtures");
    let native = read("observations");
    let mut count = 0;
    let mut maximum = 0_f64;
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let source = curve(&op["curve"]);
        let before = source.clone();
        let expected = curve(&row["value"]);
        let actual = remove(&source, op["parameter"].as_f64().unwrap()).unwrap();
        assert_eq!(source, before);
        assert_eq!(actual.degree(), expected.degree());
        assert_eq!(actual.knots(), expected.knots());
        assert_eq!(
            actual.control_points().len(),
            source.control_points().len() - 1
        );
        assert_eq!(
            actual.control_points().len(),
            expected.control_points().len()
        );
        for (a, b) in actual
            .control_points()
            .iter()
            .zip(expected.control_points())
        {
            let error = a.point().distance_to(b.point()).unwrap();
            maximum = maximum.max(error);
            assert!(error <= 1e-9, "{}: {error}", op["id"]);
            assert!((a.weight() - b.weight()).abs() <= 1e-12);
        }
        for i in 0..=64 {
            let t = i as f64 / 64.;
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(t).unwrap())
                    .unwrap()
                    <= 1e-9
            );
        }
        count += 1;
    }
    println!("REFIT_JOIN_KNOTS cases={count} maximum_control_distance={maximum}");
    assert_eq!(count, 4);
}
