use super::*;

#[test]
fn interactive_arc_center_angle_accepts_option_and_reprompts_on_bad_input() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Arc"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ArcCenter {
            center: None,
            start: None,
            clockwise: true
        })
    );
    assert!(app.try_continue_arc("Center"));
    let center = point(1.0, 2.0, 3.0);
    let start = point(5.0, 2.0, 3.0);
    assert!(app.accept_drafting_point(center));
    assert!(!app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("0"));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ArcCenter {
            center: Some(_),
            start: Some(_),
            ..
        })
    ));
    assert!(app.try_continue_arc("90"));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Arc(arc) if (arc.sweep_radians().to_degrees() - 90.0).abs() < 1e-12
    ));
    assert_eq!(app.document.undo_label(), Some("Arc"));

    assert!(app.try_start_interactive_command("Arc _Center"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ArcCenter {
            center: None,
            start: None,
            clockwise: true
        })
    );
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("-90"));
    let Geometry::Arc(arc) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("expected arc");
    };
    assert!(arc.point_at(1.0).unwrap().y() < center.y());

    assert!(app.try_start_interactive_command("Arc Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("450"));
    let Geometry::Arc(full) = app.document.objects().nth(2).unwrap().geometry() else {
        panic!("expected full arc")
    };
    assert!((full.sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
}

#[test]
fn bare_arc_picks_center_first_and_start_point_can_choose_through_point() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("A"));
    assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    assert!(app.accept_drafting_point(point(5.0, 2.0, 3.0)));
    assert!(app.accept_drafting_point(point(1.0, 6.0, 3.0)));
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected center-first arc")
    };
    assert!((arc.sweep_radians().to_degrees() - 270.0).abs() < 1e-12);

    assert!(app.try_start_interactive_command("Arc"));
    assert!(app.try_continue_arc("StartPoint"));
    assert!(app.accept_drafting_point(point(5.0, 0.0, 0.0)));
    assert!(app.try_continue_arc("ThroughPoint"));
    assert!(app.accept_drafting_point(point(0.0, 5.0, 0.0)));
    assert!(app.accept_drafting_point(point(-5.0, 0.0, 0.0)));
    let Geometry::Arc(through) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("expected through-point arc")
    };
    assert!((through.sweep_radians().to_degrees() - 180.0).abs() < 1e-12);
}

