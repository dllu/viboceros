use super::*;
use crate::app::tests::circle_preview::{viewport_click, viewport_frame};
use crate::app::tests::radius::move_measurement_pointer;
use std::time::{Duration, Instant};

fn add_reference_ellipse(app: &mut VibocerosApp) -> ObjectId {
    let axes = viboceros_command::CommandContext::default().construction_plane;
    let ellipse = viboceros_geometry::Ellipse3::try_new(
        point(20., 10., 0.),
        4.,
        2.,
        axes.x_axis(),
        axes.y_axis(),
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .add_geometry(Geometry::Ellipse(ellipse))
        .unwrap()
}

fn choose_edge(app: &mut VibocerosApp, pick: ComponentPick) {
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: egui::Modifiers::NONE,
        preselection: false,
        crossing: false,
        inverted: false,
    });
}

#[test]
fn edge_fillet_from_curve_accepts_typed_points_and_rejects_zero_curvature() {
    for (command, size) in [
        ("FilletEdge", "Radius"),
        ("ChamferEdge", "Distance"),
        ("BlendEdge", "Radius"),
    ] {
        let mut app = test_app();
        let pick = box_pick(&mut app);
        add_reference_ellipse(&mut app);
        submit(&mut app, "Line w17,16,0 w23,16,0");
        app.document.clear_history().unwrap();
        let last_point = point(3., 5., 7.);
        app.last_point = Some(last_point);
        submit(&mut app, &format!("{command} 2 _FromCurve"));
        assert!(app.measuring_edge_blend_radius(), "{:?}", app.command_log);
        submit(&mut app, "w20,16,0");
        assert!(app.measuring_edge_blend_radius());
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("finite positive radius")
        );
        app.cancel_current_prompt_or_selection();
        assert!(!app.measuring_edge_blend_radius());
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains(&format!("{size}=2"))
        );

        choose_edge(&mut app, pick);
        submit(&mut app, "FromCurve");
        submit(&mut app, "w24,10,0");
        assert!(!app.measuring_edge_blend_radius());
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains(&format!("{size}=1"))
        );
        assert_eq!(app.last_point, Some(last_point));
        assert_eq!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap(),
            [pick]
        );
        assert!(!app.document.can_undo());
        submit(&mut app, "");
        assert_eq!(app.document.undo_label(), Some(command));
        assert_eq!(app.document.objects().count(), 3);
    }
}

#[test]
fn edge_fillet_from_curve_hover_preview_matches_the_committed_chamfer() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    let pick = box_pick(&mut app);
    let reference = add_reference_ellipse(&mut app);
    let original = app.document.object(reference).unwrap().geometry().clone();
    app.document.clear_history().unwrap();
    submit(&mut app, "ChamferEdge Distance=2 Preview=Yes");
    choose_edge(&mut app, pick);

    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    let wait = |app: &mut VibocerosApp, expected: Option<&[viboceros_geometry::NurbsCurve]>| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let strokes = viewport_frame(app, &context, vec![]);
            let curves = app.edge_blend_preview.curves();
            if !strokes.is_empty()
                && !curves.is_empty()
                && expected.is_none_or(|expected| curves == expected)
            {
                return curves.to_vec();
            }
            assert!(Instant::now() < deadline, "{:?}", app.command_log);
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let initial = wait(&mut app, None);
    submit(&mut app, "FromCurve");
    let target = move_measurement_pointer(&mut app, &context, point(24., 10., 0.));
    let radius = app.edge_blend_size_hover().unwrap();
    assert!((radius - 1.).abs() < 1e-7, "measured radius {radius}");
    let expected = viboceros_command::EdgeBlendSelection::prepare_chamfer(
        &app.document,
        [(pick.object, pick.index)],
        radius,
        viboceros_command::chamfer_edge::ChamferOptions::default().rail,
    )
    .unwrap();
    let Geometry::Brep(expected) = &expected.replacements()[0].1 else {
        panic!("expected a chamfer preview");
    };
    let expected = expected
        .edges()
        .iter()
        .map(|edge| edge.curve().clone())
        .collect::<Vec<_>>();
    let trial = wait(&mut app, Some(&expected));
    assert_ne!(trial, initial);
    assert!(!app.document.can_undo());

    viewport_click(&mut app, &context, target);
    assert!(!app.measuring_edge_blend_radius(), "{:?}", app.command_log);
    assert!(app.edge_fillet_prompt.is_some());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains(&format!("Distance={radius}")),
        "{:?}",
        app.command_log
    );
    assert!(!app.document.can_undo());
    submit(&mut app, "");
    let Geometry::Brep(committed) = app.document.object(pick.object).unwrap().geometry() else {
        panic!("expected a committed chamfer");
    };
    assert!(
        committed
            .edges()
            .iter()
            .map(|edge| edge.curve())
            .eq(trial.iter())
    );
    assert_eq!(
        app.document.object(reference).unwrap().geometry(),
        &original
    );
    assert_eq!(app.document.undo_label(), Some("ChamferEdge"));
}
