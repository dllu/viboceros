use super::*;

#[test]
fn bare_radius_reports_preselected_circles_but_prompts_for_lines() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        app.execute_command("Circle 0,0,0 2");
        app.execute_command("SelAll");
        let before = format!("{:?}", app.document);
        app.command_input = name.into();
        app.run_command();
        assert!(app.active_command.is_none());
        assert_eq!(app.command_log.back().unwrap(), "Radius = 2; Diameter = 4");
        assert_eq!(format!("{:?}", app.document), before);
        app.execute_command("Delete");
        app.execute_command("Line 0,0,0 1,0,0");
        app.execute_command("SelAll");
        assert!(app.try_start_interactive_command(name));
        assert!(app.active_command.is_some());
    }
}

#[test]
fn radius_without_preselection_keeps_failed_pick_open_then_measures() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command(name));
        let pending = app.active_command;
        let previous_last = app.last_point;
        assert!(!app.accept_drafting_point(point(2., 0., 0.)));
        assert_eq!(app.active_command, pending);
        assert_eq!(app.last_point, previous_last);
        assert_eq!(format!("{:?}", app.document), before);
        // Simulate a document update without dispatching a replacement command.
        app.commands
            .execute(&mut app.document, "Circle 0,0,0 2")
            .unwrap();
        assert_eq!(app.document.selected_object_count(), 0);
        let before = format!("{:?}", app.document);
        assert!(app.accept_drafting_point(point(2., 0., 0.)));
        assert!(app.active_command.is_none());
        assert_eq!(app.command_log.back().unwrap(), "Radius = 2; Diameter = 4");
        assert_eq!(format!("{:?}", app.document), before);
    }
}

#[test]
fn radius_and_diameter_point_input_report_mark_and_cancel() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        app.execute_command("Circle 0,0,0 2");
        app.execute_command("SelAll");
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command(&format!("{name} Mark{name}=No")));
        assert!(app.accept_drafting_point(point(2., 0., 0.)));
        assert_eq!(app.command_log.back().unwrap(), "Radius = 2; Diameter = 4");
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command(&format!("{name} Mark{name}=No")));
        app.cancel_interactive_command(true);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.try_start_interactive_command(&format!("{name} Mark{name}=Yes")));
        assert!(app.try_continue_point_input("w2,0,0"));
        assert_eq!(app.document.objects().count(), 3);
        assert_eq!(app.document.undo_label(), Some(name));
        app.execute_command("Undo");
        assert_eq!(app.document.objects().count(), 1);
        assert!(!app.try_start_interactive_command(&format!("{name} Mark{name}=Maybe")));
        assert!(!app.try_start_interactive_command(&format!("{name} 2,0,0")));
    }
}

#[test]
fn radius_prompt_accepts_and_revises_display_units() {
    for name in ["Radius", "Diameter"] {
        let mut app = test_app();
        app.execute_command("Units Meters Scale=No");
        app.execute_command("Circle 0,0,0 2");
        let before = format!("{:?}", app.document);
        assert!(app.try_start_interactive_command(&format!("{name} Units=cm")));
        let pending = app.active_command;
        assert!(app.try_continue_radius("Units=invalid"));
        assert_eq!(app.active_command, pending);
        assert!(app.try_continue_radius("Units=Model_Units"));
        assert!(app.try_continue_radius("Units=cm"));
        assert!(app.try_continue_point_input("2,0,0"));
        assert_eq!(
            app.command_log.back().map(String::as_str),
            Some("Radius = 200 Centimetres; Diameter = 400 Centimetres")
        );
        assert_eq!(format!("{:?}", app.document), before);

        app.execute_command("SelAll");
        assert!(!app.try_start_interactive_command(&format!("{name} Units=cm")));
        assert!(app.try_start_interactive_command(&format!("{name} Mark{name}=No")));
        let pending = app.active_command;
        assert!(app.try_continue_radius("Units=cm"));
        assert_eq!(app.active_command, pending);
    }
}

