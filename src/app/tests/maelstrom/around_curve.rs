use super::super::circle_preview::{screen_point, viewport_frame};
use super::super::curve_fillet::enter;
use super::super::curve_match::click_world;
use super::*;
use viboceros_geometry::{Brep, CircularArc3, LineSegment};

const ORIGINAL: [f64; 3] = [2., 3.3, 0.4];
const POSITIVE_COIL: [f64; 3] = [2., 2.3898091340607315, 2.3105870039364236];
const NEGATIVE_COIL: [f64; 3] = [2., 2.8726336379062265, -1.6727151527889133];

fn line(app: &mut VibocerosApp, points: [Point3; 2]) -> ObjectId {
    app.document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(points[0], points[1], app.document.tolerance()).unwrap(),
        ))
        .unwrap()
}

fn setup(reverse: bool) -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let mut ends = [point(0., 0., 0.), point(10., 0., 0.)];
    if reverse {
        ends.reverse();
    }
    let guide = line(&mut app, ends);
    let target = app
        .document
        .add_geometry(Geometry::Point(Point3::try_from(ORIGINAL).unwrap()))
        .unwrap();
    app.document
        .select_object(target, SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, guide, target)
}

fn assert_point(geometry: &Geometry, expected: [f64; 3], label: &str) {
    let Geometry::Point(actual) = geometry else {
        panic!("{label}: expected a point")
    };
    let expected = Point3::try_from(expected).unwrap();
    assert!(
        actual.distance_to(expected).unwrap() < 1e-10,
        "{label}: {actual:?} != {expected:?}"
    );
}

