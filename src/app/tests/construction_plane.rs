use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn front_view_circle_uses_the_front_plane() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Circle", "0", "0,5"] {
        enter(&mut app, input);
    }
    let Geometry::Circle(circle) = app
        .document
        .objects()
        .next()
        .expect("front circle")
        .geometry()
    else {
        panic!("circle");
    };
    assert_eq!(circle.radius(), 5.0);
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, -1.0, 0.0]
    );
}

#[test]
fn right_view_rectangle_uses_yz_width_and_height() {
    let mut app = test_app();
    app.active_viewport = 3;
    for input in ["Rectangle", "0", "4,3"] {
        enter(&mut app, input);
    }
    let Geometry::Polyline(rectangle) = app
        .document
        .objects()
        .next()
        .expect("right rectangle")
        .geometry()
    else {
        panic!("rectangle");
    };
    assert_eq!(
        rectangle.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(0.0, 4.0, 0.0),
            point(0.0, 4.0, 3.0),
            point(0.0, 0.0, 3.0),
            point(0.0, 0.0, 0.0)
        ]
    );
}

#[test]
fn first_accepted_pick_latches_the_plane_until_completion() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    app.active_viewport = 2;
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "w0,0,5");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, -1.0, 0.0]
    );
    assert_eq!(circle.radius(), 5.0);
    assert!(app.drafting_plane.is_none());
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
}

#[test]
fn cancellation_and_replacement_do_not_leak_a_previous_plane() {
    let mut app = test_app();
    app.active_viewport = 2;
    enter(&mut app, "Circle");
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "Circle 0,0,0 5");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle")
    };
    assert_eq!(
        circle.normal().unwrap().as_vector().to_array(),
        [0.0, 0.0, 1.0]
    );
    enter(&mut app, "Rectangle");
    enter(&mut app, "0");
    app.cancel_interactive_command(true);
    assert!(app.drafting_plane.is_none());
}

#[test]
fn zero_radius_and_zero_width_picks_remain_correctable() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Circle", "0", "w0,0,0"] {
        enter(&mut app, input);
    }
    assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
    assert_eq!(app.command_input, "w0,0,0");
    enter(&mut app, "w0,0,5");
    assert_eq!(app.document.objects().len(), 1);
    for input in ["Rectangle", "0", "0,5"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_some());
    enter(&mut app, "4,5");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn boxes_use_plane_height_and_one_undo_transaction_in_every_view() {
    for viewport in 0..4 {
        for name in ["Box", "MeshBox XCount=2 YCount=3 ZCount=2"] {
            let mut app = test_app();
            app.active_viewport = viewport;
            for input in [name, "1,2,3", "5,7,3", "5,7,-3"] {
                enter(&mut app, input);
            }
            assert!(
                app.active_command.is_none(),
                "{name}: {:?}",
                app.command_log
            );
            let object = app.document.objects().next().unwrap();
            match object.geometry() {
                Geometry::Brep(brep) => {
                    assert!(brep.is_solid());
                    assert!(
                        (brep.signed_volume(app.document.tolerance()).unwrap() - 120.0).abs()
                            < 1e-9
                    );
                }
                Geometry::Mesh(mesh) => assert!(mesh.topology().is_solid()),
                _ => panic!("box"),
            }
            assert!(app.drafting_plane.is_none());
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 0);
            enter(&mut app, "Redo");
            assert_eq!(app.document.objects().len(), 1);
        }
    }
}