fn radius_view() -> (VibocerosApp, egui::Context) {
    use super::circle_preview::viewport_frame;
    let mut app = test_app();
    app.active_viewport = 0;
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    let center = point(0., 0., 5.);
    let xy = viboceros_command::CommandContext::default().construction_plane;
    let ellipse = viboceros_geometry::Ellipse3::try_new(
        center,
        4.,
        2.,
        xy.x_axis(),
        xy.y_axis(),
        app.document.tolerance(),
    )
    .unwrap();
    app.document
        .add_geometry(Geometry::Ellipse(ellipse))
        .unwrap();
    app.execute_command("Line w-5,2.3,0 w5,2.3,0");
    app.document.clear_history().unwrap();
    let ctx = egui::Context::default();
    viewport_frame(&mut app, &ctx, Vec::new());
    viewport_frame(&mut app, &ctx, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(&mut app, &ctx, Vec::new());
    (app, ctx)
}

fn radius_enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

fn move_measurement_pointer(
    app: &mut VibocerosApp,
    ctx: &egui::Context,
    point: Point3,
) -> egui::Pos2 {
    use super::circle_preview::{screen_point, viewport_frame};
    // Starting a command or adding a log row can resize the surrounding panels.
    for _ in 0..6 {
        let pixel = screen_point(app, point);
        viewport_frame(app, ctx, vec![egui::Event::PointerMoved(pixel)]);
        if screen_point(app, point) == pixel && app.radius_status().is_some() {
            return pixel;
        }
    }
    panic!(
        "no measurement at {point:?}, rect {:?}, active {:?}, log {:?}",
        app.viewports[0].screen_rect(),
        app.active_command,
        app.command_log
    );
}

fn report_radius(line: &str) -> f64 {
    line.strip_prefix("Radius = ")
        .unwrap()
        .split(|c: char| c == ';' || c.is_whitespace())
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn radius_viewport_reports_the_hovered_curve_repeatedly_until_enter() {
    use super::circle_preview::{viewport_click, viewport_frame};
    let (mut app, ctx) = radius_view();
    radius_enter(&mut app, "Radius");
    for (position, expected) in [(point(0., 2., 5.), 8.), (point(4., 0., 5.), 1.)] {
        let pixel = move_measurement_pointer(&mut app, &ctx, position);
        let radius = report_radius(app.radius_status().unwrap());
        assert!((radius - expected).abs() < 1e-7, "{radius}");
        viewport_click(&mut app, &ctx, pixel);
        assert!(!app.command_line_idle());
        let report = app
            .command_log
            .iter()
            .rev()
            .find(|line| line.starts_with("Radius = ") && line.contains("; Diameter = "))
            .unwrap_or_else(|| panic!("{:?}", app.command_log));
        assert!((report_radius(report) - radius).abs() < 1e-7, "{report}");
    }
    viewport_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(egui::pos2(5., 5.))],
    );
    assert!(app.radius_status().is_none());
    assert!(!app.document.can_undo());
    radius_enter(&mut app, "");
    assert!(app.command_line_idle());
}

#[test]
fn select_curve_and_subcurve_stages_preserve_selection_and_group_marker_undo() {
    use super::circle_preview::{screen_point, viewport_click, viewport_frame};
    let (mut app, ctx) = radius_view();
    radius_enter(&mut app, "Diameter SelectCurve");
    let on_ellipse = screen_point(&app, point(4., 0., 5.));
    viewport_click(&mut app, &ctx, on_ellipse);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().count(), 2);
    radius_enter(&mut app, "SubCrv");
    assert!(app.subcurve_prompt.is_some());
    radius_enter(&mut app, "w4,0,5");
    radius_enter(&mut app, "w0,2,5");
    assert!(app.subcurve_prompt.is_none(), "{:?}", app.command_log);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Radius { diameter: true, .. })
    ));
    assert_eq!(app.document.objects().count(), 2);
    radius_enter(&mut app, "MarkDiameter");
    for position in [point(4., 0., 5.), point(0., 2., 5.)] {
        let pixel = screen_point(&app, position);
        viewport_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pixel)]);
        assert!(app.radius_status().unwrap().starts_with("Diameter = "));
        viewport_click(&mut app, &ctx, pixel);
        assert!(!app.command_line_idle());
    }
    radius_enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 6);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.undo_label(), Some("Diameter"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.document.can_undo());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 6);
    radius_enter(&mut app, "Diameter");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Radius { mark: true, .. })
    ));
    app.cancel_interactive_command(false);
}

