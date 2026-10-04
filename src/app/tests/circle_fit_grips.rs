use super::*;
use serde_json::{Value, json};
use viboceros_document::{ControlPointId, SelectionMode};
use viboceros_geometry::{MeshFace, NurbsSurface, WeightedPoint3};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn read_point(p: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap()).unwrap()
}
fn source(op: &Value) -> Geometry {
    let controls = op["controls"]
        .as_array()
        .unwrap()
        .iter()
        .map(read_point)
        .collect::<Vec<_>>();
    match op["source"].as_str().unwrap() {
        "curve" => {
            let base = NurbsCurve::try_clamped_uniform(2, controls.clone()).unwrap();
            let weights = op["weights"].as_array().unwrap();
            Geometry::NurbsCurve(
                NurbsCurve::try_new_rational(
                    2,
                    controls
                        .into_iter()
                        .enumerate()
                        .map(|(i, p)| {
                            WeightedPoint3::try_new(
                                p,
                                weights.get(i).and_then(Value::as_f64).unwrap_or(1.),
                            )
                            .unwrap()
                        })
                        .collect(),
                    base.knots().to_vec(),
                )
                .unwrap(),
            )
        }
        "surface" => Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![controls[0], controls[2], controls[1], controls[3]],
                vec![0., 0., 1., 1.],
                vec![0., 0., 1., 1.],
            )
            .unwrap(),
        ),
        "mesh" => {
            let mut faces = vec![MeshFace::Quad([0, 1, 2, 3])];
            if controls.len() == 5 {
                faces.push(MeshFace::Triangle([4, 2, 3]));
            }
            Geometry::Mesh(
                TriangleMesh::try_new_faces(controls, faces, Tolerance::DEFAULT).unwrap(),
            )
        }
        _ => unreachable!(),
    }
}
fn roles(mut rows: Value) -> Value {
    for row in rows.as_array_mut().unwrap() {
        let row = row.as_object_mut().unwrap();
        for key in ["definition", "vertices", "circle", "seam"] {
            row.remove(key);
        }
    }
    rows
}
fn snapshot(app: &VibocerosApp, parent: ObjectId, points: &[ObjectId], kind: &Value) -> Value {
    Value::Array(app.document.objects().map(|object| {
        let mut row = json!({"selected":app.document.is_selected(object.id())});
        if object.id() == parent {
            let locations = app.document.control_point_locations(parent);
            row["kind"] = kind.clone();
            row["grips_on"] = json!(locations.is_some());
            row["grips"] = json!(app.document.control_points().filter(|(id, _, _)| id.object == parent)
                .map(|(id, point, selected)| json!({"index":id.index,"point":point.to_array(),"selected":selected})).collect::<Vec<_>>());
        } else if let Some(i) = points.iter().position(|&id| id == object.id()) {
            let Geometry::Point(point) = object.geometry() else { panic!("point source changed") };
            row["kind"] = json!("point"); row["source"] = json!(i); row["point"] = json!(point.to_array());
        } else { assert!(matches!(object.geometry(), Geometry::Circle(_))); row["kind"] = json!("circle"); }
        row
    }).collect())
}