#[test]
fn interactive_arc_center_length_accepts_signed_sizes() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Arc Center"));
    let center = point(1.0, 2.0, 3.0);
    let start = point(5.0, 2.0, 3.0);
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("Length"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ArcCenterLength { center, start })
    );
    assert!(app.try_continue_arc("0"));
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.try_continue_arc("-6.283185307179586"));
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected arc")
    };
    assert!((*arc.domain().end() - std::f64::consts::TAU).abs() < 1e-12);
    assert!(arc.point_at(1.0).unwrap().y() < center.y());

    assert!(app.try_start_interactive_command("Arc Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("Length=9.42477796076938"));
    assert_eq!(app.document.objects().len(), 2);

    assert!(app.try_start_interactive_command("Arc Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("Length=100"));
    let Geometry::Arc(full) = app.document.objects().nth(2).unwrap().geometry() else {
        panic!("expected full arc")
    };
    assert!((full.sweep_radians() - std::f64::consts::TAU).abs() < 1e-12);
}

#[test]
fn interactive_arc_center_endpoint_pick_uses_the_cplane_and_reprompts() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Arc Center"));
    let center = point(1.0, 2.0, 3.0);
    let start = point(5.0, 2.0, 3.0);
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(!app.accept_drafting_point(center));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ArcCenter {
            center: Some(_),
            start: Some(_),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(1.0, 10.0, 3.0)));
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected arc")
    };
    assert!((arc.sweep_radians().to_degrees() - 270.0).abs() < 1e-12);
    assert!(
        arc.end()
            .unwrap()
            .is_near(point(1.0, 6.0, 3.0), Tolerance::DEFAULT)
    );
    assert_eq!(app.document.undo_label(), Some("Arc"));

    assert!(app.try_start_interactive_command("Arc Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("Direction=Counterclockwise"));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ArcCenter {
            clockwise: false,
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(1.0, 6.0, 3.0)));
    let Geometry::Arc(short) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("expected counterclockwise arc")
    };
    assert!((short.sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
}

#[test]
fn interactive_arc_start_point_direction_retains_invalid_end_pick() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Arc StartPoint"));
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert!(app.try_continue_arc("Direction"));
    assert!(!app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(1.0, 0.0, 0.0)));
    assert!(!app.accept_drafting_point(point(2.0, 0.0, 0.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ArcStartDirection {
            start: Some(_),
            tangent: Some(_)
        })
    ));
    assert!(app.accept_drafting_point(point(2.0, 2.0, 0.0)));
    let Geometry::Arc(arc) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected arc")
    };
    assert!((arc.sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    assert_eq!(app.document.undo_label(), Some("Arc"));

    assert!(app.try_start_interactive_command("Arc StartPoint"));
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(2.0, 2.0, 0.0)));
    assert!(app.accept_drafting_point(point(1.0, 0.0, 0.0)));
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn interactive_arc_start_point_center_accepts_angle_length_and_endpoint() {
    let mut app = test_app();
    let start = point(5.0, 2.0, 3.0);
    let center = point(1.0, 2.0, 3.0);
    assert!(app.try_start_interactive_command("Arc StartPoint Center"));
    assert!(app.accept_drafting_point(start));
    assert!(!app.accept_drafting_point(start));
    assert!(app.accept_drafting_point(center));
    assert!(app.try_continue_arc("90"));

    assert!(app.try_start_interactive_command("Arc StartPoint"));
    assert!(app.accept_drafting_point(start));
    assert!(app.try_continue_arc("Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.try_continue_arc("Length"));
    assert!(app.try_continue_arc("-6.283185307179586"));

    assert!(app.try_start_interactive_command("Arc StartPoint Center"));
    assert!(app.accept_drafting_point(start));
    assert!(app.accept_drafting_point(center));
    assert!(app.try_continue_arc("Direction=Counterclockwise"));
    assert!(!app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(point(1.0, 10.0, 3.0)));

    let arcs = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 3);
    assert!((arcs[0].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
    assert!(arcs[1].point_at(0.25).unwrap().y() < 2.0);
    assert!((arcs[2].sweep_radians().to_degrees() - 90.0).abs() < 1e-12);
}

#[test]
fn interactive_arc_center_midpoint_accepts_angle_length_and_endpoint() {
    let mut app = test_app();
    let center = point(1.0, 2.0, 3.0);
    let midpoint = point(1.0, 6.0, 3.0);
    assert!(app.try_start_interactive_command("Arc Center Midpoint"));
    assert!(app.accept_drafting_point(center));
    assert!(!app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(midpoint));
    assert!(app.try_continue_arc("90"));

    assert!(app.try_start_interactive_command("Arc Center"));
    assert!(app.accept_drafting_point(center));
    assert!(app.try_continue_arc("Midpoint"));
    assert!(app.accept_drafting_point(midpoint));
    assert!(app.try_continue_arc("Length"));
    assert!(app.try_continue_arc("-6.283185307179586"));

    assert!(app.try_start_interactive_command("Arc Center Midpoint"));
    assert!(app.accept_drafting_point(center));
    assert!(app.accept_drafting_point(midpoint));
    assert!(!app.accept_drafting_point(point(1.0, 10.0, 3.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ArcCenterMidpoint {
            midpoint: Some(_),
            ..
        })
    ));
    assert!(app.try_continue_arc("Direction=Clockwise"));
    assert!(app.accept_drafting_point(point(9.0, 2.0, 3.0)));

    let arcs = app
        .document
        .objects()
        .map(|object| match object.geometry() {
            Geometry::Arc(arc) => *arc,
            _ => panic!("expected arc"),
        })
        .collect::<Vec<_>>();
    assert_eq!(arcs.len(), 3);
    assert!(
        arcs[0]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!(
        arcs[1]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
    assert!(
        arcs[2]
            .point_at(0.5)
            .unwrap()
            .is_near(midpoint, Tolerance::DEFAULT)
    );
}
