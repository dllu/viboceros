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

#[test]
fn natural_rebuild_replays_default_native_warped_and_curved_boundaries() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_natural.json"
    ))
    .unwrap();
    replay(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
}

#[test]
fn closed_rebuild_replays_native_seams_poles_and_independent_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_closed.json"
    ))
    .unwrap();
    replay(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
}

#[test]
fn closed_rebuild_replays_native_degree_parity_and_small_counts() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_closed_degrees.json"
    ))
    .unwrap();
    replay(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap());
}

#[test]
fn seam_trim_rebuild_replays_native_cylinder_patches_and_holes() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_seam_trim.json"
    ))
    .unwrap();
    replay_with_knots(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap(), 1e-12);
}

#[test]
fn seam_crossing_hole_rebuild_preserves_native_trim_incidence() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_crossing_hole.json"
    ))
    .unwrap();
    replay_with_knots(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap(), 1e-12);
}

#[test]
fn singular_trim_rebuild_replays_native_caps_holes_wedges_and_chart_edits() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/surface_rebuild_singular_trim.json"
    ))
    .unwrap();
    replay_with_knots(&q, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap(), 1e-12);
}

fn boundary_samples(curves: &[&NurbsCurve]) -> Vec<Vec<Point3>> {
    curves
        .iter()
        .map(|c| {
            (0..=16)
                .map(|j| {
                    c.evaluate(c.parameter_at(j as Real / 16.).unwrap())
                        .unwrap()
                })
                .collect()
        })
        .collect()
}
fn boundary_witness(
    curves: &[&NurbsCurve],
    samples: &[Vec<Point3>],
    p: Point3,
    tolerance: Tolerance,
) -> Real {
    let mut seeds = samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            (
                s.iter()
                    .map(|q| q.distance_to(p).unwrap())
                    .fold(f64::INFINITY, f64::min),
                i,
            )
        })
        .collect::<Vec<_>>();
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, i) in seeds {
        let c = curves[i];
        let t = c.closest_parameter(p, tolerance).unwrap();
        let distance = c.evaluate(t).unwrap().distance_to(p).unwrap();
        if distance < 2e-6 {
            return distance;
        }
    }
    f64::INFINITY
}