#[test]
fn maelstrom_around_curve_replays_native_size_modes_and_signed_coil_picks() {
    let (mut app, mut guide, mut target) = setup(true);
    let mut previous_direction = true;
    // Point witnesses from Rhino 8.32 with a directed X-axis guide.
    for (label, reverse, first, second, angle, expected) in [
        ("reverse", true, "2", "5", "90", NEGATIVE_COIL),
        ("numeric", false, "2", "5", "90", POSITIVE_COIL),
        (
            "projected picks",
            false,
            "w4,2,0",
            "w4,5,0",
            "w4,0,5",
            POSITIVE_COIL,
        ),
        ("numeric seam", false, "2", "5", "w2,0,5", POSITIVE_COIL),
        ("picked seam", false, "w2,0,2", "5", "w2,0,5", ORIGINAL),
        ("diameter", false, "Diameter=4", "10", "90", POSITIVE_COIL),
        (
            "circumference",
            false,
            "Circumference=12.566370614359172",
            "10",
            "90",
            POSITIVE_COIL,
        ),
        (
            "area",
            false,
            "Area=12.566370614359172",
            "10",
            "90",
            POSITIVE_COIL,
        ),
    ] {
        if reverse != previous_direction {
            (app, guide, target) = setup(reverse);
            previous_direction = reverse;
        }
        app.document
            .select_object(target, SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        let guide_before = app.document.object(guide).unwrap().clone();
        enter(&mut app, "Maelstrom");
        enter(&mut app, "_AroundCurve");
        assert!(app.picking_circle_around_curve());
        enter(&mut app, "_-SelID");
        enter(&mut app, &guide.to_string());
        enter(&mut app, "w2,3,1");
        let center = app.drafting_plane.unwrap().origin();
        assert!(
            center.distance_to(point(2., 0., 0.)).unwrap() < 1e-10,
            "{label}"
        );
        enter(&mut app, first);
        let Some(MaelstromRadius::Number(radius)) = app.maelstrom_preview().unwrap().initial else {
            panic!("{label}: expected an accepted first radius");
        };
        assert!((radius - 2.).abs() < 1e-10, "{label}");
        enter(&mut app, "Copy=No");
        enter(&mut app, second);
        enter(&mut app, angle);
        assert!(app.command_line_idle(), "{label}: {:?}", app.command_log);
        assert_eq!(app.document.objects().len(), 2);
        assert_eq!(app.document.object(guide), Some(&guide_before));
        assert_point(
            app.document.object(target).unwrap().geometry(),
            expected,
            label,
        );
        assert_eq!(app.document.selected_object_count(), 0, "{label}");
        enter(&mut app, "Undo");
        assert_point(
            app.document.object(target).unwrap().geometry(),
            ORIGINAL,
            label,
        );
        assert!(!app.document.can_undo());
        assert_eq!(app.document.selected_object_count(), 0, "{label}");
        enter(&mut app, "Redo");
        assert_point(
            app.document.object(target).unwrap().geometry(),
            expected,
            label,
        );
        assert_eq!(app.document.selected_object_count(), 0, "{label}");
        enter(&mut app, "Undo");
    }
    assert_eq!(app.commands.maelstrom_radius_default(), 2.);
    assert!(app.commands.maelstrom_uses_diameter());
    assert_eq!(app.commands.circle_preferences().radius, 1.);
    assert!(!app.commands.circle_preferences().diameter);
}

#[test]
fn maelstrom_around_curve_uses_the_tangent_at_the_projected_center() {
    for (reverse, expected) in [
        (false, [2., 3.24029525888357, 0.5582720401300227]),
        (true, [2., 3.3395971177443706, 0.2355409988324207]),
    ] {
        let (mut app, guide, target) = setup(false);
        app.document.delete_objects([guide]).unwrap();
        let mut points = [point(0., 0., 0.), point(2., 2., 0.), point(4., 0., 0.)];
        if reverse {
            points.reverse();
        }
        let arc = CircularArc3::try_from_three_points(
            points[0],
            points[1],
            points[2],
            app.document.tolerance(),
        )
        .unwrap();
        let guide = app.document.add_geometry(Geometry::Arc(arc)).unwrap();
        app.document.clear_history().unwrap();
        enter(&mut app, "Maelstrom");
        enter(&mut app, "AroundCurve");
        enter(&mut app, &format!("Curve={guide}"));
        enter(&mut app, "w2,4,0");
        let center = app.drafting_plane.unwrap().origin();
        assert!(center.distance_to(point(2., 2., 0.)).unwrap() < 1e-10);
        for input in ["0.6", "Copy=No", "5", "90"] {
            enter(&mut app, input);
        }
        assert!(app.command_line_idle(), "{:?}", app.command_log);
        assert_point(
            app.document.object(target).unwrap().geometry(),
            expected,
            "arc guide",
        );
        assert_eq!(
            app.document.object(guide).unwrap().geometry(),
            &Geometry::Arc(arc)
        );
    }
}

#[test]
fn maelstrom_around_curve_viewport_guide_circle_and_coil_previews() {
    let (mut app, guide, target) = setup(false);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "AroundCurve");
    click_world(&mut app, &context, point(5., 0., 0.));
    assert!(app.document.is_selected(guide));
    assert!(!app.document.is_selected(target));
    assert!(
        app.circle_around_session
            .as_ref()
            .unwrap()
            .cursor_curve()
            .is_some()
    );
    enter(&mut app, "w2,0,0");
    enter(&mut app, "ProjectOsnap=No");
    assert!(!app.circle_project_osnap);
    assert!(app.circle_snap_plane().is_some());
    assert!(app.maelstrom_preview().is_none());
    app.viewports[0] = Viewport::new(ViewKind::Right);
    viewport_frame(&mut app, &context, vec![]);
    let pixel = screen_point(&app, point(2., 2., 0.));
    let overlay = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pixel)]);
    assert!(
        overlay.len() > 10,
        "the perpendicular circle should be visible"
    );
    assert!(!app.document.can_undo());
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "w2,2,0");
    assert!(app.circle_around_session.is_none());
    assert!(app.maelstrom_preview().is_some());
    enter(&mut app, "5");
    viewport_frame(&mut app, &context, vec![]);
    let angle_pixel = screen_point(&app, point(2., 0., 5.));
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(angle_pixel)],
    );
    let angle = app
        .maelstrom_preview()
        .unwrap()
        .last
        .unwrap()
        .angle
        .unwrap();
    assert!((angle - 90.).abs() < 0.5, "hovered coil angle {angle}");
    assert!(app.document.is_selected(guide));
    assert_point(
        app.document.object(target).unwrap().geometry(),
        ORIGINAL,
        "preview",
    );
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(app.document.selected_object_count(), 0);
    assert!(!app.document.can_undo());
}

