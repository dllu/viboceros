//! Native creator representation and end-to-end conversion qualification.
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{
    Frame3, NurbsCurve, NurbsSurface, Point3, Tolerance, Vector3, WeightedPoint3,
};
fn point(v: &Value) -> Point3 {
    Point3::try_new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        v["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}
fn surface_error(actual: &NurbsSurface, expected: &Value, cyclic: bool) -> f64 {
    assert_eq!(
        [actual.degree_u(), actual.degree_v()],
        [
            expected["degree"][0].as_u64().unwrap() as usize,
            expected["degree"][1].as_u64().unwrap() as usize
        ]
    );
    assert_eq!(
        [
            actual.control_point_count_u(),
            actual.control_point_count_v()
        ],
        [
            expected["control_count"][0].as_u64().unwrap() as usize,
            expected["control_count"][1].as_u64().unwrap() as usize
        ]
    );
    let mut error = 0_f64;
    let close = |a: f64, b: f64| {
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
        (a - b).abs()
    };
    for (axis, knots) in [("knots_u", actual.knots_u()), ("knots_v", actual.knots_v())] {
        if cyclic && axis == "knots_u" {
            continue;
        }
        let expected = expected[axis].as_array().unwrap();
        assert_eq!(knots.len(), expected.len());
        for (&a, b) in knots.iter().zip(expected) {
            error = error.max(close(a, b.as_f64().unwrap()));
        }
    }
    let nu = actual.control_point_count_u();
    for v in 0..actual.control_point_count_v() {
        for u in 0..nu {
            let p = actual.control_point(u, v).unwrap();
            let row = expected["control_points"].as_array().unwrap();
            let candidates = if cyclic {
                &row[v * nu..(v + 1) * nu]
            } else {
                &row[v * nu + u..v * nu + u + 1]
            };
            let (distance, weight) = candidates
                .iter()
                .map(|q| {
                    (
                        p.point().distance_to(point(&q["point"])).unwrap(),
                        q["weight"].as_f64().unwrap(),
                    )
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap();
            error = error.max(distance).max(close(p.weight(), weight));
            assert!(distance < 1e-6, "control {u},{v}: {distance}");
        }
    }
    error
}
fn expected_surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        v["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        v["knots_u"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_f64().unwrap())
            .collect(),
        v["knots_v"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}
fn wall_error(actual: &NurbsSurface, expected: &Value) -> f64 {
    let target = expected_surface(expected);
    let mut best = f64::INFINITY;
    for transpose in [false, true] {
        for reverse_u in [false, true] {
            for reverse_v in [false, true] {
                let (nu, nv) = (
                    target.control_point_count_u(),
                    target.control_point_count_v(),
                );
                if (
                    actual.control_point_count_u(),
                    actual.control_point_count_v(),
                ) != if transpose { (nv, nu) } else { (nu, nv) }
                {
                    continue;
                }
                let mut error = 0_f64;
                for j in 0..nv {
                    for i in 0..nu {
                        let u = if reverse_u { nu - 1 - i } else { i };
                        let v = if reverse_v { nv - 1 - j } else { j };
                        let a = if transpose {
                            actual.control_point(v, u)
                        } else {
                            actual.control_point(u, v)
                        }
                        .unwrap();
                        let b = target.control_point(i, j).unwrap();
                        error = error
                            .max(a.point().distance_to(b.point()).unwrap())
                            .max((a.weight() - b.weight()).abs());
                    }
                }
                // Equal tensor controls, degree and normalized knot coordinates define
                // the same surface, allowing parameter reversal and axis transposition.
                for axis in 0..2 {
                    let (a, d) = if (axis == 0) ^ transpose {
                        (actual.knots_u(), actual.degree_u())
                    } else {
                        (actual.knots_v(), actual.degree_v())
                    };
                    let (b, db) = if axis == 0 {
                        (target.knots_u(), target.degree_u())
                    } else {
                        (target.knots_v(), target.degree_v())
                    };
                    if a.len() != b.len() || d != db {
                        error = f64::INFINITY;
                        continue;
                    }
                    let normalize = |k: &[f64], degree: usize| {
                        k.iter()
                            .map(|x| (x - k[degree]) / (k[k.len() - degree - 1] - k[degree]))
                            .collect::<Vec<_>>()
                    };
                    let a = normalize(a, d);
                    let b = normalize(b, db);
                    let reverse = if axis == 0 { reverse_u } else { reverse_v };
                    for n in 0..a.len() {
                        let av = if reverse {
                            1. - a[a.len() - 1 - n]
                        } else {
                            a[n]
                        };
                        error = error.max((av - b[n]).abs());
                    }
                }
                best = best.min(error);
            }
        }
    }
    best
}
fn context(v: &Value) -> CommandContext {
    let Some(p) = v["input"].get("construction_plane") else {
        return CommandContext::default();
    };
    CommandContext {
        construction_plane: Frame3::try_from_directions(
            point(&p[0]),
            Vector3::try_from(point(&p[1]).to_array()).unwrap(),
            Vector3::try_from(point(&p[2]).to_array()).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    }
}
#[test]
fn native_creators_preserve_extrusion_types_signed_paths_caps_and_conversion() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/creator_extrusions.json"
    ))
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut count = 0;
    for row in data["results"].as_array().unwrap() {
        eprintln!("checking creator {}", row["id"]);
        let v = &row["value"];
        let mut doc = Document::default();
        let commands = CommandRegistry::with_builtins();
        let source = v["before"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == true)
            .map(|r| {
                doc.add_geometry(Geometry::NurbsCurve(curve(&r["curve"])))
                    .unwrap()
            });
        if let Some(id) = source {
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_history().unwrap();
        commands
            .execute_in_context(
                &mut doc,
                v["input"]["application"].as_str().unwrap(),
                context(v),
            )
            .unwrap();
        assert_eq!(doc.undo_label(), v["input"]["command"].as_str());
        let output = doc.objects().find(|o| Some(o.id()) != source).unwrap();
        let output_id = output.id();
        let expected = v["created"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] != true)
            .unwrap();
        let Geometry::Brep(brep) = output.geometry() else {
            panic!("creator must retain a B-rep or typed extrusion")
        };
        assert_eq!(brep.is_extrusion(), expected.get("extrusion").is_some());
        assert_eq!(brep.is_solid(), expected["solid"].as_bool().unwrap());
        assert_eq!(
            brep.faces().len(),
            expected["faces"].as_array().unwrap().len()
        );
        let mut error = 0_f64;
        let cyclic = v["input"]["command"] == "Box";
        if let Some(e) = expected.get("extrusion") {
            let raw = brep.extrusion().unwrap().nurbs_form().unwrap();
            assert_eq!(
                raw.caps,
                [
                    e["caps"][0].as_bool().unwrap(),
                    e["caps"][1].as_bool().unwrap()
                ]
            );
            error = surface_error(&raw.surface, &e["surface"], cyclic);
        } else if brep.faces().len() == 1 {
            error = surface_error(
                brep.faces()[0].surface(),
                &expected["faces"][0]["surface"],
                false,
            );
        }
        let faces = expected["faces"].as_array().unwrap();
        // The captures place explicit-sweep caps first/last, while extrusion
        // B-rep views append their two caps after the walls.
        let walls = if expected["solid"] == true {
            if brep.is_extrusion() {
                &faces[..faces.len() - 2]
            } else {
                &faces[1..faces.len() - 1]
            }
        } else {
            faces.as_slice()
        };
        let expected_bounds = if let Some(e) = expected.get("extrusion") {
            expected_surface(&e["surface"])
                .tight_bounds(doc.tolerance())
                .unwrap()
        } else {
            let mut bounds = expected_surface(&walls[0]["surface"])
                .tight_bounds(doc.tolerance())
                .unwrap();
            for face in &walls[1..] {
                bounds = bounds
                    .union(
                        expected_surface(&face["surface"])
                            .tight_bounds(doc.tolerance())
                            .unwrap(),
                    )
                    .unwrap();
            }
            bounds
        };
        let actual_bounds = brep.tight_bounds(doc.tolerance()).unwrap();
        let bounds_error = actual_bounds
            .min()
            .distance_to(expected_bounds.min())
            .unwrap()
            .max(
                actual_bounds
                    .max()
                    .distance_to(expected_bounds.max())
                    .unwrap(),
            );
        assert!(bounds_error < 1e-6, "native bounds differ: {bounds_error}");
        if !brep.is_extrusion() {
            for face in walls {
                let e = brep
                    .faces()
                    .iter()
                    .map(|f| wall_error(f.surface(), &face["surface"]))
                    .fold(f64::INFINITY, f64::min);
                assert!(e < 1e-6, "native wall differs: {e}");
                error = error.max(e);
            }
        }
        if let Some(id) = source {
            assert_eq!(doc.object(id).unwrap(), &before[0]);
        }
        let after = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_selection();
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        let path = temp.path().join("creator.3dm");
        commands
            .execute(&mut doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let model = viboceros_io::read_3dm_file(&path, doc.tolerance()).unwrap();
        assert_eq!(model.unsupported_object_count(), 0);
        assert_eq!(model.objects.len(), doc.objects().len());
        let restored = model
            .objects
            .iter()
            .find(|o| o.id == output_id.uuid())
            .unwrap();
        let viboceros_io::ThreeDmGeometry::Brep(restored) = &restored.geometry else {
            panic!("3DM lost creator geometry")
        };
        assert_eq!(restored.is_extrusion(), expected.get("extrusion").is_some());
        assert_eq!(restored.is_solid(), expected["solid"].as_bool().unwrap());
        let restored_bounds = restored.tight_bounds(doc.tolerance()).unwrap();
        assert!(
            restored_bounds
                .min()
                .distance_to(expected_bounds.min())
                .unwrap()
                < 1e-6
        );
        assert!(
            restored_bounds
                .max()
                .distance_to(expected_bounds.max())
                .unwrap()
                < 1e-6
        );
        if let Some(e) = expected.get("extrusion") {
            error = error.max(surface_error(
                &restored.extrusion().unwrap().nurbs_form().unwrap().surface,
                &e["surface"],
                cyclic,
            ));
        }

        let mut conversion = Value::Null;
        if v["tosubd_success"] == true {
            doc.select_objects_direct([output_id], SelectionMode::Replace)
                .unwrap();
            commands
                .execute(
                    &mut doc,
                    "ToSubD UseSurface=Location SurfaceCorners=No DeleteInput=No",
                )
                .unwrap();
            let Geometry::SubD(subd) = doc.objects().last().unwrap().geometry() else {
                panic!()
            };
            let expected = v["after"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|r| r.get("subd"))
                .unwrap();
            let mut controls = expected.clone();
            controls.as_object_mut().unwrap().remove("surface_mesh");
            conversion = subd_native::compare(subd, &controls);
            subd_native::facets::certify_common_triangulation(subd, expected, &mut conversion);
            doc.clear_selection();
            doc.undo().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
            doc.redo().unwrap();
        }
        println!(
            "CREATOR_EXTRUSIONS {}",
            json!({"id":row["id"],"typed_extrusion":expected.get("extrusion").is_some(),"source_unchanged":true,"history":true,"roundtrip":true,"cyclic_box_profile":cyclic,"maximum_surface_control_error":error,"maximum_bounds_error":bounds_error,"conversion":conversion})
        );
        count += 1;
    }
    assert_eq!(count, 42);
}
