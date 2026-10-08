use super::*;
use crate::boolean_union::tests::{Boundary, compare, regions, snapshot, witnesses};
use serde_json::Value;
use viboceros_document::{GroupId, LayerId};
use viboceros_geometry::{NurbsSurface, Point3};
fn setup(v: &Value) -> (Document, Vec<ObjectId>, Vec<LayerId>, Vec<GroupId>) {
    let mut doc = Document::default();
    if let Some(scale) = v["scale"].as_f64() {
        doc.set_tolerance(
            Tolerance::try_new(
                1e-7 * scale,
                doc.tolerance().relative(),
                doc.tolerance().angular(),
            )
            .unwrap(),
        );
    }
    let mut ids = Vec::new();
    let mut layers = Vec::new();
    for (i, shape) in v["shapes"].as_array().unwrap().iter().enumerate() {
        let b = shape_brep(shape, doc.tolerance());
        let layer = doc
            .add_layer(format!("Source {i}"), ColorRgb::BLACK)
            .unwrap();
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{i}"))
            .with_object_color(ColorRgb::new(20 + i as u8, 40, 60))
            .try_with_user_text("Code", format!("attribute-{i}"))
            .unwrap();
        let id = doc
            .add_geometry_with_attributes(Geometry::Brep(b), attrs)
            .unwrap();
        doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{i}")))
            .unwrap();
        ids.push(id);
        layers.push(layer);
    }
    let mut groups = ids
        .iter()
        .map(|&id| doc.add_group(None, [id]).unwrap())
        .collect::<Vec<_>>();
    groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
    doc.clear_history().unwrap();
    (doc, ids, layers, groups)
}

#[test]
fn planar_booleans_compute_finite_area_and_leave_solid_booleans_separate() {
    let registry = CommandRegistry::with_builtins();
    for (name, area) in [
        ("PlanarUnion", 18.),
        ("PlanarDifference", 12.),
        ("PlanarIntersection", 4.),
    ] {
        let mut doc = Document::default();
        registry.execute(&mut doc, "SrfPt 0,0 4,0 4,4 0,4").unwrap();
        registry.execute(&mut doc, "SrfPt 2,1 5,1 5,3 2,3").unwrap();
        let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
        doc.clear_history().unwrap();
        registry
            .execute(&mut doc, &format!("{name} Sources={},{}", ids[0], ids[1]))
            .unwrap();
        assert_eq!(doc.objects().len(), 1);
        let Geometry::Brep(b) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(b.faces().len(), 1);
        assert!((b.area(doc.tolerance()).unwrap() - area).abs() < 1e-9);
        assert!(!b.is_solid());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 1);
    }
}

#[test]
fn planar_command_failures_preserve_geometry_history_and_selection() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0 2,2 2").unwrap();
    registry.execute(&mut doc, "Sphere 0,0 1").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for name in ["PlanarUnion", "PlanarDifference", "PlanarIntersection"] {
        assert!(
            registry
                .execute(&mut doc, &format!("{name} Sources={},{}", ids[0], ids[1]))
                .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}

#[test]
fn planar_booleans_replay_native_geometry_defaults_identity_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_command.json"
    ))
    .unwrap();
    replay(&q);
}

#[test]
fn planar_booleans_replay_trim_holes_and_nonparallel_projection() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_topology.json"
    ))
    .unwrap();
    replay(&q);
}

fn replay(q: &Value) {
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let (mut doc, ids, layers, groups) = setup(v);
        let registry = CommandRegistry::with_builtins();
        let command = format!(
            "{} Sources={}",
            v["command_name"].as_str().unwrap(),
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        if v["pre"] == true {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        let result = registry.execute(&mut doc, &command);
        assert_eq!(
            result.is_ok(),
            v["command"]["success"] == true,
            "{} {result:?}",
            v["case"]
        );
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["command"]["after_script"],
            v["case"].as_str().unwrap(),
        );
        for (o, n) in doc
            .objects()
            .zip(v["command"]["after_script"].as_array().unwrap())
        {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            let actual = regions(b);
            let expected = serde_json::from_value::<Boundary>(n["face_regions"].clone()).unwrap();
            witnesses(&actual, &expected, "planar Boolean");
            witnesses(&expected, &actual, "planar Boolean");
        }
        registry.execute(&mut doc, "Undo").unwrap();
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["undo"]["after_script"],
            "undo",
        );
        registry.execute(&mut doc, "Redo").unwrap();
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["redo"]["after_script"],
            "redo",
        );
    }
}