#[test]
fn nested_radius_returns_a_model_length_to_fillet_and_preserves_the_parent_on_escape() {
    use super::circle_preview::viewport_click;
    let (mut app, ctx) = radius_view();
    radius_enter(&mut app, "Fillet Radius=0.5");
    radius_enter(&mut app, "Radius");
    let parent = app.active_command;
    let old_radius = app.commands.fillet_preferences().radius;
    radius_enter(&mut app, "'Radius");
    assert!(app.measuring_nested_radius());
    app.cancel_current_prompt_or_selection();
    assert!(!app.measuring_nested_radius());
    assert_eq!(app.active_command, parent);
    assert_eq!(app.commands.fillet_preferences().radius, old_radius);

    radius_enter(&mut app, "Radius");
    let flat = move_measurement_pointer(&mut app, &ctx, point(-4.5, 2.3, 0.));
    assert_eq!(app.radius_status(), Some("Radius = infinite"));
    viewport_click(&mut app, &ctx, flat);
    assert!(app.measuring_nested_radius(), "{:?}", app.command_log);
    let ellipse = move_measurement_pointer(&mut app, &ctx, point(4., 0., 5.));
    viewport_click(&mut app, &ctx, ellipse);
    assert!(!app.measuring_nested_radius());
    assert!(app.curve_fillet_session.is_some());
    assert!((app.commands.fillet_preferences().radius - 1.).abs() < 1e-7);
    assert!(!app.document.can_undo());
}

#[test]
fn nested_radius_can_measure_an_unselected_curve_during_offset_surface() {
    let (mut app, _) = radius_view();
    radius_enter(&mut app, "Plane w0,0,0 w10,10,0");
    let plane = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([plane], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    radius_enter(&mut app, "OffsetSrf");
    radius_enter(&mut app, "Distance");
    radius_enter(&mut app, "'Radius");
    assert!(app.measuring_nested_radius(), "{:?}", app.command_log);
    radius_enter(&mut app, "w0,2,5");
    assert!(!app.measuring_nested_radius(), "{:?}", app.command_log);
    assert!(app.offset_surface_session.is_some());
    assert!((app.commands.offset_surface_options().distance - 8.).abs() < 1e-7);
    assert!(app.document.is_selected(plane));
    assert!(!app.document.can_undo());
}

#[test]
fn radius_unit_menu_accepts_and_remembers_native_imperial_formats() {
    let (mut app, _) = radius_view();
    app.execute_command("Units Millimeters Scale=No");
    radius_enter(&mut app, "Radius");
    radius_enter(&mut app, "Units");
    radius_enter(&mut app, "Fractional_Inches");
    radius_enter(&mut app, "w0,2,5");
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Radius = 3/8 inches; Diameter = 5/8 inches")
    );
    radius_enter(&mut app, "Radius");
    radius_enter(&mut app, "w0,2,5");
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Radius = 3/8 inches; Diameter = 5/8 inches")
    );
    radius_enter(&mut app, "Radius Units=Feet_Inches");
    radius_enter(&mut app, "w0,2,5");
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Radius = 0-3/8\"; Diameter = 0-5/8\"")
    );
    assert_eq!(
        app.document.units(),
        &viboceros_geometry::LengthUnitSystem::Millimeters
    );
}
