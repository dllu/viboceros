use super::*;

fn enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

#[test]
fn polar_array_center_pick_uses_the_current_plane_and_one_model_undo_step() {
    let mut app = test_app();
    for command in [
        "Line 2,0,0 4,1,3",
        "SelAll",
        "ArrayPolar 3 180 ZOffset=2",
        "CPlane World Front",
        "w0,0,0",
    ] {
        enter(&mut app, command);
    }
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.viewports[0].kind(), ViewKind::Top);
    let frame = app.viewports[0].construction_plane();
    for (object, (start, end)) in app.document.objects().zip([
        (point(2., 0., 0.), point(4., 1., 3.)),
        (point(0., -2., 2.), point(-3., -1., 4.)),
        (point(-2., -4., 0.), point(-4., -3., -3.)),
    ]) {
        let Geometry::Line(line) = object.geometry() else {
            panic!("line")
        };
        assert!(line.start().distance_to(start).unwrap() < 1e-12);
        assert!(line.end().distance_to(end).unwrap() < 1e-12);
    }
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.viewports[0].construction_plane(), frame);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn rectangular_arrays_respect_custom_planes_and_linear_arrays_keep_picked_world_vectors() {
    let mut app = test_app();
    for command in [
        "Point 1,2,3",
        "SelAll",
        "CPlane World Right",
        "Array 2 1 2 4 0 6",
    ] {
        enter(&mut app, command);
    }
    let points = app
        .document
        .objects()
        .map(|o| match o.geometry() {
            Geometry::Point(p) => *p,
            _ => panic!("point"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        points,
        [
            point(1., 2., 3.),
            point(1., 6., 3.),
            point(7., 2., 3.),
            point(7., 6., 3.)
        ]
    );
    enter(&mut app, "Undo");
    for command in ["ArrayLinear 3", "w0,0,0", "CPlane World Front", "w4,5,6"] {
        enter(&mut app, command);
    }
    let points = app
        .document
        .objects()
        .map(|o| match o.geometry() {
            Geometry::Point(p) => *p,
            _ => panic!("point"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        points,
        [point(1., 2., 3.), point(5., 7., 9.), point(9., 12., 15.)]
    );
}

#[test]
fn picked_array_corners_use_the_first_plane_even_after_a_nested_plane_change() {
    use viboceros_command::construction_plane::WorldPlane;
    let oblique = Frame3::try_from_directions(
        point(5., 7., 11.),
        viboceros_geometry::Vector3::try_new(1., 2., 3.).unwrap(),
        viboceros_geometry::Vector3::try_new(-4., 8., 1.).unwrap(),
        Default::default(),
    )
    .unwrap();
    for frame in WorldPlane::ALL
        .map(WorldPlane::frame)
        .into_iter()
        .chain([oblique])
    {
        for fill in [false, true] {
            let mut app = test_app();
            enter(&mut app, "Point 1,2,3");
            enter(&mut app, "SelAll");
            app.viewports[0].plane.set(frame);
            enter(
                &mut app,
                if fill {
                    "Array 3 2 2 Mode=Fill ZDistance=5"
                } else {
                    "Array 3 2 2 ZDistance=5"
                },
            );
            let first = point(5., 7., 11.);
            assert!(app.accept_drafting_point(first));
            enter(&mut app, "CPlane World Front");
            let second = frame.with_origin(first).point_at([6., -4., 99.]).unwrap();
            assert!(app.accept_drafting_point(second));
            assert!(app.active_command.is_none(), "{:?}", app.command_log);
            assert_eq!(app.document.objects().len(), 12);
            let mut expected = Vec::new();
            for z in 0..2 {
                for y in 0..2 {
                    for x in 0..3 {
                        expected.push(
                            point(1., 2., 3.)
                                .translated(
                                    frame
                                        .vector_at([
                                            if fill {
                                                3. * f64::from(x)
                                            } else {
                                                6. * f64::from(x)
                                            },
                                            -4. * f64::from(y),
                                            5. * f64::from(z),
                                        ])
                                        .unwrap(),
                                )
                                .unwrap(),
                        );
                    }
                }
            }
            for (object, expected) in app.document.objects().zip(expected) {
                let Geometry::Point(actual) = object.geometry() else {
                    panic!("point")
                };
                assert!(actual.distance_to(expected).unwrap() < 1e-11);
            }
            enter(&mut app, "Undo");
            assert_eq!(app.document.objects().len(), 1);
        }
    }
}
