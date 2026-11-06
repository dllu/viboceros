//! Reference lifetime, transparent CPlane and command-file controls.
use super::*;
fn enter(app: &mut VibocerosApp, s: &str) {
    app.command_input = s.into();
    app.run_command();
}
#[test]
fn changed_or_unavailable_perpendicular_references_require_a_fresh_pick() {
    for change in ["replace", "delete", "hide", "lock", "tolerance"] {
        let mut app = test_app();
        enter(&mut app, "Line 0,0,0 10,0,0");
        let id = app.document.objects().next().unwrap().id();
        app.document.clear_history().unwrap();
        enter(&mut app, "Line Perpendicular BothSides");
        assert!(app.pick_line_perpendicular_at(id, None, point(5., 0., 0.)));
        match change {
            "replace" => {
                app.document
                    .replace_object_geometries([(
                        id,
                        Geometry::Line(
                            viboceros_geometry::LineSegment::try_new(
                                point(0., 0., 0.),
                                point(20., 0., 0.),
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
        enter(&mut app, "w7,4,3");
        assert!(
            app.picking_line_perpendicular(),
            "{change}: {:?}",
            app.command_log
        );
        assert!(app.line_perpendicular.as_ref().unwrap().input.both_sides());
        assert_eq!(app.document.objects().len(), count);
        enter(&mut app, "!");
    }
}
#[test]
fn perpendicular_macros_units_and_transparent_planes_use_the_shared_getters() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_history().unwrap();
    let parent = app.document.object(id).unwrap().clone();
    for length in ["2+3", "5mm", "0.5cm"] {
        enter(&mut app, "Line Perpendicular");
        enter(&mut app, &format!("Curve={id} w5,0,0"));
        enter(&mut app, length);
        enter(&mut app, "");
        enter(&mut app, "CPlane 3Point");
        for s in ["w10,20,30", "w10,21,30", "w10,20,31"] {
            enter(&mut app, s);
        }
        enter(&mut app, "w7,4,3");
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Line(line) = app.document.objects().last().unwrap().geometry() else {
            panic!()
        };
        assert_eq!(line.start(), point(5., 0., 0.));
        assert!(line.end().distance_to(point(5., 4., 3.)).unwrap() < 1e-12);
        app.document.undo().unwrap();
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Perpendicular input.txt");
    std::fs::write(
        &path,
        format!(
            "_Line _Perpendicular PointOnCurve Curve={id} w5,0,0 w3,0,0 w7,4,3\n_Point r1,2,3\n"
        ),
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
    let Geometry::Line(line) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(line.start(), point(3., 0., 0.));
    assert_eq!(line.end(), point(3., 4., 3.));
    let Geometry::Point(p) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    // Relative coordinates follow the active YZ construction plane.
    assert_eq!(*p, point(6., 5., 5.));
    assert_eq!(app.document.object(id).unwrap(), &parent);
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    std::fs::write(
        &path,
        "_Line _Perpendicular Curve=999999 w5,0,0 w7,4,3\n_Point w99,99,99\n",
    )
    .unwrap();
    enter(&mut app, &format!("ReadCommandFile \"{}\"", path.display()));
    for _ in 0..100 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(app.last_command_file_result, Some(false));
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.object(id).unwrap(), &parent);
}