fn shape_brep(shape: &Value, tolerance: Tolerance) -> Brep {
    match shape["kind"].as_str().unwrap() {
        "annulus" | "half_disk" => {
            let circle = viboceros_geometry::Circle3::try_new(
                Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(shape["center"].clone()).unwrap(),
                )
                .unwrap(),
                shape["radius"].as_f64().unwrap(),
                viboceros_geometry::Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(shape["normal"].clone()).unwrap(),
                )
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
                tolerance,
            )
            .unwrap();
            if shape["kind"] == "annulus" {
                let inner = viboceros_geometry::Circle3::try_new(
                    circle.center(),
                    shape["inner"].as_f64().unwrap(),
                    circle.normal().unwrap(),
                    tolerance,
                )
                .unwrap();
                Brep::try_planar_face_with_holes(
                    &circle.to_nurbs().unwrap(),
                    &[inner.to_nurbs().unwrap()],
                    tolerance,
                )
                .unwrap()
            } else {
                let arc = viboceros_geometry::CircularArc3::try_from_circle_angles(
                    circle,
                    0. ..=std::f64::consts::PI,
                )
                .unwrap();
                let line = viboceros_geometry::LineSegment::try_new(
                    arc.end().unwrap(),
                    arc.start().unwrap(),
                    tolerance,
                )
                .unwrap();
                let curve = viboceros_geometry::PolyCurve3::try_new(vec![
                    viboceros_geometry::CurveSegment3::Arc(arc),
                    viboceros_geometry::CurveSegment3::Line(line),
                ])
                .unwrap();
                Brep::try_planar_face(&curve.to_nurbs().unwrap(), tolerance)
                    .unwrap()
                    .try_split_edges_at_parameters(&[(0, vec![curve.parameters()[1]])], tolerance)
                    .unwrap()
            }
        }
        "disk" => {
            let circle = viboceros_geometry::Circle3::try_new(
                Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(shape["center"].clone()).unwrap(),
                )
                .unwrap(),
                shape["radius"].as_f64().unwrap(),
                viboceros_geometry::Vector3::try_from(
                    serde_json::from_value::<[f64; 3]>(shape["normal"].clone()).unwrap(),
                )
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
                tolerance,
            )
            .unwrap();
            let b = Brep::try_planar_face(&circle.to_nurbs().unwrap(), tolerance).unwrap();
            if shape["reverse"] == true {
                b.reversed()
            } else {
                b
            }
        }
        "box" => crate::boolean_union::tests::box_brep(
            serde_json::from_value(shape["bounds"].clone()).unwrap(),
        ),
        "compound" => Brep::try_disjoint_union(
            shape["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| shape_brep(p, tolerance))
                .collect(),
            tolerance,
        )
        .unwrap(),
        "sheet_hole" => {
            use viboceros_geometry::{
                BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
                NurbsCurve2, Point2, SurfaceIso,
            };
            let outer = serde_json::from_value::<[[f64; 3]; 4]>(shape["outer"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            let mut hole = serde_json::from_value::<[[f64; 3]; 4]>(shape["hole"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            hole.reverse();
            let surface = NurbsSurface::try_bilinear(outer).unwrap();
            let mut vertices = Vec::new();
            let mut edges = Vec::new();
            let mut loops = Vec::new();
            for (index, points) in [outer, hole].into_iter().enumerate() {
                let offset = vertices.len();
                vertices.extend(points.map(|p| BrepVertex::try_new(p, 0.).unwrap()));
                let mut trims = Vec::new();
                for i in 0..4 {
                    let ids = [offset + i, offset + (i + 1) % 4];
                    let e = edges.len();
                    edges.push(
                        BrepEdge::try_new(
                            ids,
                            NurbsCurve::try_new(
                                1,
                                ids.map(|i| vertices[i].point()).to_vec(),
                                vec![0., 0., 1., 1.],
                            )
                            .unwrap(),
                            0.,
                        )
                        .unwrap(),
                    );
                    let uv = |p: Point3| {
                        let (u, v) = surface.closest_parameters(p, tolerance).unwrap();
                        Point2::try_new(u, v).unwrap()
                    };
                    trims.push(
                        BrepTrim::try_new(
                            ids,
                            Some(e),
                            false,
                            NurbsCurve2::try_line(
                                uv(vertices[ids[0]].point()),
                                uv(vertices[ids[1]].point()),
                            )
                            .unwrap(),
                            BrepTrimType::Boundary,
                            SurfaceIso::NotIso,
                            [0., 0.],
                        )
                        .unwrap(),
                    );
                }
                loops.push(
                    BrepLoop::try_new(
                        if index == 0 {
                            BrepLoopType::Outer
                        } else {
                            BrepLoopType::Inner
                        },
                        trims,
                    )
                    .unwrap(),
                );
            }
            Brep::try_new(
                vertices,
                edges,
                vec![BrepFace::try_new(surface, false, loops).unwrap()],
                tolerance,
            )
            .unwrap()
        }
        _ => {
            let points = serde_json::from_value::<[[f64; 3]; 4]>(shape["points"].clone())
                .unwrap()
                .map(|p| Point3::try_from(p).unwrap());
            let b = Brep::try_surface_face(NurbsSurface::try_bilinear(points).unwrap(), tolerance)
                .unwrap();
            if shape["reverse"] == true {
                b.reversed()
            } else {
                b
            }
        }
    }
}

fn native_curve(v: &Value) -> viboceros_geometry::NurbsCurve {
    let controls = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            viboceros_geometry::WeightedPoint3::try_new(
                Point3::try_from(serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap())
                    .unwrap(),
                c["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    viboceros_geometry::NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls,
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn curve_distance(
    curves: &[&viboceros_geometry::NurbsCurve],
    point: Point3,
    tolerance: Tolerance,
) -> f64 {
    curves
        .iter()
        .map(|c| {
            c.evaluate(c.closest_parameter(point, tolerance).unwrap())
                .unwrap()
                .distance_to(point)
                .unwrap()
        })
        .fold(f64::INFINITY, f64::min)
}
#[test]
fn planar_circular_native_boundary_history_and_contact_diagnostics() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_circular.json"
    ))
    .unwrap();
    replay_curved(&q, 26);
}

#[test]
fn planar_mixed_native_boundaries_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_mixed.json"
    ))
    .unwrap();
    replay_curved(&q, 34);
}

