//! Independent high-accuracy SDK lengths constrain the mathematical result.
use serde_json::Value;
use viboceros_geometry::{CurveRef, NurbsCurve, Point3, WeightedPoint3};
// Native GetLength's default fractional tolerance is 1e-8. Independent
// SDK captures with 1e-12 bound the mathematical controls separately below.
pub const COMMAND_EPSILON: f64 = 5e-8;
pub fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn ideal(curve: CurveRef<'_>, sdk: &Value, recipe: &Value, target: Point3) -> f64 {
    let c = curve.to_nurbs().unwrap();
    let definition = &sdk["definition"];
    let source = definition["control_points"].as_array().unwrap();
    let n = source.len();
    assert_eq!(c.control_points().len(), n);
    assert_eq!(c.degree(), definition["degree"].as_u64().unwrap() as usize);
    let unique = if sdk["periodic"] == true {
        n - c.degree()
    } else if sdk["closed"] == true {
        n - 1
    } else {
        n
    };
    let mut factors = vec![0.; n];
    let distance = recipe["distance"].as_f64().unwrap();
    let rows = sdk["lengths"].as_array().unwrap();
    let marker_count = rows.len() - usize::from(sdk["closed"] == true);
    let projected = sdk["projection"].as_f64().unwrap();
    let selected = rows[..marker_count]
        .iter()
        .min_by(|a, b| {
            (a["parameter"].as_f64().unwrap() - projected)
                .abs()
                .total_cmp(&(b["parameter"].as_f64().unwrap() - projected).abs())
        })
        .unwrap();
    // The capture chose an integration origin independently of the command.
    // Signed precise lengths allow any selected marker to use the same data.
    let origin = sdk["anchor_parameter"].as_f64().unwrap();
    let signed = |r: &Value| {
        r["accurate"].as_f64().unwrap() * (r["parameter"].as_f64().unwrap() - origin).signum()
    };
    let center = signed(selected);
    for row in rows {
        let i = row["index"].as_u64().unwrap() as usize;
        if recipe["fix_ends"] == true && sdk["periodic"] != true && (i == 0 || i + 1 == n) {
            continue;
        }
        let ratio = (signed(row) - center).abs() / distance;
        factors[i % unique] += (-4.5 * ratio * ratio).exp();
    }
    for i in unique..n {
        factors[i] = factors[i % unique];
    }
    let source_curve = NurbsCurve::try_new_rational(
        c.degree(),
        source
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(definition["knots"].clone()).unwrap(),
    )
    .unwrap();
    let anchor = source_curve
        .evaluate(selected["parameter"].as_f64().unwrap())
        .unwrap();
    let delta = anchor.vector_to(target).unwrap();
    let mut gap = 0_f64;
    for (i, (actual, original)) in c.control_points().iter().zip(source).enumerate() {
        let expected = point(&original["point"])
            .translated(delta.scaled(factors[i]).unwrap())
            .unwrap();
        gap = gap.max(actual.point().distance_to(expected).unwrap());
    }
    assert!(
        gap <= 1e-9,
        "{}: precise SDK Gaussian difference {gap}",
        recipe["id"]
    );
    gap
}

pub fn check_many(
    doc: &viboceros_document::Document,
    expected: &Value,
    id: viboceros_document::ObjectId,
    label: &str,
) -> f64 {
    use viboceros_document::ObjectAttributes;
    let rows = expected.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len(), "{label}: object count");
    let original = rows
        .iter()
        .find(|o| o["id"].as_str().unwrap() == id.to_string())
        .unwrap();
    let mut pairs = vec![(doc.object(id).unwrap(), original)];
    let copies = doc.objects().filter(|o| o.id() != id);
    let native = rows
        .iter()
        .filter(|o| o["id"].as_str().unwrap() != id.to_string())
        .rev();
    pairs.extend(copies.zip(native));
    let mut gap = 0_f64;
    for (object, row) in pairs {
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap(),
            "{label}: selection"
        );
        assert_eq!(
            serde_json::to_value(object.geometry_user_text()).unwrap(),
            row["geometry_text"],
            "{label}: geometry text"
        );
        if object.id() != id {
            assert_eq!(
                object.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id())
            );
            assert!(object.group_ids().is_empty());
        }
        gap = gap.max(super::comparison::compare_curve(
            object.geometry().curve_ref().unwrap(),
            &row["geometry"],
            label,
            1e-12,
        ));
    }
    gap
}
