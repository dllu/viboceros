use super::*;
use crate::app::tests::circle_preview::{screen_point, viewport_frame};
use crate::viewport::edge_blend::{HandleLocation, SizeHandle, SizeHandleId};
use viboceros_geometry::NurbsCurve;

pub(super) fn begin(command: &str) -> (VibocerosApp, egui::Context, ComponentPick, ComponentPick) {
    let mut app = test_app();
    let base = box_pick(&mut app);
    let Geometry::Brep(solid) = app.document.object(base.object).unwrap().geometry() else {
        unreachable!()
    };
    let select = |y, z| ComponentPick {
        index: solid
            .edges()
            .iter()
            .position(|edge| {
                edge.vertices().iter().all(|&v| {
                    let p = solid.vertices()[v].point();
                    p.y() == y && p.z() == z
                })
            })
            .unwrap(),
        ..base
    };
    let first = select(0., 0.);
    let second = select(10., 10.);
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    submit(&mut app, command);
    for pick in [first, second] {
        app.accept_component_window(ComponentWindow {
            picks: vec![pick],
            modifiers: egui::Modifiers::NONE,
            preselection: false,
            crossing: false,
            inverted: false,
        });
        let key = if command.starts_with("ChamferEdge") {
            "NextChamferDistance"
        } else {
            "NextRadius"
        };
        submit(&mut app, &format!("{key}=1.4"));
    }
    submit(&mut app, "");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    (app, context, first, second)
}

fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    pos: egui::Pos2,
    pressed: Option<bool>,
    modifiers: egui::Modifiers,
) {
    let mut events = vec![
        egui::Event::ModifiersChanged(modifiers),
        egui::Event::PointerMoved(pos),
    ];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers,
        });
    }
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show_frame(ui, None),
        )
        .drop_without_applying_deltas();
}

pub(super) fn click(
    app: &mut VibocerosApp,
    context: &egui::Context,
    pos: egui::Pos2,
    modifiers: egui::Modifiers,
) {
    frame(app, context, pos, Some(true), modifiers);
    frame(app, context, pos, Some(false), modifiers);
}

fn handle(app: &VibocerosApp, id: SizeHandleId) -> SizeHandle {
    app.edge_blend_size_handles()
        .into_iter()
        .find(|h| h.id == id)
        .unwrap()
}

fn ends(pick: ComponentPick) -> [SizeHandleId; 2] {
    [HandleLocation::Start, HandleLocation::End].map(|location| SizeHandleId { pick, location })
}