#[test]
fn planar_mixed_native_scale_boundaries_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/planar_boolean_scale.json"
    ))
    .unwrap();
    replay_curved(&q, 27);
}

fn replay_curved(q: &Value, expected_regular: usize) {
    let mut regular = 0;
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let case = v["case"].as_str().unwrap();
        let scale = v["scale"].as_f64().unwrap_or(1.);
        let area_epsilon = if v["scale"].is_number() { 3e-4 } else { 2e-5 };
        let source_area_epsilon = if v["scale"].is_number() { 2e-4 } else { 5e-7 };
        let (mut doc, ids, layers, groups) = setup(v);
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let command = format!(
            "{} Sources={}",
            v["command_name"].as_str().unwrap(),
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        let result = registry.execute(&mut doc, &command);
        assert!(result.is_ok(), "{case} {result:?}");
        let source_native = v["command"]["after_script"].as_array().unwrap();
        let mut used = BTreeSet::new();
        let mut aligned = Vec::new();
        for object in doc.objects() {
            let Geometry::Brep(b) = object.geometry() else {
                panic!()
            };
            let edge = b.edges().first().unwrap().curve();
            let d = edge.domain();
            let probe = edge.evaluate(d.start().midpoint(*d.end())).unwrap();
            let owner = ids.iter().position(|id| *id == object.id());
            let mut candidates = source_native
                .iter()
                .enumerate()
                .filter(|(i, n)| !used.contains(i) && n["source"] == serde_json::json!(owner))
                .map(|(i, n)| {
                    let curves = n["edge_curves"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(native_curve)
                        .collect::<Vec<_>>();
                    let refs = curves.iter().collect::<Vec<_>>();
                    (curve_distance(&refs, probe, doc.tolerance()), i)
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (_, index) = candidates.first().copied().unwrap();
            used.insert(index);
            aligned.push(source_native[index].clone());
        }
        assert_eq!(used.len(), source_native.len(), "{case} output count");
        let expected = &aligned;
        let mut actual = snapshot(&doc, &ids, &layers, &groups);
        let mut native = Value::Array(aligned.clone());
        for (i, (o, n)) in doc.objects().zip(expected).enumerate() {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            assert!(
                (b.area(doc.tolerance()).unwrap() - n["area"].as_f64().unwrap()).abs()
                    / scale.powi(2)
                    < area_epsilon,
                "{case} area"
            );
            actual[i]["area"] = Value::Null;
            native[i]["area"] = Value::Null;
            if case == "planarintersection_internal_tangent" {
                assert_eq!(b.edges().len(), 1);
                assert_eq!(n["edges"], 3);
                actual[i]["edges"] = Value::Null;
                native[i]["edges"] = Value::Null;
            }
            let own = b.edges().iter().map(|e| e.curve()).collect::<Vec<_>>();
            let theirs = n["edge_curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(native_curve)
                .collect::<Vec<_>>();
            let refs = theirs.iter().collect::<Vec<_>>();
            for samples in n["edge_samples"].as_array().unwrap() {
                for p in samples.as_array().unwrap() {
                    let point =
                        Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                            .unwrap();
                    assert!(
                        curve_distance(&own, point, doc.tolerance()) / scale < 5e-6,
                        "{case} native boundary"
                    );
                }
            }
            if case == "planardifference_internal_tangent" {
                let contact = Point3::try_new(2., 0., 0.).unwrap();
                let gap = curve_distance(&refs, contact, doc.tolerance());
                assert!(
                    gap > 5e-3 && gap < 6e-3,
                    "native contact gap remains an explicit diagnostic: {gap}"
                );
            }
            for edge in &own {
                let d = edge.domain();
                for sample in 0..33 {
                    let p = edge
                        .evaluate(*d.start() + (*d.end() - *d.start()) * sample as f64 / 32.)
                        .unwrap();
                    let epsilon = if case == "planardifference_internal_tangent" {
                        6e-3
                    } else if case.ends_with("partial_arc") {
                        1e-5
                    } else {
                        5e-6
                    };
                    let error = curve_distance(&refs, p, doc.tolerance());
                    assert!(
                        error / scale < epsilon,
                        "{case} local boundary {error}: {p:?}"
                    );
                }
            }
        }
        if v["scale"].is_number() {
            let intersection = if case.contains("_strip_") {
                3.75f64.sqrt() + 8. * 0.25f64.asin()
            } else {
                2. * std::f64::consts::PI
            };
            let rectangle = if case.contains("_strip_") { 6. } else { 18. };
            let disk = 4. * std::f64::consts::PI;
            let expected_area = match v["command_name"].as_str().unwrap() {
                "PlanarUnion" => disk + rectangle - intersection,
                "PlanarIntersection" => intersection,
                "PlanarDifference" => {
                    (if case.contains("_first_polygon_") {
                        rectangle
                    } else {
                        disk
                    }) - intersection
                }
                _ => unreachable!(),
            };
            let actual_area: f64 = doc
                .objects()
                .map(|o| {
                    let Geometry::Brep(b) = o.geometry() else {
                        panic!()
                    };
                    b.area(doc.tolerance()).unwrap() / scale.powi(2)
                })
                .sum();
            assert!(
                (actual_area - expected_area).abs() < 1e-9,
                "{case} analytic area {actual_area} expected {expected_area}"
            );
        }
        compare(&actual, &native, case);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        let mut own_before = snapshot(&doc, &ids, &layers, &groups);
        let mut native_before = v["undo"]["after_script"].clone();
        for (a, b) in own_before
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(native_before.as_array_mut().unwrap())
        {
            assert!(
                (a["area"].as_f64().unwrap() - b["area"].as_f64().unwrap()).abs() / scale.powi(2)
                    < source_area_epsilon
            );
            a["area"] = Value::Null;
            b["area"] = Value::Null;
        }
        compare(&own_before, &native_before, "circular undo");
        registry.execute(&mut doc, "Redo").unwrap();
        let mut redone = snapshot(&doc, &ids, &layers, &groups);
        for o in redone.as_array_mut().unwrap() {
            o["area"] = Value::Null;
            if case == "planarintersection_internal_tangent" {
                o["edges"] = Value::Null;
            }
        }
        compare(&redone, &native, "circular redo");
        if !case.ends_with("internal_tangent") || case.starts_with("planarunion") {
            regular += 1;
        }
    }
    assert_eq!(regular, expected_regular);
}
