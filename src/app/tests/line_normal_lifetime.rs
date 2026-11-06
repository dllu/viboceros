//! Reference changes, numeric completion and command-file atomicity.
use super::*;
use viboceros_command::line::normal::Component;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn circle(app: &mut VibocerosApp) -> ObjectId {
    app.document
        .add_geometry(Geometry::Circle(
            viboceros_geometry::Circle3::try_new(
                point(0., 0., 0.),
                5.,
                viboceros_command::CommandContext::default()
                    .construction_plane
                    .z_axis(),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
}
#[test]
fn unavailable_or_replaced_sources_return_to_reference_picking_before_commit() {
    for change in ["replace", "delete", "hide", "lock", "tolerance"] {
        let mut app = test_app();
        let id = circle(&mut app);
        app.document.clear_history().unwrap();
        enter(&mut app, "Line Normal BothSides");
        assert!(app.pick_line_normal_at(id, Component::Object, point(5., 0., 0.)));
        enter(&mut app, "w3,4,0");
        assert!(app.line_input.unwrap().endpoint());
        match change {
            "replace" => {
                app.document
                    .replace_object_geometries([(
                        id,
                        Geometry::Line(
                            viboceros_geometry::LineSegment::try_new(
                                point(0., 0., 0.),
                                point(10., 0., 0.),
                                app.document.tolerance(),
                            )
                            .unwrap(),
                        ),
                    )])
                    .unwrap();
            }
            "delete" => {
                app.document.delete_object(id).unwrap();
            }
            "hide" => {
                app.document.set_objects_visibility([id], false).unwrap();
            }
            "lock" => {
                app.document.set_objects_locked([id], true).unwrap();
            }
            _ => {
                app.document.set_tolerance(
                    viboceros_geometry::Tolerance::try_new(0.01, 1e-9, 1e-6).unwrap(),
                );
            }
        }
        let count = app.document.objects().len();
        enter(&mut app, "w8,9,3");
        assert!(app.picking_line_normal(), "{change}: {:?}", app.command_log);
        assert!(app.line_input.unwrap().both_sides());
        assert!(app.line_input.unwrap().anchor().is_none());
        assert_eq!(app.document.objects().len(), count);
        enter(&mut app, "!");
        assert!(app.line_normal.is_none());
    }
}
#[test]
fn scalar_completion_updates_last_point_and_macros_use_the_projected_base() {
    let mut app = test_app();
    let id = circle(&mut app);
    let parent = app.document.object(id).unwrap().clone();
    app.document.clear_history().unwrap();
    for value in ["2+3", "5mm", "0.5cm"] {
        enter(&mut app, "Line Normal");
        enter(&mut app, &format!("Curve={id} w5,0,0"));
        enter(&mut app, "w3,4,0");
        enter(&mut app, value);
        assert!(
            app.active_command.is_none(),
            "{value}: {:?}",
            app.command_log
        );
        let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert!(line.end().distance_to(point(6., 8., 0.)).unwrap() < 1e-12);
        assert_eq!(app.last_point, Some(line.end()));
        assert!(app.line_accepted_point.is_none());
        app.document.undo().unwrap();
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Normal input.txt");
    std::fs::write(
        &path,
        format!("_Line _Normal Curve={id} w5,0,0 w4,4,3 w8,7,5\n_Point r1,2,3\n"),
    )
    .unwrap();
    enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
    for _ in 0..100 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(
        app.last_command_file_result,
        Some(true),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.undo_label(), Some("ReadCommandFile"));
    assert_eq!(app.document.objects().len(), 3);
    let Geometry::Line(line) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(p) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(
        line.start()
            .distance_to(point(5. / 2f64.sqrt(), 5. / 2f64.sqrt(), 0.))
            .unwrap()
            < 1e-12
    );
    assert!(
        p.distance_to(
            line.end()
                .translated(viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap())
                .unwrap()
        )
        .unwrap()
            < 1e-12
    );
    assert_eq!(app.document.object(id).unwrap(), &parent);
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
}
#[test]
fn transparent_construction_planes_leave_the_reference_and_normal_axis_intact() {
    let mut app = test_app();
    let id = circle(&mut app);
    app.document.clear_history().unwrap();
    enter(&mut app, "Line Normal");
    enter(&mut app, "CPlane 3Point");
    assert!(!app.picking_line_normal());
    for token in ["w0,0,0", "w1,0,0", "w0,1,0"] {
        enter(&mut app, token);
    }
    assert!(app.picking_line_normal());
    assert!(app.pick_line_normal_at(id, Component::Object, point(5., 0., 0.)));
    enter(&mut app, "w3,4,0");
    enter(&mut app, "CPlane 3Point");
    assert!(app.line_axis_constraint().is_none());
    for token in ["w10,20,30", "w10,21,30", "w10,20,31"] {
        enter(&mut app, token);
    }
    enter(&mut app, "5");
    assert!(app.active_command.is_none());
    let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(line.end().distance_to(point(6., 8., 0.)).unwrap() < 1e-12);
}
