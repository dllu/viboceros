use super::boolean_intersection::enter;
use super::*;
use viboceros_geometry::{Brep, NurbsSurface, Point3, Tolerance};
fn pair() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0 4,0 4,4 0,4");
    enter(&mut app, "SrfPt 2,1 5,1 5,3 2,3");
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    (app, [ids[0], ids[1]])
}
#[test]
fn planar_difference_and_intersection_pick_ordered_surfaces_and_finish_on_second_pick() {
    for (name, area) in [("PlanarDifference", 12.), ("PlanarIntersection", 4.)] {
        let (mut app, ids) = pair();
        enter(&mut app, name);
        assert!(!app.command_line_idle());
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[0]),
            mode: SelectionMode::Replace,
        });
        assert!(!app.document.can_undo());
        app.apply_selection_click(SelectionClick {
            object_id: Some(ids[1]),
            mode: SelectionMode::Replace,
        });
        assert!(app.planar_boolean_prompt.is_none());
        let Geometry::Brep(b) = app.document.objects().next().unwrap().geometry() else {
            panic!()
        };
        assert!((b.area(app.document.tolerance()).unwrap() - area).abs() < 1e-9);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 1);
    }
}
#[test]
fn planar_union_set_getter_and_cancel_leave_sources_intact() {
    let (mut app, ids) = pair();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "PlanarUnion");
    for id in ids {
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Add,
        });
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "PlanarDifference");
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn planar_preselection_uses_native_command_admission_and_empty_results() {
    for name in ["PlanarUnion", "PlanarDifference", "PlanarIntersection"] {
        let (mut app, ids) = pair();
        app.document
            .select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        enter(&mut app, name);
        if name == "PlanarDifference" {
            assert!(app.planar_boolean_prompt.is_some());
            assert_eq!(app.document.objects().len(), 2);
            for id in ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
            }
        } else {
            assert!(app.planar_boolean_prompt.is_none());
        }
        assert_eq!(
            app.document.objects().len(),
            1,
            "{name}: {:?}",
            app.command_log
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(
            app.document.selected_object_count(),
            if name == "PlanarDifference" { 0 } else { 2 }
        );
        enter(&mut app, "Redo");
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn planar_application_replays_all_native_recipes_and_history() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_command.json"
    ))
    .unwrap();
    replay(&q);
}

#[test]
fn planar_application_replays_trim_holes_and_nonparallel_projection() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_topology.json"
    ))
    .unwrap();
    replay(&q);
}

#[test]
fn planar_application_replays_mixed_native_scale_recipes() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_scale.json"
    ))
    .unwrap();
    replay(&q);
}

fn replay(q: &serde_json::Value) {
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        let name = v["command_name"].as_str().unwrap();
        let mut app = test_app();
        let scale = v["scale"].as_f64().unwrap_or(1.);
        if v["scale"].is_number() {
            app.document.set_tolerance(
                Tolerance::try_new(
                    1e-7 * scale,
                    app.document.tolerance().relative(),
                    app.document.tolerance().angular(),
                )
                .unwrap(),
            );
        }
        for shape in v["shapes"].as_array().unwrap() {
            app.document
                .add_geometry(Geometry::Brep(shape_brep(shape, app.document.tolerance())))
                .unwrap();
        }
        let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        if v["pre"] == true {
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, name);
        if v["pre"] != true || name == "PlanarDifference" {
            for id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(*id),
                    mode: SelectionMode::Add,
                });
            }
            if name == "PlanarUnion" {
                enter(&mut app, "");
            }
        }
        assert!(app.planar_boolean_prompt.is_none());
        assert!(
            app.object_prompt.is_none(),
            "{} {:?}",
            v["case"],
            app.command_log
        );
        let native = v["command"]["after_script"].as_array().unwrap();
        assert_eq!(app.document.objects().len(), native.len());
        for (o, n) in app.document.objects().zip(native) {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            assert_eq!(b.faces().len(), n["faces"].as_u64().unwrap() as usize);
            if v["case"] == "planarintersection_internal_tangent" {
                assert_eq!(b.edges().len(), 1);
                assert_eq!(n["edges"], 3);
            } else {
                assert_eq!(b.edges().len(), n["edges"].as_u64().unwrap() as usize);
            }
            assert!(
                (b.area(app.document.tolerance()).unwrap() - n["area"].as_f64().unwrap()).abs()
                    / scale.powi(2)
                    < if v["scale"].is_number() {
                        3e-4
                    } else if ["disk", "annulus", "half_disk"]
                        .iter()
                        .any(|kind| v["shapes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|s| s["kind"] == *kind))
                    {
                        2e-5
                    } else {
                        1e-9
                    }
            );
        }
        assert!(app.document.can_undo());
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(
            app.document.selected_object_count(),
            v["undo"]["after_script"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|o| o["selected"] == true)
                .count()
        );
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), native.len());
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

fn shape_brep(shape: &serde_json::Value, tolerance: Tolerance) -> Brep {
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
            let c = viboceros_geometry::Circle3::try_new(
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
            let b = Brep::try_planar_face(&c.to_nurbs().unwrap(), tolerance).unwrap();
            if shape["reverse"] == true {
                b.reversed()
            } else {
                b
            }
        }
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

#[test]
fn planar_application_replays_circular_regions_and_retains_contact_diagnostics() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_circular.json"
    ))
    .unwrap();
    replay(&q);
}

#[test]
fn planar_application_replays_mixed_lines_arcs_and_holes() {
    let q: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/planar_boolean_mixed.json"
    ))
    .unwrap();
    replay(&q);
}
