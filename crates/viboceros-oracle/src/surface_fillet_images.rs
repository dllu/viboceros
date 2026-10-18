//! Sampled image checks independent of each constructor's parameterization.
use super::super::*;

pub(super) fn surface(record: &Value) -> NurbsSurface {
    let d = &record["definition"];
    let definition: NurbsSurfaceDefinition = serde_json::from_value(json!({
        "degree_u":d["degree"][0], "degree_v":d["degree"][1],
        "control_point_count_u":d["control_count"][0],
        "control_point_count_v":d["control_count"][1],
        "control_points":d["control_points"],
        "knots_u":d["knots_u"], "knots_v":d["knots_v"],
        "domain_u":d["domain_u"], "domain_v":d["domain_v"],
    }))
    .unwrap();
    nurbs_surface_from_definition(&definition).unwrap()
}

fn membership(surface: &NurbsSurface, p: Point3, tolerance: Tolerance) {
    let (u, v) = surface.closest_parameters(p, tolerance).unwrap();
    let distance = surface.evaluate(u, v).unwrap().distance_to(p).unwrap();
    assert!(
        distance <= tolerance.absolute(),
        "surface image gap: {distance}"
    );
}

pub(super) fn assert_agreement(
    actual: &Value,
    expected: &Value,
    queries: &[[f64; 3]],
    tolerance: Tolerance,
) {
    // These recipes each produce one rectangular, untrimmed fillet sheet.
    let a = actual["geometry"]["faces"].as_array().unwrap();
    let e = expected["geometry"]["faces"].as_array().unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(e.len(), 1);
    let surfaces = [surface(&a[0]), surface(&e[0])];
    for (source, target) in [(&a[0], &surfaces[1]), (&e[0], &surfaces[0])] {
        let samples = source["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 81);
        for p in samples {
            membership(
                target,
                serde_json::from_value::<[f64; 3]>(p.clone())
                    .unwrap()
                    .try_into()
                    .unwrap(),
                tolerance,
            );
        }
    }
    let a = actual["closest_points"].as_array().unwrap();
    let e = expected["closest_points"].as_array().unwrap();
    assert_eq!(a.len(), queries.len());
    assert_eq!(e.len(), queries.len());
    for ((a, e), q) in a.iter().zip(e).zip(queries) {
        let a: Point3 = serde_json::from_value::<[f64; 3]>(a.clone())
            .unwrap()
            .try_into()
            .unwrap();
        let e: Point3 = serde_json::from_value::<[f64; 3]>(e.clone())
            .unwrap()
            .try_into()
            .unwrap();
        membership(&surfaces[1], a, tolerance);
        membership(&surfaces[0], e, tolerance);
        let q = Point3::try_from(*q).unwrap();
        let distance_gap = (q.distance_to(a).unwrap() - q.distance_to(e).unwrap()).abs();
        assert!(
            distance_gap <= tolerance.absolute(),
            "closest distance gap: {distance_gap}"
        );
    }
}