#[test]
fn maelstrom_around_curve_accepts_a_viewport_edge_without_editing_its_parent() {
    let (mut app, guide, target) = setup(false);
    app.document.delete_objects([guide]).unwrap();
    let parent = app
        .document
        .add_geometry(Geometry::Brep(
            Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 10.], [0., 2.], [0., 1.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    let saved = app.document.object(parent).unwrap().clone();
    app.document.clear_history().unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "AroundCurve");
    app.handle_viewport_action(ViewportOutput {
        edge_click: Some(vec![crate::viewport::EdgePick {
            object: parent,
            edge: 0,
        }]),
        ..Default::default()
    });
    assert!(!app.picking_circle_around_curve());
    assert!(!app.document.is_selected(parent));
    enter(&mut app, "w2,0,0");
    let plane = app.drafting_plane.unwrap();
    assert!(plane.origin().distance_to(point(2., 0., 0.)).unwrap() < 1e-10);
    let expected = if plane.z_axis().as_vector().x() > 0. {
        POSITIVE_COIL
    } else {
        NEGATIVE_COIL
    };
    for input in ["2", "5", "90"] {
        enter(&mut app, input);
    }
    assert_point(
        app.document.object(target).unwrap().geometry(),
        expected,
        "edge guide",
    );
    for input in ["Undo", "Redo"] {
        enter(&mut app, input);
        assert_eq!(app.document.object(parent), Some(&saved));
        assert_eq!(app.document.selected_object_count(), 0);
    }
}

#[test]
fn maelstrom_around_curve_distinguishes_automatic_reference_from_explicit_pick() {
    let (mut app, guide, target) = setup(false);
    app.document
        .select_object(guide, SelectionMode::Add)
        .unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "AroundCurve");
    assert!(!app.picking_circle_around_curve());
    for input in ["w2,3,1", "2", "5", "90"] {
        enter(&mut app, input);
    }
    assert_point(
        app.document.object(target).unwrap().geometry(),
        POSITIVE_COIL,
        "automatic guide",
    );
    assert!(app.document.is_selected(guide));
    assert!(app.document.is_selected(target));
    enter(&mut app, "Undo");

    // Two eligible preselected guide curves require a new pick. The explicit
    // guide remains a deformation target when it was in the original set.
    let peer = line(&mut app, [point(0., 9., 0.), point(10., 9., 0.)]);
    app.document
        .select_object(peer, SelectionMode::Add)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Maelstrom");
    enter(&mut app, "AroundCurve");
    assert!(app.picking_circle_around_curve());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &format!("SelID={guide}"));
    for input in ["w2,0,0", "2", "5", "90"] {
        enter(&mut app, input);
    }
    assert_eq!(app.document.selected_object_count(), 0);
    for id in [guide, peer] {
        assert!(matches!(
            app.document.object(id).unwrap().geometry(),
            Geometry::NurbsCurve(_)
        ));
    }
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
    for id in [guide, peer] {
        assert!(matches!(
            app.document.object(id).unwrap().geometry(),
            Geometry::Line(_)
        ));
    }
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn maelstrom_around_curve_repeated_copies_share_one_undo_entry() {
    let (mut app, guide, target) = setup(false);
    enter(&mut app, "Maelstrom");
    enter(&mut app, "AroundCurve");
    enter(&mut app, &format!("SelID {guide}"));
    for input in ["w2,0,0", "2", "Copy=Yes", "5", "90", "-90"] {
        enter(&mut app, input);
    }
    assert!(app.document.is_selected(guide));
    enter(&mut app, "Enter");
    let copies = app
        .document
        .objects()
        .filter(|o| o.id() != guide && o.id() != target)
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    for (object, expected) in copies.into_iter().zip([POSITIVE_COIL, NEGATIVE_COIL]) {
        assert_point(object.geometry(), expected, "copy");
    }
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.selected_object_count(), 0);
}
