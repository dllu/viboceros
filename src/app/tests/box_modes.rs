use super::{
    circle_preview::{screen_point, viewport_frame},
    curve_fillet::enter,
    *,
};

#[test]
fn box_modes_match_rhino_solids_and_mesh_grids() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("box_modes/rhino.json")).unwrap();
    for case in cases {
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        let name = case["name"].as_str().unwrap();
        let command = case["command"].as_str().unwrap();
        let guide = if command.contains("AroundCurve") {
            enter(&mut app, "Line w0,0,0 w0,0,10");
            Some(app.document.objects().next().unwrap().id())
        } else {
            None
        };
        let input_count = usize::from(guide.is_some());
        enter(&mut app, command);
        assert!(
            app.rectangle_primitive_session.is_some(),
            "{name}: {:?}",
            app.command_log
        );
        let mut interpreted = command.to_owned();
        for input in case["inputs"].as_array().unwrap() {
            assert_eq!(app.document.objects().len(), input_count, "{name}");
            let input = input.as_str().unwrap();
            let input = if let Some(guide) = guide {
                input.replace("{guide}", &guide.to_string())
            } else {
                input.to_owned()
            };
            interpreted.push(' ');
            interpreted.push_str(&input);
            enter(&mut app, &input);
        }
        assert!(
            app.rectangle_primitive_session.is_none(),
            "{name}: {:?}",
            app.command_log
        );
        assert_eq!(app.document.objects().len(), input_count + 1, "{name}");
        check_geometry(
            app.document.objects().last().unwrap().geometry(),
            &case,
            name,
        );
        let undo = if command.starts_with("MeshBox") {
            "MeshBox"
        } else {
            "Box"
        };
        assert_eq!(app.document.undo().unwrap().as_deref(), Some(undo));
        assert_eq!(app.document.objects().len(), input_count);
        if let Some(guide) = guide {
            assert!(app.document.object(guide).is_some());
        }
        assert_eq!(app.document.redo().unwrap().as_deref(), Some(undo));
        check_geometry(
            app.document.objects().last().unwrap().geometry(),
            &case,
            name,
        );

        // Legacy three-point spelling and the staged rectangle arguments
        // also enter the public command registry as one line.
        enter(&mut app, &interpreted);
        assert_eq!(
            app.document.objects().len(),
            input_count + 2,
            "{name}: {:?}",
            app.command_log
        );
        check_geometry(
            app.document.objects().last().unwrap().geometry(),
            &case,
            name,
        );
    }
}

fn check_geometry(geometry: &Geometry, case: &serde_json::Value, name: &str) {
    let bounds = match geometry {
        Geometry::Brep(brep) => {
            assert!(brep.is_extrusion() && brep.is_solid(), "{name}");
            let expected: Vec<[f64; 3]> = serde_json::from_value(case["vertices"].clone()).unwrap();
            assert_eq!(brep.vertices().len(), expected.len(), "{name}");
            for point in expected {
                let point = Point3::try_from(point).unwrap();
                assert!(
                    brep.vertices()
                        .iter()
                        .any(|v| v.point().distance_to(point).unwrap() < 1e-8),
                    "{name}: missing corner {point:?}"
                );
            }
            brep.bounds()
        }
        Geometry::Mesh(mesh) => {
            let expected = &case["mesh"];
            assert!(mesh.topology().is_solid(), "{name}");
            assert_eq!(
                mesh.face_count(),
                expected["faces"].as_u64().unwrap() as usize,
                "{name}"
            );
            assert_eq!(
                mesh.vertices().len(),
                expected["vertices"].as_u64().unwrap() as usize,
                "{name}"
            );
            let points: Vec<(usize, [f64; 3])> =
                serde_json::from_value(expected["points"].clone()).unwrap();
            for (i, coordinates) in points {
                let point = Point3::try_from(coordinates).unwrap();
                assert!(
                    mesh.vertices()[i].distance_to(point).unwrap() < 1e-8,
                    "{name}: vertex {i} {:?} != {point:?}",
                    mesh.vertices()[i]
                );
            }
            let quads: Vec<(usize, [u32; 4])> =
                serde_json::from_value(expected["quads"].clone()).unwrap();
            for (i, corners) in quads {
                assert_eq!(
                    mesh.faces()[i],
                    viboceros_geometry::MeshFace::Quad(corners),
                    "{name}"
                );
            }
            mesh.bounds()
        }
        _ => panic!("{name}: expected a box"),
    };
    let expected: [[f64; 3]; 2] = serde_json::from_value(case["bounds"].clone()).unwrap();
    for (actual, point) in [bounds.min(), bounds.max()].into_iter().zip(expected) {
        let point = Point3::try_from(point).unwrap();
        assert!(
            actual.distance_to(point).unwrap() < 1e-8,
            "{name}: {actual:?} != {point:?}"
        );
    }
}

#[test]
fn box_mode_previews_follow_shift_clicks_and_signed_heights() {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "Box");
    enter(&mut app, "w0,0,0");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let cursor = screen_point(&app, point(6., 2., 0.));
    let free = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
    assert_eq!(free.len(), 4);
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let square = viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::ModifiersChanged(shift)],
    );
    assert!(free != square);
    for pressed in [true, false] {
        viewport_frame(
            &mut app,
            &context,
            vec![egui::Event::PointerButton {
                pos: cursor,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: shift,
            }],
        );
    }
    let base = app
        .rectangle_primitive_session
        .as_ref()
        .unwrap()
        .base
        .unwrap();
    let [length, width] = [1, 3].map(|i| base.corners[0].distance_to(base.corners[i]).unwrap());
    assert!((length - width).abs() < 1e-8);
    assert!((length - 6.).abs() < 1e-5);
    for height in [-3., 5.] {
        app.update_rectangle_primitive_hover(point(1., 1., height));
        let wires = app
            .rectangle_primitive_session
            .as_ref()
            .unwrap()
            .preview_curves();
        assert_eq!(wires.len(), 6);
        assert!(wires.iter().any(|curve| {
            curve
                .control_points()
                .iter()
                .any(|p| p.point().z() == height)
        }));
        assert_eq!(app.document.objects().len(), 0);
    }
    enter(&mut app, "-3");
    assert_eq!(app.document.undo_label(), Some("Box"));
    assert!(
        app.document
            .objects()
            .next()
            .unwrap()
            .geometry()
            .bounds()
            .unwrap()
            .min()
            .z()
            == -3.
    );
}