#[test]
fn circle_fit_grips_replay_sixteen_native_input_and_history_workflows() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/circle_fit_grips.json"
    ))
    .unwrap();
    let r: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/circle_fit_grips.json"
    ))
    .unwrap();
    assert_eq!(q["operations"].as_array().unwrap().len(), 16);
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        for incremental in [false, true] {
            let mut app = test_app();
            app.document
                .begin_transaction("Circle grip sources")
                .unwrap();
            let parent = app.document.add_geometry(source(op)).unwrap();
            let points = op["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    app.document
                        .add_geometry(Geometry::Point(read_point(p)))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            app.document.commit_transaction().unwrap();
            let sources = app.document.objects().cloned().collect::<Vec<_>>();
            app.document
                .select_objects_direct([parent], SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "PointsOn");
            assert!(app.object_prompt.is_none());
            if op["op"] == "circle_fit_grips_commands" {
                enter(&mut app, "SelAll");
            }
            app.document
                .select_control_points(
                    op["selected"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| ControlPointId {
                            object: parent,
                            index: i.as_u64().unwrap() as usize,
                        }),
                    SelectionMode::Add,
                )
                .unwrap();
            app.document
                .select_objects_direct(
                    op["selected_points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| points[i.as_u64().unwrap() as usize]),
                    SelectionMode::Add,
                )
                .unwrap();
            let native = &row["value"];
            assert_eq!(
                snapshot(&app, parent, &points, &op["source"]),
                roles(native["before"].clone()),
                "{}",
                op["id"]
            );
            if incremental {
                enter(&mut app, "Circle");
                enter(&mut app, "_FitPoints");
            } else {
                enter(&mut app, "_C _FitPoints");
            }
            if app.object_prompt.is_some() {
                assert_eq!(app.document.selected_control_points().count(), 0);
                if op["inputs"] == "all" {
                    enter(&mut app, "SelAll");
                }
                if op["inputs"] != "auto" {
                    enter(&mut app, "");
                }
            }
            if app.object_prompt.is_some() {
                app.cancel_interactive_command(true);
            }
            assert_eq!(
                snapshot(&app, parent, &points, &op["source"]),
                roles(native["after_script"].clone()),
                "{} {incremental}: {:?}",
                op["id"],
                app.command_log
            );
            assert_eq!(
                app.document
                    .objects()
                    .take(sources.len())
                    .cloned()
                    .collect::<Vec<_>>(),
                sources
            );
            if let Some(circle) = native["after"]
                .as_array()
                .unwrap()
                .iter()
                .find(|o| o["kind"] == "circle")
            {
                let Geometry::Circle(actual) = app.document.objects().last().unwrap().geometry()
                else {
                    panic!("missing circle")
                };
                assert!(
                    actual
                        .center()
                        .distance_to(read_point(&circle["circle"]["origin"]))
                        .unwrap()
                        + (actual.radius() - circle["circle"]["radius"].as_f64().unwrap()).abs()
                        < 1e-7
                );
            }
            if op["off"] == true {
                enter(&mut app, "PointsOff");
                assert_eq!(
                    snapshot(&app, parent, &points, &op["source"]),
                    roles(native["points_off"].clone())
                );
            }
            for phase in ["undo", "redo"] {
                enter(&mut app, phase);
                assert_eq!(
                    snapshot(&app, parent, &points, &op["source"]),
                    roles(native[phase].clone()),
                    "{} {incremental} {phase}",
                    op["id"]
                );
            }
        }
    }
}

pub(super) fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                rect = ui.available_rect_before_wrap();
                let object_filter = app.viewport_object_filter();
                let control_point_pick = app.control_point_picking_available();
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        object_filter,
                        control_point_pick,
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    (output, rect)
}
pub(super) fn button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers,
    }
}

#[test]
fn circle_fit_grips_mouse_clicks_fit_without_selecting_the_parent_curve() {
    let mut app = test_app();
    enter(&mut app, "ControlPointCurve 2 2,0,0 0,2,0 -2,0,0");
    let parent = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([parent], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "PointsOn");
    enter(&mut app, "Circle FitPoints");
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    let controls = app
        .document
        .control_point_locations(parent)
        .unwrap()
        .to_vec();
    for (index, point) in controls.into_iter().enumerate() {
        let screen = app.viewports[0].project(point, rect).unwrap();
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
                object: parent,
                index
            }]
        );
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.document.selected_control_points().count(), index + 1);
        assert!(!app.document.is_selected(parent));
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    let Geometry::Circle(circle) = app.document.objects().last().unwrap().geometry() else {
        panic!("missing fit")
    };
    assert!((circle.radius() - 2.).abs() < 1e-12);
}

#[test]
fn circle_fit_grips_mouse_window_accepts_mixed_grips_and_point_objects() {
    let mut app = test_app();
    enter(&mut app, "ControlPointCurve 2 2,0,0 0,2,0 -2,0,0");
    let parent = app.document.objects().next().unwrap().id();
    app.document
        .select_objects_direct([parent], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "PointsOn");
    enter(&mut app, "Point 0,-2,0");
    enter(&mut app, "Circle FitPoints");
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    let mut window = egui::Rect::NOTHING;
    for point in [
        point(-2., 0., 0.),
        point(2., 0., 0.),
        point(0., -2., 0.),
        point(0., 2., 0.),
    ] {
        window.extend_with(app.viewports[0].project(point, rect).unwrap());
    }
    let window = window.expand(12.);
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::PointerMoved(window.min),
            button(window.min, true, egui::Modifiers::NONE),
        ],
    );
    frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(window.max)],
    );
    let (output, _) = frame(
        &mut app,
        &context,
        vec![button(window.max, false, egui::Modifiers::NONE)],
    );
    assert_eq!(
        output.control_point_selection.as_ref().unwrap().picks.len(),
        3
    );
    assert_eq!(
        output.selection_window.as_ref().unwrap().object_ids.len(),
        1
    );
    app.handle_viewport_action(output);
    assert_eq!(app.document.selected_control_points().count(), 3);
    assert_eq!(app.document.selected_object_count(), 1);
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().len(), 3);
}
