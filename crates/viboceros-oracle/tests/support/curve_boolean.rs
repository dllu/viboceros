//! Unordered object matching with strict metadata and sampled rational loci.
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};
pub const FAMILIES: [(&str, usize); 8] = [
    ("curve_boolean_pilot", 13),
    ("curve_boolean_holdouts", 15),
    ("curve_boolean_interactions", 18),
    ("curve_boolean_confirmations", 13),
    ("curve_boolean_weighted", 3),
    ("curve_boolean_subcurves", 10),
    ("curve_boolean_subcurve_holdouts", 7),
    ("curve_boolean_subcurve_edges", 4),
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
        let ops = query["operations"].as_array().unwrap();
        let results = native["results"].as_array().unwrap();
        assert_eq!(ops.len(), count);
        assert_eq!(results.len(), count);
        for (op, r) in ops.iter().zip(results) {
            assert_eq!(op["id"], r["id"]);
            rows.push((op.clone(), r["value"].clone()));
        }
    }
    rows
}
fn curve(v: &Value) -> NurbsCurve {
    let d: viboceros_oracle::NurbsCurveDefinition = serde_json::from_value(v.clone()).unwrap();
    let c = NurbsCurve::try_new_rational(
        d.degree,
        d.control_points
            .iter()
            .map(|p| WeightedPoint3::try_new(Point3::try_from(p.point).unwrap(), p.weight).unwrap())
            .collect(),
        d.knots,
    )
    .unwrap();
    if let Some([a, b]) = d.domain {
        c.try_reparameterized(a..=b).unwrap()
    } else {
        c
    }
}
fn boundaries(v: &Value) -> Vec<Vec<NurbsCurve>> {
    if v["kind"] == "curve" {
        return vec![vec![curve(&v["curve"])]];
    }
    let b = &v["brep"];
    b["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["loops"].as_array().unwrap())
        .map(|l| {
            l["trims"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    curve(&b["edges"][t["edge"].as_u64().unwrap() as usize]["curve"]["definition"])
                })
                .collect()
        })
        .collect()
}
fn directed(a: &[NurbsCurve], b: &[NurbsCurve]) -> f64 {
    let mut max = 0f64;
    for c in a {
        for (lo, hi) in c.spans() {
            for i in 0..=8 {
                let p = c.evaluate(lo + (hi - lo) * i as f64 / 8.).unwrap();
                let distance = b
                    .iter()
                    .map(|c| {
                        let t = c.closest_parameter(p, Tolerance::DEFAULT).unwrap();
                        p.distance_to(c.evaluate(t).unwrap()).unwrap()
                    })
                    .fold(f64::INFINITY, f64::min);
                max = max.max(distance);
            }
        }
    }
    max
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Comparison {
    pub model_error: f64,
    pub area_error: f64,
    pub relative_area_error: f64,
    pub topology_gap: bool,
}
impl Comparison {
    fn combine(&mut self, other: Self) {
        self.model_error = self.model_error.max(other.model_error);
        self.area_error = self.area_error.max(other.area_error);
        self.relative_area_error = self.relative_area_error.max(other.relative_area_error);
        self.topology_gap |= other.topology_gap;
    }
}

/// Keep the 1e-6 model gate. Six named circle constructions have measured
/// native endpoint/area differences and remain explicitly unqualified. The
/// ceilings bound those existing gaps; any other geometric regression fails.
pub fn qualify(id: &str, r: Comparison) -> Result<bool, String> {
    if r.topology_gap
        && !matches!(
            id,
            "curve_boolean_holdout_dangling"
                | "curve_boolean_confirmation_precise_circles_False"
                | "curve_boolean_confirmation_precise_circles_True"
                | "curve_boolean_confirmation_three_circles"
                | "curve_boolean_confirmation_scaled_small"
                | "curve_boolean_confirmation_scaled_large"
        )
    {
        return Err(format!("{id}: unexpected topology gap"));
    }
    if ![r.model_error, r.area_error, r.relative_area_error]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(format!("{id}: non-finite comparison"));
    }
    if r.model_error <= 1e-6 && (r.area_error <= 1e-8 || r.relative_area_error <= 1e-12) {
        return Ok(true);
    }
    let ceilings = match id {
        "curve_boolean_holdout_circles_False"
        | "curve_boolean_confirmation_precise_circles_False" => Some((4.22e-6, 4.96e-5)),
        "curve_boolean_holdout_circles_True"
        | "curve_boolean_confirmation_precise_circles_True" => Some((4.22e-6, 1.07e-5)),
        "curve_boolean_confirmation_three_circles" => Some((2.20e-5, 5.14e-5)),
        "curve_boolean_confirmation_scaled_large" => Some((0.00422, 49.48)),
        _ => None,
    };
    if ceilings.is_some_and(|(model, area)| r.model_error <= model && r.area_error <= area) {
        Ok(false)
    } else {
        Err(format!("{id}: unexpected geometry gap {r:?}"))
    }
}
fn object_error(a: &Value, b: &Value) -> Result<Comparison, String> {
    for key in [
        "kind",
        "source",
        "selected",
        "attributes",
        "geometry_user_text",
    ] {
        if a[key] != b[key] {
            return Err(format!("{key}: {} != {}", a[key], b[key]));
        }
    }
    if !b["native_valid"].as_bool().unwrap() {
        return Err("invalid native result".into());
    }
    if a["kind"] == "curve" && a["closed"] != b["closed"] {
        return Err("curve closure".into());
    }
    // Retained inputs are checked coefficient by coefficient. Apart from
    // dehomogenization rounding, their recorded representation is unchanged.
    if a["source"].is_number() && a["kind"] == "curve" {
        let x = curve(&a["curve"]);
        let y = curve(&b["curve"]);
        if x.degree() != y.degree()
            || x.knots() != y.knots()
            || x.domain() != y.domain()
            || x.control_points().len() != y.control_points().len()
        {
            return Err("retained source representation".into());
        }
        let mut max = 0f64;
        for (x, y) in x.control_points().iter().zip(y.control_points()) {
            if x.weight() != y.weight() {
                return Err("retained source weight".into());
            }
            max = max.max(x.point().distance_to(y.point()).unwrap());
        }
        return Ok(Comparison {
            model_error: max,
            ..Comparison::default()
        });
    }
    let loops_a = boundaries(a);
    let loops_b = boundaries(b);
    if loops_a.len() != loops_b.len() {
        return Err("loop count".into());
    }
    let mut used = vec![false; loops_b.len()];
    let mut result = Comparison::default();
    if a["kind"] == "brep" {
        let stats = |v: &Value| {
            let topology = &v["brep"]["topology"];
            let mut loops = topology["faces"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|f| f["loops"].as_array().unwrap())
                .map(|l| {
                    let mut types = l["trims"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|t| t["type"].as_str().unwrap().to_owned())
                        .collect::<Vec<_>>();
                    types.sort();
                    (l["outer"].as_bool().unwrap(), types)
                })
                .collect::<Vec<_>>();
            loops.sort();
            (
                topology["vertices"].as_u64().unwrap(),
                topology["edges"].as_array().unwrap().len(),
                loops,
            )
        };
        result.topology_gap = stats(a) != stats(b);
    }
    for x in &loops_a {
        let best = loops_b
            .iter()
            .enumerate()
            .filter(|(i, _)| !used[*i])
            .map(|(i, y)| (i, directed(x, y).max(directed(y, x))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        used[best.0] = true;
        result.model_error = result.model_error.max(best.1);
    }
    if let Some(area) = a["area"].as_f64() {
        let native = b["area"].as_f64().ok_or("missing native area")?;
        result.area_error = (area - native).abs();
        result.relative_area_error = result.area_error / area.abs().max(1e-300);
    }
    Ok(result)
}
pub fn compare(a: &Value, b: &Value) -> Result<Comparison, String> {
    if a["success"] != b["success"] {
        return Err(format!("success {} != {}", a["success"], b["success"]));
    }
    let mut result = Comparison::default();
    for phase in ["before", "after", "undo", "redo"] {
        if a[phase]["group_alive"] != b[phase]["group_alive"] {
            return Err(format!("{phase}: group definition"));
        }
        let x = a[phase]["objects"].as_array().unwrap();
        let y = b[phase]["objects"].as_array().unwrap();
        if x.len() != y.len() {
            return Err(format!("{phase}: {} != {} objects", x.len(), y.len()));
        }
        let mut used = vec![false; y.len()];
        for object in x {
            let candidates =
                y.iter()
                    .enumerate()
                    .filter(|(i, _)| !used[*i])
                    .filter_map(|(i, target)| {
                        object_error(object, target).ok().map(|error| (i, error))
                    });
            let Some((i, error)) =
                candidates.min_by(|a, b| a.1.model_error.total_cmp(&b.1.model_error))
            else {
                return Err(format!(
                    "{phase}: no object with matching kind, source, selection, attributes, text, closure and loop count: {object}"
                ));
            };
            used[i] = true;
            result.combine(error);
        }
    }
    Ok(result)
}