pub(super) fn preview_when(
    app: &mut VibocerosApp,
    context: &egui::Context,
    condition: impl Fn(&[NurbsCurve]) -> bool,
) -> Vec<NurbsCurve> {
    let start = std::time::Instant::now();
    loop {
        let strokes = viewport_frame(app, context, vec![]);
        let curves = app.edge_blend_preview.curves();
        if !strokes.is_empty() && !curves.is_empty() && condition(curves) {
            return curves.to_vec();
        }
        assert!(start.elapsed().as_secs() < 15, "{:?}", app.command_log);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn edge_fillet_shift_click_drag_changes_all_selected_handles_live() {
    let (mut app, context, first, second) = begin("FilletEdge 1 Preview=Yes");
    let [a, untouched] = ends(first);
    let [b, other] = ends(second);
    let original = app
        .document
        .object(first.object)
        .unwrap()
        .geometry()
        .clone();
    let pa = screen_point(&app, handle(&app, a).point);
    let pb = screen_point(&app, handle(&app, b).point);
    click(&mut app, &context, pa, egui::Modifiers::SHIFT);
    click(&mut app, &context, pb, egui::Modifiers::SHIFT);
    assert!(
        handle(&app, a).active && handle(&app, b).active,
        "{:?}",
        app.command_log
    );
    assert!(!handle(&app, untouched).active && !handle(&app, other).active);
    assert_eq!([handle(&app, a).size, handle(&app, b).size], [1., 1.4]);

    let more = pa - egui::vec2(0., 20.);
    let none = egui::Modifiers::NONE;
    frame(&mut app, &context, pa, Some(true), none);
    frame(&mut app, &context, more, None, none);
    let value = handle(&app, a).size;
    assert!(value > 1.);
    assert_eq!(handle(&app, b).size, value);
    assert_eq!(handle(&app, untouched).size, 1.);
    assert_eq!(handle(&app, other).size, 1.4);
    frame(&mut app, &context, more, Some(false), none);
    assert_eq!(handle(&app, a).size, value);
    assert_eq!(handle(&app, b).size, value);
    assert!(handle(&app, a).active && handle(&app, b).active);
    assert!(!app.document.can_undo());
    assert_eq!(
        app.document.object(first.object).unwrap().geometry(),
        &original
    );

    let preview = preview_when(&mut app, &context, |_| true);
    submit(&mut app, "");
    let Geometry::Brep(solid) = app.document.object(first.object).unwrap().geometry() else {
        unreachable!()
    };
    assert!(solid.is_solid());
    assert_eq!(
        solid
            .edges()
            .iter()
            .map(|edge| edge.curve().clone())
            .collect::<Vec<_>>(),
        preview
    );
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
}

#[test]
fn edge_fillet_window_select_and_control_click_edit_a_handle_subset() {
    for command in ["ChamferEdge 1 RailType=RollingBall", "BlendEdge 1"] {
        let (mut app, context, first, second) = begin(command);
        let pick_left = |app: &VibocerosApp, pick| {
            let [a, b] = ends(pick);
            if handle(app, a).point.x() < handle(app, b).point.x() {
                a
            } else {
                b
            }
        };
        let a = pick_left(&app, first);
        let b = pick_left(&app, second);
        let pa = screen_point(&app, handle(&app, a).point);
        let pb = screen_point(&app, handle(&app, b).point);
        let lo = pa.min(pb) - egui::vec2(12., 12.);
        let hi = pa.max(pb) + egui::vec2(12., 12.);
        let none = egui::Modifiers::NONE;
        frame(&mut app, &context, lo, Some(true), none);
        frame(&mut app, &context, hi, None, none);
        frame(&mut app, &context, hi, Some(false), none);
        let count = app
            .edge_blend_size_handles()
            .iter()
            .filter(|h| h.active)
            .count();
        assert_eq!(count, 2, "{:?}", app.command_log);
        assert!(handle(&app, a).active && handle(&app, b).active);

        click(&mut app, &context, pa, none);
        if command.starts_with("ChamferEdge") {
            submit(&mut app, "CurrentChamferDistance=2");
        } else {
            submit(&mut app, "FromTwoPoints");
            submit(&mut app, "w0,0,0");
            submit(&mut app, "w2,0,0");
        }
        assert_eq!([handle(&app, a).size, handle(&app, b).size], [2., 2.]);
        click(&mut app, &context, pb, egui::Modifiers::CTRL);
        assert!(handle(&app, a).active && !handle(&app, b).active);
        click(&mut app, &context, pa, none);
        submit(&mut app, "1.5");
        assert_eq!([handle(&app, a).size, handle(&app, b).size], [1.5, 2.]);
        assert!(!app.document.can_undo());
        submit(&mut app, "");
        assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
        assert!(app.document.can_undo());
    }
}

#[test]
fn edge_fillet_remove_handles_by_click_and_window_updates_pending_geometry() {
    let (mut app, context, first, second) = begin("FilletEdge 1 Preview=Yes");
    let original = app
        .document
        .object(first.object)
        .unwrap()
        .geometry()
        .clone();
    let baseline = preview_when(&mut app, &context, |_| true);
    let none = egui::Modifiers::NONE;
    submit(&mut app, "AddHandle");
    submit(&mut app, "CurrentRadius=1");
    for x in [3., 6., 8.] {
        let pixel = screen_point(&app, point(x, 0., 0.));
        click(&mut app, &context, pixel, none);
    }
    submit(&mut app, "CurrentRadius=1.6");
    let pixel = screen_point(&app, point(4., 10., 10.));
    click(&mut app, &context, pixel, none);
    submit(&mut app, "");
    let handles = app.edge_blend_size_handles();
    assert_eq!(handles.len(), 8, "{:?}", app.command_log);
    let endpoints =
        |app: &VibocerosApp| [first, second].map(|pick| ends(pick).map(|id| handle(app, id).size));
    let original_sizes = endpoints(&app);
    preview_when(&mut app, &context, |curves| curves != baseline);

    let endpoint = screen_point(&app, handle(&app, ends(first)[0]).point);
    let single_handle = handles
        .iter()
        .find(|h| {
            matches!(h.id.location, HandleLocation::Interior(_)) && (h.point.x() - 8.).abs() < 1e-5
        })
        .unwrap();
    let single = screen_point(&app, single_handle.point);
    submit(&mut app, "RemoveHandle");
    click(&mut app, &context, endpoint, none);
    assert_eq!(app.edge_blend_size_handles().len(), 8);
    click(&mut app, &context, single, none);
    assert_eq!(app.edge_blend_size_handles().len(), 7);

    // The window encloses both edges' left endpoints as well as added handles.
    let a = screen_point(&app, point(0.3, 0., 0.));
    let b = screen_point(&app, point(6.4, 10., 10.));
    let lo = a.min(b) - egui::vec2(1., 8.);
    let hi = a.max(b) + egui::vec2(1., 8.);
    frame(&mut app, &context, lo, Some(true), none);
    frame(&mut app, &context, hi, None, none);
    assert_eq!(app.edge_blend_size_handles().len(), 7);
    frame(&mut app, &context, hi, Some(false), none);
    assert_eq!(
        app.edge_blend_size_handles().len(),
        4,
        "{:?}",
        app.command_log
    );
    assert_eq!(endpoints(&app), original_sizes);
    preview_when(&mut app, &context, |curves| curves == baseline);
    assert!(!app.document.can_undo());
    assert_eq!(
        app.document.object(first.object).unwrap().geometry(),
        &original
    );

    submit(&mut app, "");
    assert!(app.adjusting_edge_blend());
    assert!(!app.document.can_undo());
    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none());
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
}

#[test]
fn edge_fillet_previews_the_unplaced_handle_and_commits_the_clicked_location() {
    let (mut app, context, first, _) = begin("FilletEdge 1 Preview=Yes");
    let original = app
        .document
        .object(first.object)
        .unwrap()
        .geometry()
        .clone();
    let baseline = preview_when(&mut app, &context, |_| true);
    submit(&mut app, "AddHandle");
    submit(&mut app, "CurrentRadius=1.6");
    let pixel = screen_point(&app, point(4., 0., 0.));
    let none = egui::Modifiers::NONE;
    frame(&mut app, &context, pixel, None, none);
    let trial = preview_when(&mut app, &context, |curves| curves != baseline);
    assert_eq!(app.edge_blend_size_handles().len(), 4);
    assert!(!app.document.can_undo());
    assert_eq!(
        app.document.object(first.object).unwrap().geometry(),
        &original
    );

    let away = screen_point(&app, point(5., 5., 0.));
    frame(&mut app, &context, away, None, none);
    preview_when(&mut app, &context, |curves| curves == baseline);
    click(&mut app, &context, pixel, none);
    assert_eq!(app.edge_blend_size_handles().len(), 5);
    submit(&mut app, "");
    preview_when(&mut app, &context, |curves| curves == trial);
    assert!(!app.document.can_undo());

    submit(&mut app, "");
    let Geometry::Brep(solid) = app.document.object(first.object).unwrap().geometry() else {
        unreachable!()
    };
    assert!(solid.is_solid());
    assert_eq!(solid.edges().len(), trial.len());
    assert!(
        solid
            .edges()
            .iter()
            .zip(&trial)
            .all(|(edge, curve)| edge.curve() == curve)
    );
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
}
