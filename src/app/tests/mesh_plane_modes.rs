use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use serde_json::Value;
use viboceros_geometry::{Circle3, CircularArc3, LineSegment, MeshFace, Vector3};

#[test]
fn mesh_plane_modes_match_rhino_vertex_and_face_order() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("mesh_plane_modes/rhino.json")).unwrap();
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let mut app = test_app();
        app.active_viewport = 0;
        app.viewports[0] = Viewport::new(ViewKind::Top);
        let tolerance = app.document.tolerance();
        if let Some(plane) = row.get("plane") {
            let [origin, x, y]: [[f64; 3]; 3] = serde_json::from_value(plane.clone()).unwrap();
            let frame = Frame3::try_from_directions(
                Point3::try_from(origin).unwrap(),
                Vector3::try_from(x).unwrap(),
                Vector3::try_from(y).unwrap(),
                tolerance,
            )
            .unwrap();
            assert!(app.viewports[0].set_construction_plane(frame));
        }
        let source = match row["guide"].as_str() {
            Some("line") => Some(Geometry::Line(
                LineSegment::try_new(point(0., 0., 0.), point(0., 0., 10.), tolerance).unwrap(),
            )),
            Some("arc") => {
                let plane = app.viewports[0].construction_plane();
                let circle = Circle3::try_from_frame(
                    point(0., 0., 0.),
                    8.,
                    plane.x_axis(),
                    plane.z_axis(),
                    tolerance,
                )
                .unwrap();
                let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
                    .unwrap();
                Some(Geometry::Arc(arc))
            }
            _ => None,
        };
        let guide = source.map(|geometry| app.document.add_geometry(geometry).unwrap());
        let prior_count = usize::from(guide.is_some());
        let command = row["command"].as_str().unwrap();
        let mut script = command.to_owned();
        enter(&mut app, command);
        assert!(app.rectangle_primitive_session.is_some(), "{name}");
        for token in row["inputs"].as_array().unwrap() {
            assert_eq!(app.document.objects().len(), prior_count);
            let token = token.as_str().unwrap();
            let token = guide.map_or_else(
                || token.to_owned(),
                |id| token.replace("{guide}", &id.to_string()),
            );
            script.push(' ');
            script.push_str(&token);
            enter(&mut app, &token);
        }
        assert!(
            app.rectangle_primitive_session.is_none(),
            "{name}: {:?}",
            app.command_log
        );
        assert_eq!(app.document.objects().len(), prior_count + 1, "{name}");
        assert_matches_capture(
            app.document.objects().last().unwrap().geometry(),
            &row,
            name,
        );
        assert_eq!(app.document.selected_object_count(), 0, "{name}");
        assert_eq!(app.document.undo().unwrap().as_deref(), Some("MeshPlane"));
        assert_eq!(app.document.objects().len(), prior_count);
        assert_eq!(app.document.redo().unwrap().as_deref(), Some("MeshPlane"));
        assert_matches_capture(
            app.document.objects().last().unwrap().geometry(),
            &row,
            name,
        );

        enter(&mut app, &script);
        assert_eq!(
            app.document.objects().len(),
            prior_count + 2,
            "{name}: {:?}",
            app.command_log
        );
        assert_matches_capture(
            app.document.objects().last().unwrap().geometry(),
            &row,
            name,
        );
    }
}

fn assert_matches_capture(geometry: &Geometry, reference: &Value, name: &str) {
    let Geometry::Mesh(mesh) = geometry else {
        panic!("{name}: expected mesh");
    };
    let points: Vec<[f64; 3]> = serde_json::from_value(reference["vertices"].clone()).unwrap();
    let faces: Vec<[u32; 4]> = serde_json::from_value(reference["faces"].clone()).unwrap();
    assert_eq!(mesh.vertices().len(), points.len(), "{name}");
    for (i, (&actual, coordinates)) in mesh.vertices().iter().zip(points).enumerate() {
        let expected = Point3::try_from(coordinates).unwrap();
        assert!(
            actual.distance_to(expected).unwrap() < 1e-8,
            "{name}: vertex {i}: {actual:?} != {expected:?}"
        );
    }
    assert_eq!(
        mesh.faces(),
        faces.into_iter().map(MeshFace::Quad).collect::<Vec<_>>(),
        "{name}"
    );
}

#[test]
fn mesh_plane_previews_the_grid_and_accepts_a_shift_square() {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "MeshPlane XCount=3 YCount=2");
    enter(&mut app, "0");
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let a = screen_point(&app, point(6., 2., 0.));
    let b = screen_point(&app, point(4., 3., 0.));
    let wide = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(a)]);
    let tall = viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
    assert_eq!(wide.len(), 7);
    assert_eq!(tall.len(), 7);
    assert_ne!(wide, tall);
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let square = viewport_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::ModifiersChanged(shift),
            egui::Event::PointerMoved(a),
        ],
    );
    assert_eq!(square.len(), 7);
    assert_ne!(wide, square);
    assert!(!app.document.can_undo());
    for pressed in [true, false] {
        viewport_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: a,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: shift,
            }],
        );
    }
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a mesh plane");
    };
    assert_eq!(mesh.face_count(), 6);
    assert!(mesh.bounds().max().distance_to(point(6., 6., 0.)).unwrap() < 1e-5);
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("MeshPlane"));
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("MeshPlane"));
}

#[test]
fn mesh_plane_count_getter_retains_preferences_without_consuming_redo() {
    let mut app = test_app();
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Point 1,2,3");
    app.document.undo().unwrap();
    enter(&mut app, "MeshPlane");
    enter(&mut app, "XCount");
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, vec![]);
    let cursor = screen_point(&app, point(1., 1., 0.));
    viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
    viewport_click(&mut app, &ctx, cursor);
    let session = app.rectangle_primitive_session.as_ref().unwrap();
    assert!(session.rectangle.input.anchor().is_none());
    assert!(session.setting_count());
    enter(&mut app, "0");
    assert!(
        app.rectangle_primitive_session
            .as_ref()
            .unwrap()
            .setting_count()
    );
    enter(&mut app, "3");
    enter(&mut app, "YCount");
    enter(&mut app, "");
    assert!(
        !app.rectangle_primitive_session
            .as_ref()
            .unwrap()
            .setting_count()
    );
    assert_eq!(app.commands.mesh_plane_preferences().counts, [3, 10]);
    app.cancel_interactive_command(true);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "MeshPlane w0,0,0 w6,4,0");
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected one mesh plane");
    };
    assert_eq!(mesh.face_count(), 30);
}