fn replay(q: &Value, tolerance: Tolerance) {
    replay_with_knots(q, tolerance, 0.);
}
fn replay_with_knots(q: &Value, tolerance: Tolerance, knot_epsilon: Real) {
    for row in q["results"].as_array().unwrap() {
        let v = &row["value"];
        let spec = &v["spec"];
        eprintln!("retrimming replay {}", v["case"]);
        let original = source(&v["before"][0]["brep"], tolerance);
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let id = doc.add_geometry(Geometry::Brep(original)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc,&format!("Rebuild UDegree={} VDegree={} UPointCount={} VPointCount={} DeleteInput=No ReTrim={}",spec["degree"][0],spec["degree"][1],spec["count"][0],spec["count"][1],if spec["retrim"]==true{"Yes"}else{"No"})).unwrap_or_else(|error|panic!("{}: {error}",v["case"]));
        assert_eq!(
            doc.objects().next().unwrap(),
            &before[0],
            "{} source",
            v["case"]
        );
        let Geometry::Brep(actual) = doc.objects().last().unwrap().geometry() else {
            panic!()
        };
        let expected = &v["command"]["after_script"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["brep"];
        let surface = actual.faces()[0].surface();
        let native =
            crate::tween_surfaces::tests::native_surface(&expected["faces"][0]["definition"]);
        assert_eq!(
            actual.faces()[0].is_reversed(),
            expected["topology"]["faces"][0]["reversed"]
                .as_bool()
                .unwrap()
        );
        assert_eq!(surface.degree_u(), native.degree_u());
        assert_eq!(surface.degree_v(), native.degree_v());
        assert_eq!(
            surface.control_point_count_u(),
            native.control_point_count_u()
        );
        assert_eq!(
            surface.control_point_count_v(),
            native.control_point_count_v()
        );
        if knot_epsilon > 0. {
            assert_eq!(surface.knots_u().len(), native.knots_u().len());
            assert_eq!(surface.knots_v().len(), native.knots_v().len());
            for (a, b) in surface
                .knots_u()
                .iter()
                .chain(surface.knots_v())
                .zip(native.knots_u().iter().chain(native.knots_v()))
            {
                assert!((a - b).abs() < knot_epsilon);
            }
        } else {
            assert_eq!(surface.knots_u(), native.knots_u());
            assert_eq!(surface.knots_v(), native.knots_v());
        }
        for (a, b) in surface.control_points().iter().zip(native.control_points()) {
            assert!(a.point().distance_to(b.point()).unwrap() < 1e-6);
            assert_eq!(a.weight(), b.weight());
        }
        if knot_epsilon > 0. && actual.edges().len() != expected["edges"].as_array().unwrap().len()
        {
            eprintln!(
                "{} segmentation: {} local edges, {} native edges",
                v["case"],
                actual.edges().len(),
                expected["edges"].as_array().unwrap().len()
            );
            assert_eq!(
                actual.faces()[0].loops().len(),
                expected["faces"][0]["loops"].as_array().unwrap().len()
            );
            assert_eq!(
                actual.is_solid(),
                expected["topology"]["solid"].as_bool().unwrap()
            );
            let local_seams = actual.faces()[0]
                .loops()
                .iter()
                .flat_map(|l| l.trims())
                .filter(|t| t.trim_type() == BrepTrimType::Seam)
                .count();
            let native_seams = expected["topology"]["faces"][0]["loops"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|l| l["trims"].as_array().unwrap())
                .filter(|t| t["type"] == "Seam")
                .count();
            assert_eq!(local_seams, native_seams);
            let local_singular = actual.faces()[0]
                .loops()
                .iter()
                .flat_map(|l| l.trims())
                .filter(|t| t.trim_type() == BrepTrimType::Singular)
                .collect::<Vec<_>>();
            let native_singular = expected["topology"]["faces"][0]["loops"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|l| l["trims"].as_array().unwrap())
                .filter(|t| t["type"] == "Singular")
                .count();
            assert_eq!(local_singular.len(), native_singular);
            for t in local_singular {
                assert!(t.edge().is_none());
                assert_eq!(t.vertices()[0], t.vertices()[1]);
            }
            let native_curves = expected["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| curve(&e["curve"]["definition"]))
                .collect::<Vec<_>>();
            let local_curves = actual.edges().iter().map(|e| e.curve()).collect::<Vec<_>>();
            let local_samples = boundary_samples(&local_curves);
            let native_refs = native_curves.iter().collect::<Vec<_>>();
            let native_samples = boundary_samples(&native_refs);
            for e in expected["edges"].as_array().unwrap() {
                for sample in e["curve"]["samples"].as_array().unwrap() {
                    let p = point(sample);
                    let distance = boundary_witness(&local_curves, &local_samples, p, tolerance);
                    assert!(distance < 2e-6, "{} {:?}: {distance}", v["case"], p);
                }
            }
            for e in actual.edges() {
                for i in 0..=16 {
                    let p = e
                        .curve()
                        .evaluate(e.curve().parameter_at(i as Real / 16.).unwrap())
                        .unwrap();
                    let distance = boundary_witness(&native_refs, &native_samples, p, tolerance);
                    assert!(distance < 2e-6, "{} local {:?}: {distance}", v["case"], p);
                }
            }
            let accepted = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), accepted);
            continue;
        }
        assert_eq!(
            actual.edges().len(),
            expected["edges"].as_array().unwrap().len(),
            "{}",
            v["case"]
        );
        assert_eq!(
            actual.vertices().len(),
            expected["vertices"].as_array().unwrap().len(),
            "{}",
            v["case"]
        );
        assert_eq!(
            actual.is_solid(),
            expected["topology"]["solid"].as_bool().unwrap(),
            "{}",
            v["case"]
        );
        let types = actual.faces()[0]
            .loops()
            .iter()
            .flat_map(|l| l.trims())
            .map(|t| format!("{:?}", t.trim_type()))
            .collect::<Vec<_>>();
        let native_types = expected["topology"]["faces"][0]["loops"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|l| l["trims"].as_array().unwrap())
            .map(|t| t["type"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(types, native_types, "{}", v["case"]);
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
