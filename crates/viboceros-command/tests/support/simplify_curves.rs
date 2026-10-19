use serde_json::Value;
use viboceros_geometry::{CurveRef, Point3};
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
pub fn compare(curve: CurveRef<'_>, reference: &Value, name: &str) -> f64 {
    let mut gap = 0_f64;
    let samples = reference["samples"].as_array().unwrap();
    let d = curve.domain();
    for (a, b) in [*d.start(), *d.end()]
        .into_iter()
        .zip(reference["domain"].as_array().unwrap())
    {
        gap = gap.max((a - b.as_f64().unwrap()).abs());
    }
    assert_eq!(
        curve.is_closed().unwrap(),
        reference["closed"].as_bool().unwrap(),
        "{name}: closure"
    );
    for (i, p) in samples.iter().enumerate() {
        let t = curve
            .parameter_at(i as f64 / (samples.len() - 1) as f64)
            .unwrap();
        gap = gap.max(curve.evaluate(t).unwrap().distance_to(point(p)).unwrap());
    }
    match (curve, reference["type"].as_str().unwrap()) {
        (CurveRef::Line(_), "line") => {}
        (CurveRef::Arc(_) | CurveRef::Circle(_), "arc") => {}
        (CurveRef::Polyline(c), "polyline") => {
            assert_eq!(
                c.vertices().len(),
                reference["vertices"].as_array().unwrap().len(),
                "{name}: polyline count"
            );
            for (a, b) in c
                .vertices()
                .iter()
                .zip(reference["vertices"].as_array().unwrap())
            {
                gap = gap.max(a.distance_to(point(b)).unwrap());
            }
            for (a, b) in c
                .parameters()
                .iter()
                .zip(reference["parameters"].as_array().unwrap())
            {
                gap = gap.max((a - b.as_f64().unwrap()).abs());
            }
        }
        (CurveRef::NurbsCurve(c), "nurbs") => {
            assert_eq!(
                c.has_rational_representation(),
                reference["rational"].as_bool().unwrap(),
                "{name}: rational form"
            );
            let definition = &reference["definition"];
            assert_eq!(
                c.degree(),
                definition["degree"].as_u64().unwrap() as usize,
                "{name}: degree"
            );
            assert_eq!(
                c.control_points().len(),
                definition["control_points"].as_array().unwrap().len(),
                "{name}: CV count"
            );
            assert_eq!(
                c.knots().len(),
                definition["knots"].as_array().unwrap().len(),
                "{name}: knots"
            );
            for (a, b) in c
                .knots()
                .iter()
                .zip(definition["knots"].as_array().unwrap())
            {
                gap = gap.max((a - b.as_f64().unwrap()).abs());
            }
            for (a, b) in c
                .control_points()
                .iter()
                .zip(definition["control_points"].as_array().unwrap())
            {
                gap = gap
                    .max(a.point().distance_to(point(&b["point"])).unwrap())
                    .max((a.weight() - b["weight"].as_f64().unwrap()).abs());
            }
        }
        (CurveRef::PolyCurve(c), "polycurve") => {
            assert_eq!(
                c.segments().len(),
                reference["segments"].as_array().unwrap().len(),
                "{name}: segments"
            );
            for (a, b) in c
                .parameters()
                .iter()
                .zip(reference["parameters"].as_array().unwrap())
            {
                gap = gap.max((a - b.as_f64().unwrap()).abs());
            }
            for (a, b) in c
                .segments()
                .iter()
                .zip(reference["segments"].as_array().unwrap())
            {
                gap = gap.max(compare(a.as_ref(), b, name));
            }
        }
        _ => panic!(
            "{name}: representation {:?} vs {}",
            curve, reference["type"]
        ),
    }
    gap
}

pub fn compare_selection(document: &viboceros_document::Document, reference: &Value, name: &str) {
    for object in reference.as_array().unwrap() {
        let id = object["id"]
            .as_str()
            .unwrap()
            .parse::<viboceros_document::ObjectId>()
            .unwrap();
        assert_eq!(
            document.is_selected(id),
            object["selected"].as_bool().unwrap(),
            "{name}: history selection"
        );
    }
}
