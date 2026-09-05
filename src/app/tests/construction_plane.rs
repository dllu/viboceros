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

#[test]
fn front_view_transform_prompts_use_their_plane_and_undo_atomically() {
    for (inputs, expected) in [
        (vec!["Rotate", "0", "0,1", "1,0"], point(3.0, 2.0, -1.0)),
        (vec!["Mirror", "0", "0,1"], point(-1.0, 2.0, 3.0)),
        (vec!["Scale2D", "0", "0,1", "0,2"], point(2.0, 2.0, 6.0)),
        (vec!["Shear", "0", "0,1", "1,1"], point(4.0, 2.0, 3.0)),
        (
            vec!["ProjectToCPlane DeleteInput=Yes"],
            point(1.0, 0.0, 3.0),
        ),
    ] {
        let mut app = test_app();
        app.active_viewport = 2;
        enter(&mut app, "Point 1,2,3");
        enter(&mut app, "SelAll");
        let id = app.document.objects().next().unwrap().id();
        for input in inputs {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let Geometry::Point(actual) = app.document.object(id).unwrap().geometry() else {
            panic!("point")
        };
        assert!(
            actual.distance_to(expected).unwrap() < 1e-12,
            "{:?}",
            app.command_log
        );
        enter(&mut app, "Undo");
        assert_eq!(
            app.document.object(id).unwrap().geometry(),
            &Geometry::Point(point(1.0, 2.0, 3.0))
        );
        enter(&mut app, "Redo");
        assert!(app.drafting_plane.is_none());
    }
}

#[test]
fn normal_only_angle_and_mirror_references_remain_correctable_in_front_view() {
    for name in ["Rotate", "Mirror", "Shear"] {
        let mut app = test_app();
        app.active_viewport = 2;
        for input in ["Point 1,2,3", "SelAll", name, "0", "w0,5,0"] {
            enter(&mut app, input);
        }
        assert!(app.active_command.is_some(), "{name}");
        assert_eq!(app.command_input, "w0,5,0");
        assert_eq!(app.last_point, Some(point(0.0, 0.0, 0.0)));
        enter(&mut app, "0,1");
        if name != "Mirror" {
            enter(&mut app, "1,1");
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
    }
}

#[test]
fn scale2d_uses_full_reference_distances_and_the_finishing_viewport() {
    let mut app = test_app();
    app.active_viewport = 2;
    for input in ["Point 1,2,3", "SelAll", "Scale2D", "0", "0,1"] {
        enter(&mut app, input);
    }
    app.active_viewport = 0;
    enter(&mut app, "w0,0,2");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::Point(point(2.0, 4.0, 3.0))
    );
    assert!(app.drafting_plane.is_none());
}

#[test]
fn shear_accepts_a_normal_target_with_a_tilted_reference() {
    let mut app = test_app();
    for input in ["Point 3,4,7", "SelAll", "Shear", "0", "3,4,5", "0,0,5"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point")
    };
    assert!(
        p.distance_to(point(
            3.0 - 4.0 * 2.0_f64.sqrt(),
            4.0 + 3.0 * 2.0_f64.sqrt(),
            7.0
        ))
        .unwrap()
            < 1e-12
    );
}
