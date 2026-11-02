//! Replay owned native cameras without exposing raw vectors to projection math.
use super::*;
use serde_json::Value;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn corpus() -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    (
        serde_json::from_slice(&std::fs::read(root.join("fixtures/camera_vectors.json")).unwrap())
            .unwrap(),
        serde_json::from_slice(
            &std::fs::read(root.join("observations/camera_vectors.json")).unwrap(),
        )
        .unwrap(),
    )
}
fn xyz(value: &Value) -> [f64; 3] {
    serde_json::from_value(value.clone()).unwrap()
}
fn camera(value: &Value) -> ThreeDmNamedView {
    ThreeDmNamedView {
        name: "Raw camera".into(),
        projection: match value["projection"].as_str().unwrap() {
            "Parallel" => ThreeDmProjection::Parallel,
            "Perspective" => ThreeDmProjection::Perspective,
            "TwoPointPerspective" => ThreeDmProjection::TwoPointPerspective,
            other => panic!("unknown projection {other}"),
        },
        camera_location: Point3::try_from(xyz(&value["location"])).unwrap(),
        camera_direction: Vector3::try_from(xyz(&value["direction"])).unwrap(),
        camera_up: Vector3::try_from(xyz(&value["up"])).unwrap(),
        target: Some(Point3::try_from(xyz(&value["target"])).unwrap()),
        construction_plane: WorldPlane::Top.frame(),
        frustum: serde_json::from_value(value["frustum"].clone()).unwrap(),
        screen_port: [0, 800, 600, 0],
    }
}
fn close(actual: [f64; 3], expected: [f64; 3]) {
    let scale = expected
        .into_iter()
        .map(f64::abs)
        .fold(0., f64::max)
        .max(1e-300);
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() <= 2e-12 * scale, "{actual:?} vs {expected:?}");
    }
}
fn exported(view: &Viewport) -> ThreeDmNamedView {
    Viewport::named_view_to_3dm(view.named_view_snapshot(), "Raw camera".into()).unwrap()
}
fn check_frame(view: &Viewport, native: &Value) {
    let frame = view.construction_plane_aligned_to_view().unwrap();
    close(
        frame.x_axis().as_vector().to_array(),
        xyz(&native["frame"][0]),
    );
    close(
        frame.y_axis().as_vector().to_array(),
        xyz(&native["frame"][1]),
    );
    close(
        view.viewward_direction().to_array(),
        xyz(&native["frame"][2]),
    );
}
fn check_picking(view: &Viewport) {
    let rect = view.last_rect.unwrap();
    let plane = view.construction_plane_aligned_to_view().unwrap();
    for local in [[0., 0., 0.], [0.1, -0.2, 0.], [-0.3, 0.1, -0.5]] {
        let p = plane.point_at(local).unwrap();
        let screen = view.project_precise(p, rect).unwrap();
        let pick = view.pick_projection().unwrap().continuous_point(p).unwrap();
        assert!((pick.x() - screen[0]).abs() < 1e-8);
        assert!((pick.y() - (f64::from(rect.height()) - screen[1])).abs() < 1e-8);
    }
}

#[test]
fn camera_vectors_preserve_raw_metadata_and_native_rendering_frames() {
    let (q, n) = corpus();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        if op["op"] != "camera_vectors" {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let native = &row["value"];
        let source = camera(&native["before"]);
        assert_eq!(source.camera_direction.to_array(), xyz(&op["direction"]));
        assert_eq!(source.camera_up.to_array(), xyz(&op["up"]));
        let mut view = Viewport::new(ViewKind::Top);
        view.restore_file_camera(&source).unwrap();
        let saved = view.named_view_snapshot();
        let out = exported(&view);
        assert_eq!(
            out.camera_direction.to_array(),
            xyz(&native["roundtrip"]["direction"])
        );
        assert_eq!(out.camera_up.to_array(), xyz(&native["roundtrip"]["up"]));
        check_frame(&view, &native["before"]);
        check_picking(&view);
        match op["action"].as_str().unwrap() {
            "None" => (),
            "Zoom" | "MouseZoom" => view.zoom_by(2., None, view.last_rect.unwrap()),
            "MousePan" => view.apply_navigation_drag(
                PointerButton::Middle,
                egui::Modifiers::NONE,
                Vec2::new(12., -7.),
            ),
            "WorldTop" => view.set_world_view(ViewKind::Top).unwrap(),
            "Plan" => view.set_plan_view().unwrap(),
            "PhysicalOrbit" => {
                let delta: [f32; 2] = serde_json::from_value(op["mouse_drag"].clone()).unwrap();
                view.apply_navigation_drag(
                    PointerButton::Secondary,
                    egui::Modifiers::NONE,
                    Vec2::from(delta),
                );
                check_frame(&view, &native["after"]);
                check_picking(&view);
            }
            other => panic!("unhandled camera action {other}"),
        }
        let after = exported(&view);
        close(
            after.camera_direction.to_array(),
            xyz(&native["after"]["direction"]),
        );
        close(after.camera_up.to_array(), xyz(&native["after"]["up"]));
        // View history owns raw vectors along with the rest of the camera.
        view.restore_named_view(saved);
        view.set_world_view(ViewKind::Right).unwrap();
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), saved.camera);
        assert!(view.redo_view());
        assert_eq!(exported(&view).camera_direction.to_array(), [-1., 0., 0.]);
        count += 1;
    }
    assert_eq!(count, 31);
}

#[test]
fn camera_vectors_match_native_orbits_through_multiple_egui_frames() {
    let (q, n) = corpus();
    let document = Document::default();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        if op["action"] != "PhysicalOrbit" {
            continue;
        }
        let context = egui::Context::default();
        let mut view = Viewport::new(ViewKind::Top);
        view.restore_file_camera(&camera(&row["value"]["before"]))
            .unwrap();
        viewport_frame(&context, &mut view, &document, Vec::new());
        let initial = view.camera_snapshot();
        let start = Pos2::new(300., 200.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        viewport_frame(
            &context,
            &mut view,
            &document,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        for delta in [Vec2::new(4., -2.), Vec2::new(8., -4.), Vec2::new(12., -7.)] {
            viewport_frame(
                &context,
                &mut view,
                &document,
                vec![egui::Event::PointerMoved(start + delta)],
            );
        }
        viewport_frame(
            &context,
            &mut view,
            &document,
            vec![button(start + Vec2::new(12., -7.), false)],
        );
        let native = &row["value"]["after"];
        check_frame(&view, native);
        close(
            exported(&view).camera_direction.to_array(),
            xyz(&native["direction"]),
        );
        close(exported(&view).camera_up.to_array(), xyz(&native["up"]));
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), initial);
        assert!(view.redo_view());
        check_frame(&view, native);
        count += 1;
    }
    assert_eq!(count, 4);
}
