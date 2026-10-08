use super::*;
use serde_json::Value;
use viboceros_geometry::{
    BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, NurbsCurve2,
    SurfaceIso, WeightedPoint2, WeightedPoint3,
};

fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(point(&c["point"]), c["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
pub(crate) fn source(v: &Value, tolerance: Tolerance) -> Brep {
    let topology = &v["topology"];
    let vertices = v["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| BrepVertex::try_new(point(&p["point"]), p["tolerance"].as_f64().unwrap()).unwrap())
        .collect();
    let edges = v["edges"]
        .as_array()
        .unwrap()
        .iter()
        .zip(topology["edges"].as_array().unwrap())
        .map(|(e, t)| {
            BrepEdge::try_new(
                serde_json::from_value(t.clone()).unwrap(),
                curve(&e["curve"]["definition"]),
                e["tolerance"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let faces = v["faces"]
        .as_array()
        .unwrap()
        .iter()
        .zip(topology["faces"].as_array().unwrap())
        .map(|(f, t)| {
            let surface = crate::tween_surfaces::tests::native_surface(&f["definition"]);
            let loops = f["loops"]
                .as_array()
                .unwrap()
                .iter()
                .zip(t["loops"].as_array().unwrap())
                .map(|(l, t)| {
                    let trims = l
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(t["trims"].as_array().unwrap())
                        .map(|(c, t)| {
                            let d = &c["definition"];
                            let uv = NurbsCurve2::try_new_rational(
                                d["degree"].as_u64().unwrap() as usize,
                                d["control_points"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|c| {
                                        WeightedPoint2::try_new(
                                            viboceros_geometry::Point2::try_new(
                                                c["point"][0].as_f64().unwrap(),
                                                c["point"][1].as_f64().unwrap(),
                                            )
                                            .unwrap(),
                                            c["weight"].as_f64().unwrap(),
                                        )
                                        .unwrap()
                                    })
                                    .collect(),
                                serde_json::from_value(d["knots"].clone()).unwrap(),
                            )
                            .unwrap();
                            BrepTrim::try_new(
                                serde_json::from_value(t["vertices"].clone()).unwrap(),
                                t["edge"].as_u64().map(|i| i as usize),
                                t["reversed"].as_bool().unwrap(),
                                uv,
                                match t["type"].as_str().unwrap() {
                                    "Boundary" => BrepTrimType::Boundary,
                                    "Seam" => BrepTrimType::Seam,
                                    "Singular" => BrepTrimType::Singular,
                                    _ => panic!(),
                                },
                                match c["iso"].as_u64().unwrap() {
                                    0 => SurfaceIso::NotIso,
                                    1 => SurfaceIso::InteriorUConstant,
                                    2 => SurfaceIso::InteriorVConstant,
                                    3 => SurfaceIso::West,
                                    4 => SurfaceIso::South,
                                    5 => SurfaceIso::East,
                                    6 => SurfaceIso::North,
                                    _ => panic!(),
                                },
                                serde_json::from_value(c["tolerance"].clone()).unwrap(),
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(
                        if t["outer"] == true {
                            BrepLoopType::Outer
                        } else {
                            BrepLoopType::Inner
                        },
                        trims,
                    )
                    .unwrap()
                })
                .collect();
            BrepFace::try_new(surface, t["reversed"].as_bool().unwrap(), loops).unwrap()
        })
        .collect();
    Brep::try_new(vertices, edges, faces, tolerance).unwrap()
}

#[test]
fn retrim_replays_native_physical_boundaries_and_document_history() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_retrim.json"
    ))
    .unwrap();
    replay(&q, tolerance);
}

#[test]
fn retrim_replays_native_curved_rational_and_natural_boundaries() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_retrim_followup.json"
    ))
    .unwrap();
    replay(&q, tolerance);
}

fn replay(q: &Value, tolerance: Tolerance) {
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let spec = &v["spec"];
        let original = source(&v["before"][0]["brep"], tolerance);
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let id = doc.add_geometry(Geometry::Brep(original)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc,&format!("Rebuild UDegree=3 VDegree=2 UPointCount=5 VPointCount=4 DeleteInput=No ReTrim={}",if spec["retrim"]==true{"Yes"}else{"No"})).unwrap();
        let Geometry::Brep(actual) = doc.objects().last().unwrap().geometry() else {
            panic!()
        };
        let expected = &v["command"]["after_script"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["brep"];
        assert_eq!(
            actual.edges().len(),
            expected["edges"].as_array().unwrap().len(),
            "{}",
            v["case"]
        );
        let surface = actual.faces()[0].surface();
        let native =
            crate::tween_surfaces::tests::native_surface(&expected["faces"][0]["definition"]);
        assert_eq!(surface.knots_u(), native.knots_u());
        assert_eq!(surface.knots_v(), native.knots_v());
        for (a, b) in surface.control_points().iter().zip(native.control_points()) {
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-6);
        }
        for (edge, record) in actual
            .edges()
            .iter()
            .zip(expected["edges"].as_array().unwrap())
        {
            for p in record["curve"]["samples"].as_array().unwrap() {
                let p = point(p);
                let t = edge.curve().closest_parameter(p, tolerance).unwrap();
                assert!(
                    edge.curve().evaluate(t).unwrap().distance_to(p).unwrap() < 2e-6,
                    "{} {:?}",
                    v["case"],
                    p
                );
            }
        }
        let accepted = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), accepted);
    }
}
