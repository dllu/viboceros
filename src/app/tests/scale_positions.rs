use super::*;
use viboceros_command::scale_positions::ScaleMode;

#[test]
fn scale_positions_native_modes_groups_grips_repetition_and_memory_match() {
    super::scale_nu::replay_scale_nu_options(
        include_str!("../../../tools/rhino_oracle/fixtures/scale_positions.json"),
        include_str!("../../../tools/rhino_oracle/observations/scale_positions.json"),
        57,
        "ScalePositions",
    );
}

#[test]
fn scale_positions_preview_keeps_grouped_shapes_independent_and_rejected_input_retryable() {
    let mut app = test_app();
    let enter = |app: &mut VibocerosApp, text: &str| {
        app.command_input = text.into();
        app.run_command();
    };
    enter(&mut app, "Line 2,0,0 4,0,0");
    enter(&mut app, "Line 8,0,0 10,0,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "Group");
    let ids = app.document.selected_object_ids().collect::<Vec<_>>();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ScalePositions");
    enter(&mut app, "Mode");
    enter(&mut app, "2D");
    enter(&mut app, "w0,0,0");
    assert_eq!(
        app.transform_default_hint().as_deref(),
        Some("Mode=2D; Enter accepts the default: 1")
    );
    enter(&mut app, "0");
    assert!(app.active_command.is_some());
    assert_eq!(
        app.commands.scale_mode_default("ScalePositions"),
        Some(ScaleMode::ThreeDimensional)
    );
    enter(&mut app, "w1,0,0");
    let preview = app.affine_preview().unwrap();
    let map = preview
        .definition
        .transform_at(
            preview.frame.unwrap(),
            point(2., 0., 0.),
            app.document.tolerance(),
        )
        .unwrap();
    let layout = preview.rigid_layout.unwrap();
    for (id, x) in ids.into_iter().zip([3., 9.]) {
        assert_eq!(layout.center(id), Some(point(x, 0., 0.)));
        let translation =
            viboceros_command::rigid_transform::rigid_map(layout.center(id).unwrap(), map).unwrap();
        let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
            panic!("expected line")
        };
        let posed = line
            .transformed(translation, app.document.tolerance())
            .unwrap();
        assert_eq!(posed.start(), point(line.start().x() + x, 0., 0.));
        assert_eq!(posed.length().unwrap(), line.length().unwrap());
    }
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "_Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.commands.scale_mode_default("ScalePositions"),
        Some(ScaleMode::ThreeDimensional)
    );
}
