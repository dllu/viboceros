//! Spatial checks against every used native C3 edge, independent of source labels.
use serde_json::Value;
use viboceros_geometry::{Brep, NurbsCurve, Point3, Tolerance, WeightedPoint3};
pub const FAMILIES: [(&str, usize); 9] = [
    ("planar_chains", 35),
    ("planar_chains_holdouts", 27),
    ("planar_chains_contexts", 52),
    ("planar_chains_salvage", 8),
    ("planar_chains_filled", 6),
    ("planar_chains_modes", 8),
    ("planar_chain_networks", 33),
    ("planar_chain_network_holdouts", 17),
    ("planar_network_crossings", 12),
];
pub fn rows(
    base: &std::path::Path,
    read: fn(&std::path::Path, &str) -> Value,
) -> Vec<(Value, Value)> {
    let mut rows = Vec::new();
    for (family, count) in FAMILIES {
        let query: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = read(base, family);
        let operations = query["operations"].as_array().unwrap();
        let results = native["results"].as_array().unwrap();
        assert_eq!(operations.len(), count);
        assert_eq!(results.len(), count);
        for (op, row) in operations.iter().zip(results) {
            assert_eq!(op["id"], row["id"]);
            rows.push((op.clone(), row["value"].clone()));
        }
    }
    rows
}
fn curve(value: &Value) -> NurbsCurve {
    let d: viboceros_oracle::NurbsCurveDefinition = serde_json::from_value(value.clone()).unwrap();
    let c = NurbsCurve::try_new_rational(
        d.degree,
        d.control_points
            .iter()
            .map(|p| WeightedPoint3::try_new(Point3::try_from(p.point).unwrap(), p.weight).unwrap())
            .collect(),
        d.knots,
    )
    .unwrap();
    match d.domain {
        Some([a, b]) => c.try_reparameterized(a..=b).unwrap(),
        None => c,
    }
}
fn directed(a: &[&NurbsCurve], b: &[&NurbsCurve], tolerance: Tolerance) -> f64 {
    let mut maximum = 0f64;
    for curve in a {
        for (lo, hi) in curve.spans() {
            for i in 0..=16 {
                let point = curve.evaluate(lo + (hi - lo) * i as f64 / 16.).unwrap();
                let distance = b
                    .iter()
                    .map(|target| {
                        let t = target.closest_parameter(point, tolerance).unwrap();
                        point.distance_to(target.evaluate(t).unwrap()).unwrap()
                    })
                    .fold(f64::INFINITY, f64::min);
                maximum = maximum.max(distance);
            }
        }
    }
    maximum
}
fn error(actual: &Brep, native: &Value, tolerance: Tolerance) -> f64 {
    let edges = native["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| curve(&e["curve"]["definition"]))
        .collect::<Vec<_>>();
    let loops = native["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["loops"].as_array().unwrap())
        .collect::<Vec<_>>();
    if actual
        .faces()
        .iter()
        .map(|f| f.loops().len())
        .sum::<usize>()
        != loops.len()
    {
        return f64::INFINITY;
    }
    let mut used = vec![false; loops.len()];
    let mut maximum = 0f64;
    for boundary in actual.faces().iter().flat_map(|f| f.loops()) {
        let a = boundary
            .trims()
            .iter()
            .map(|t| actual.edges()[t.edge().unwrap()].curve())
            .collect::<Vec<_>>();
        let mut best = (usize::MAX, f64::INFINITY);
        for (i, native_loop) in loops.iter().enumerate() {
            if used[i]
                || (native_loop["outer"] == true)
                    != (format!("{:?}", boundary.loop_type()) == "Outer")
            {
                continue;
            }
            let b = native_loop["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| &edges[t["edge"].as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            let distance = directed(&a, &b, tolerance).max(directed(&b, &a, tolerance));
            if distance < best.1 {
                best = (i, distance);
            }
        }
        if best.0 == usize::MAX {
            return f64::INFINITY;
        }
        used[best.0] = true;
        maximum = maximum.max(best.1);
    }
    maximum
}
/// Returns the largest bidirectional sample distance; loop and surface pairing
/// are one-to-one. This measures the locus, not native knot/trim encoding parity.
pub fn assert_geometry(actual: &[&Brep], phase: &Value, tolerance: Tolerance, case: &str) -> f64 {
    let native = phase
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o["kind"] == "brep")
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), native.len(), "{case}");
    let mut used = vec![false; native.len()];
    let mut maximum = 0f64;
    for b in actual {
        let (i, distance) = native
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(i, row)| (i, error(b, &row["brep"], tolerance)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(
            distance <= tolerance.absolute(),
            "{case}: boundary distance {distance}"
        );
        let area = b.area(tolerance).unwrap();
        let expected = native[i]["area"].as_f64().unwrap();
        assert!(
            (area - expected).abs() <= 1e-9 + 1e-12 * area.abs().max(expected.abs()),
            "{case}: area {area} != {expected}"
        );
        used[i] = true;
        maximum = maximum.max(distance);
    }
    maximum
}
