use super::*;
use serde_json::{Value, json};
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::{Brep, Circle3, MeshFace, NurbsSurface, Polyline3, WeightedPoint3};

fn enter(app: &mut VibocerosApp, value: &str) {
    app.command_input = value.into();
    app.run_command();
}
fn p(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn controls(value: &Value) -> Vec<WeightedPoint3> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| WeightedPoint3::try_new(p(&v["point"]), v["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn floats(value: &Value) -> Vec<f64> {
    serde_json::from_value(value.clone()).unwrap()
}
fn geometry(op: &Value, native: &Value) -> Geometry {
    let kind = op["source"].as_str().unwrap();
    if kind == "circle" || kind == "arc" {
        let circle = Circle3::try_new(
            point(0., 0., 0.),
            2.,
            viboceros_geometry::Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        return if kind == "circle" {
            Geometry::Circle(circle)
        } else {
            Geometry::Arc(
                viboceros_geometry::CircularArc3::try_from_circle_sweep(
                    circle,
                    std::f64::consts::PI,
                )
                .unwrap(),
            )
        };
    }
    if kind.starts_with("surface") {
        let s = &native["surface"];
        let surface = NurbsSurface::try_new_rational(
            s["degree"][0].as_u64().unwrap() as usize,
            s["degree"][1].as_u64().unwrap() as usize,
            s["control_count"][0].as_u64().unwrap() as usize,
            s["control_count"][1].as_u64().unwrap() as usize,
            controls(&s["control_points"]),
            floats(&s["knots_u"]),
            floats(&s["knots_v"]),
        )
        .unwrap();
        return Geometry::Brep(Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap());
    }
    if kind == "mesh" {
        let m = &native["mesh"];
        let faces = m["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let a = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                match *a.as_slice() {
                    [a, b, c] => MeshFace::Triangle([a, b, c]),
                    [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                    _ => unreachable!(),
                }
            })
            .collect();
        return Geometry::Mesh(
            TriangleMesh::try_new_faces(
                m["vertices"].as_array().unwrap().iter().map(p).collect(),
                faces,
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    let c = &native["curve"];
    let controls = controls(&c["control_points"]);
    if kind == "line" {
        return Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                controls[0].point(),
                controls[1].point(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    if kind == "polyline" {
        return Geometry::Polyline(
            Polyline3::try_new(
                controls.iter().map(|p| p.point()).collect(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        );
    }
    Geometry::NurbsCurve(
        NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            controls,
            floats(&c["knots"]),
        )
        .unwrap(),
    )
}
fn control_json(points: &[WeightedPoint3]) -> Value {
    json!(
        points
            .iter()
            .map(|p| json!({"point":p.point().to_array(), "weight":p.weight()}))
            .collect::<Vec<_>>()
    )
}
fn snapshot(app: &VibocerosApp, source: ObjectId, extra: Option<ObjectId>) -> Value {
    json!(app.document.objects().map(|o| {
        let locations = app.document.control_point_locations(o.id());
        let mut row = json!({"role":if o.id()==source {"source"} else if Some(o.id())==extra {"point"} else {"output"},
            "selected":app.document.is_selected(o.id()), "name":o.attributes().name(),
            "grips_on":locations.is_some(), "grips":app.document.control_points().filter(|(id,_,_)|id.object==o.id())
                .map(|(id,pick,selected)|json!({"index":id.index,"point":pick.to_array(),"selected":selected})).collect::<Vec<_>>()});
        match o.geometry() {
            Geometry::Brep(b) => {
                let s = b.faces()[0].surface(); row["kind"]=json!("Brep");
                row["surface"]=json!({"degree":[s.degree_u(),s.degree_v()], "control_count":[s.control_point_count_u(),s.control_point_count_v()],
                    "control_points":control_json(s.control_points()),"knots_u":s.knots_u(),"knots_v":s.knots_v(),
                    "domain_u":[*s.domain_u().start(),*s.domain_u().end()],"domain_v":[*s.domain_v().start(),*s.domain_v().end()]});
            }
            Geometry::Mesh(m) => { row["kind"]=json!("Mesh");row["mesh"]=json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                "faces":m.faces().iter().map(|f|f.indices().to_vec()).collect::<Vec<_>>()}); }
            Geometry::Point(p) => { row["kind"]=json!("Point");row["point"]=json!(p.to_array()); }
            geometry => {
                row["kind"]=json!(match geometry { Geometry::Line(_)=>"LineCurve", Geometry::Polyline(_)=>"PolylineCurve",
                    Geometry::Circle(_)|Geometry::Arc(_)=>"ArcCurve", _=>"NurbsCurve" });
                let c = geometry.nurbs_curve_representation().unwrap().unwrap();
                row["curve"]=json!({"degree":c.degree(),"control_points":control_json(c.control_points()),"knots":c.knots(),
                    "domain":[*c.domain().start(),*c.domain().end()]});
            }
        }
        row
    }).collect::<Vec<_>>())
}
fn compare(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-9,
            "{context}: {actual} vs {expected}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{context}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{context}"
            );
            for (k, b) in b {
                compare(&a[k], b, &format!("{context}/{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}

#[test]
fn grip_transform_replays_native_geometry_selection_display_copy_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/grip_transform.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/grip_transform.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 32);
    assert_eq!(r["results"].as_array().unwrap().len(), 32);
    let mut verified = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        for incremental in [false, true] {
            let mut app = test_app();
            let native = &row["value"];
            app.document
                .begin_transaction("Grip transform sources")
                .unwrap();
            let source = app
                .document
                .add_geometry(geometry(op, &native["before"][0]))
                .unwrap();
            app.document
                .set_object_names([(source, Some("grip source".into()))])
                .unwrap();
            let extra = if op["point"] == true {
                Some(
                    app.document
                        .add_geometry(Geometry::Point(point(8., 0., 0.)))
                        .unwrap(),
                )
            } else {
                None
            };
            app.document.commit_transaction().unwrap();
            app.document.enable_control_points([source]).unwrap();
            app.document
                .select_control_points(
                    op["selected"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| ControlPointId {
                            object: source,
                            index: i.as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
            if op["parent"] == true {
                app.document
                    .select_objects_direct([source], SelectionMode::Add)
                    .unwrap();
            }
            if let Some(id) = extra {
                app.document
                    .select_objects_direct([id], SelectionMode::Add)
                    .unwrap();
            }
            compare(
                &snapshot(&app, source, extra),
                &native["before"],
                op["id"].as_str().unwrap(),
            );
            let before = app.document.clone();
            let (name, arguments) = match op["command"].as_str().unwrap() {
                "move" => ("Move", vec!["0,0,0", "1,2,3"]),
                "zero" => ("Move", vec!["0,0,0", "0,0,0"]),
                "rotate" => ("Rotate", vec!["0,0,0", "30"]),
                "scale" => ("Scale", vec!["0,0,0", "2"]),
                "mirror" => ("Mirror", vec!["0,0,0", "0,1,0"]),
                "copy" => ("Copy", vec!["0,0,0", "1,2,3"]),
                _ => unreachable!(),
            };
            if incremental || op["inputs"] == "all" {
                enter(
                    &mut app,
                    &format!(
                        "{name}{}",
                        if matches!(name, "Rotate" | "Scale" | "Mirror") {
                            " Copy=No"
                        } else {
                            ""
                        }
                    ),
                );
                if op["inputs"] == "all" {
                    enter(&mut app, "SelAll");
                    enter(&mut app, "");
                }
                for arg in &arguments {
                    enter(&mut app, arg);
                }
                if name == "Copy" {
                    enter(&mut app, "");
                }
            } else {
                enter(
                    &mut app,
                    &format!(
                        "{name} {}{}",
                        arguments.join(" "),
                        if matches!(name, "Rotate" | "Scale" | "Mirror") {
                            " Copy=No"
                        } else {
                            ""
                        }
                    ),
                );
            }
            // Native retains a collapsed triangle in this mesh mirror case.
            // The current mesh kernel rejects collapsed faces; retain the
            // incompatibility rather than deleting the face or moving peers.
            if op["id"] == "grip-transform-13" {
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    before.objects().cloned().collect::<Vec<_>>()
                );
                assert!(
                    app.command_log
                        .iter()
                        .any(|line| line.starts_with("Error:"))
                );
                assert_eq!(app.document.undo_label(), Some("Grip transform sources"));
                continue;
            }
            compare(
                &snapshot(&app, source, extra),
                &native["after_script"],
                &format!("{} {incremental}: {:?}", op["id"], app.command_log),
            );
            assert!(app.active_command.is_none());
            if op["off"] == true {
                enter(&mut app, "PointsOff");
                compare(
                    &snapshot(&app, source, extra),
                    &native["points_off"],
                    "PointsOff",
                );
            }
            for phase in ["undo", "redo"] {
                enter(&mut app, phase);
                compare(
                    &snapshot(&app, source, extra),
                    &native[phase],
                    &format!("{} {incremental} {phase}", op["id"]),
                );
            }
            verified += 1;
        }
    }
    assert_eq!(verified, 62);
}

#[test]
fn repeated_grip_copies_freeze_sources_and_commit_one_history_entry() {
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    app.document
        .select_control_points(
            [ControlPointId {
                object: source,
                index: 0,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    for input in ["Copy", "0,0,0", "1,2,3", "4,5,6", ""] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let owners = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    assert_eq!(owners.len(), 3);
    assert_eq!(app.document.selected_control_points().count(), 3);
    for (i, id) in owners.into_iter().enumerate() {
        let Geometry::NurbsCurve(c) = app.document.object(id).unwrap().geometry() else {
            panic!("curve")
        };
        let expected = [point(2., 0., 0.), point(3., 2., 3.), point(6., 5., 6.)][i];
        assert_eq!(c.control_points()[0].point(), expected);
        assert_eq!(
            c.control_points()[1..]
                .iter()
                .map(|p| p.point())
                .collect::<Vec<_>>(),
            points[1..]
        );
    }
    assert_eq!(app.document.undo_label(), Some("Copy"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.selected_control_points().count(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.selected_control_points().count(), 3);
}

#[test]
fn command_first_move_accepts_multiple_mouse_grips_without_selecting_parent() {
    use super::circle_fit_grips::{button, frame};
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    enter(&mut app, "Move");
    assert!(app.object_prompt.is_some());
    assert!(app.control_point_picking_available());
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    for index in [0, 2] {
        let screen = app.viewports[0].project(points[index], rect).unwrap();
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(screen),
                button(screen, true, egui::Modifiers::NONE),
            ],
        );
        let (output, _) = frame(
            &mut app,
            &context,
            vec![button(screen, false, egui::Modifiers::NONE)],
        );
        assert!(output.selection_click.is_none());
        assert_eq!(
            output.control_point_selection.as_ref().unwrap().picks,
            [ControlPointId {
                object: source,
                index
            }]
        );
        assert!(app.handle_viewport_action(output));
    }
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.selected_control_points().count(), 2);
    for input in ["", "0,0,0", "1,2,3"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    let Geometry::NurbsCurve(c) = app.document.object(source).unwrap().geometry() else {
        panic!("curve")
    };
    assert_eq!(c.control_points()[0].point(), point(3., 2., 3.));
    assert_eq!(c.control_points()[1].point(), points[1]);
    assert_eq!(c.control_points()[2].point(), point(-1., 2., 3.));
    assert_eq!(c.control_points()[3].point(), points[3]);
    assert_eq!(app.document.selected_control_points().count(), 2);
}

#[test]
fn automatic_scale_center_uses_selected_grips_even_when_parent_selected() {
    let mut app = test_app();
    let points = [
        point(2., 0., 0.),
        point(0., 2., 0.),
        point(-2., 0., 0.),
        point(0., -2., 0.),
    ];
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(
            NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
        ))
        .unwrap();
    app.document.enable_control_points([source]).unwrap();
    app.document
        .select_control_points(
            [0, 1].map(|index| ControlPointId {
                object: source,
                index,
            }),
            SelectionMode::Add,
        )
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Add)
        .unwrap();
    for input in ["Scale Copy=No", "", "2"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let Geometry::NurbsCurve(c) = app.document.object(source).unwrap().geometry() else {
        panic!("curve")
    };
    assert_eq!(c.control_points()[0].point(), point(3., -1., 0.));
    assert_eq!(c.control_points()[1].point(), point(-1., 3., 0.));
    assert_eq!(c.control_points()[2].point(), points[2]);
    assert_eq!(c.control_points()[3].point(), points[3]);
}
