//! Read public native B-rep definitions solely for expected geometry.
use super::*;
use viboceros_geometry::{
    BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, NurbsCurve2,
    Point2, SurfaceIso, WeightedPoint2,
};

pub fn expected(record: &Value, tolerance: Tolerance) -> Brep {
    let b = &record["brep"];
    let vertices = b["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| BrepVertex::try_new(point(&v["point"]), v["tolerance"].as_f64().unwrap()).unwrap())
        .collect();
    let edges = b["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(b["topology"]["edges"].as_array().unwrap())
        .map(|(e, vertices)| {
            BrepEdge::try_new(
                serde_json::from_value(vertices.clone()).unwrap(),
                curve(&e["curve"]["definition"]),
                e["tolerance"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let faces = b["faces"]
        .as_array()
        .unwrap()
        .iter()
        .zip(b["topology"]["faces"].as_array().unwrap())
        .map(|(definition, topology)| {
            let loops = definition["loops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(topology["loops"].as_array().unwrap())
                .map(|(curves, boundary)| {
                    let trims = curves
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(boundary["trims"].as_array().unwrap())
                        .map(|(c, t)| {
                            let uv_tolerance =
                                serde_json::from_value(c["tolerance"].clone()).unwrap();
                            let c = &c["definition"];
                            let controls = c["control_points"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|p| {
                                    WeightedPoint2::try_new(
                                        Point2::try_new(
                                            p["point"][0].as_f64().unwrap(),
                                            p["point"][1].as_f64().unwrap(),
                                        )
                                        .unwrap(),
                                        p["weight"].as_f64().unwrap(),
                                    )
                                    .unwrap()
                                })
                                .collect();
                            let curve = NurbsCurve2::try_new_rational(
                                c["degree"].as_u64().unwrap() as usize,
                                controls,
                                serde_json::from_value(c["knots"].clone()).unwrap(),
                            )
                            .unwrap();
                            let kind = match t["type"].as_str().unwrap() {
                                "Mated" => BrepTrimType::Mated,
                                "Boundary" => BrepTrimType::Boundary,
                                "Seam" => BrepTrimType::Seam,
                                "Singular" => BrepTrimType::Singular,
                                other => panic!("unexpected native trim {other}"),
                            };
                            BrepTrim::try_new(
                                serde_json::from_value(t["vertices"].clone()).unwrap(),
                                t["edge"].as_u64().map(|n| n as usize),
                                t["reversed"].as_bool().unwrap(),
                                curve,
                                kind,
                                SurfaceIso::NotIso,
                                uv_tolerance,
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(
                        if boundary["outer"] == true {
                            BrepLoopType::Outer
                        } else {
                            BrepLoopType::Inner
                        },
                        trims,
                    )
                    .unwrap()
                })
                .collect();
            BrepFace::try_new(
                surface(&definition["definition"]),
                topology["reversed"].as_bool().unwrap(),
                loops,
            )
            .unwrap()
        })
        .collect();
    Brep::try_new(vertices, edges, faces, tolerance).unwrap()
}
