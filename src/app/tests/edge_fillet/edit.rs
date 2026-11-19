use super::*;
use crate::app::tests::circle_preview::{screen_point, viewport_frame};
use crate::viewport::{SelectionClick, ViewportOutput};
use std::time::{Duration, Instant};
use viboceros_document::GeometrySnapshot;

fn snapshot(app: &VibocerosApp, id: ObjectId) -> GeometrySnapshot {
    app.document.object(id).unwrap().geometry_snapshot().clone()
}

fn pick_edge(app: &mut VibocerosApp, pick: ComponentPick, remove: bool) {
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: if remove {
            egui::Modifiers::CTRL
        } else {
            egui::Modifiers::NONE
        },
        preselection: false,
        crossing: false,
        inverted: false,
    });
}

fn apply(app: &mut VibocerosApp, command: &str, pick: ComponentPick) {
    submit(app, command);
    pick_edge(app, pick, false);
    submit(app, "");
    submit(app, "");
    assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
}

fn box_setback(geometry: &Geometry) -> f64 {
    let Geometry::Brep(solid) = geometry else {
        panic!("expected a B-rep");
    };
    assert!(solid.is_solid());
    assert_eq!(solid.faces().len(), 7);
    solid
        .vertices()
        .iter()
        .flat_map(|vertex| {
            let p = vertex.point();
            [p.x(), p.y(), p.z()]
        })
        .map(|coordinate| coordinate.min(10. - coordinate))
        .filter(|&setback| setback > 1e-7)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn edge_fillet_edit_previews_and_replaces_joined_results_on_the_same_object() {
    for command in ["FilletEdge", "ChamferEdge", "BlendEdge"] {
        let mut app = test_app();
        let pick = box_pick(&mut app);
        let id = pick.object;
        let original = snapshot(&app, id);
        let rail = if command == "ChamferEdge" {
            " RailType=RollingBall"
        } else {
            ""
        };
        apply(&mut app, &format!("{command} 1{rail}"), pick);
        let joined = snapshot(&app, id);
        assert!((box_setback(&joined) - 1.).abs() < 1e-6);
        let attributes = app.document.object(id).unwrap().attributes().clone();
        app.document.clear_history().unwrap();

        submit(&mut app, &format!("{command} _Edit Preview=Yes"));
        assert!(app.picking_edge_blend_edit());
        app.apply_selection_click(SelectionClick {
            object_id: Some(id),
            mode: SelectionMode::Replace,
        });
        assert!(app.picking_edge_blend_edit());
        submit(&mut app, "");
        assert!(app.adjusting_edge_blend());
        assert_eq!(app.edge_blend_edit_target(), Some(id));
        assert_eq!(
            app.component_selection
                .checked_picks(app.edge_blend_source_document())
                .unwrap(),
            [pick]
        );
        assert!(app.edge_blend_size_handles().iter().all(|h| h.size == 1.));
        submit(&mut app, "SetAll=2");
        let context = egui::Context::default();
        let started = Instant::now();
        let preview = loop {
            let drawn = viewport_frame(&mut app, &context, vec![]);
            if !drawn.is_empty() && !app.edge_blend_preview.curves().is_empty() {
                break app.edge_blend_preview.curves().to_vec();
            }
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "{:?}",
                app.command_log
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(snapshot(&app, id).shares_storage_with(&joined));
        assert!(!app.document.can_undo());
        submit(&mut app, "");
        assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
        let updated = snapshot(&app, id);
        assert!((box_setback(&updated) - 2.).abs() < 1e-6);
        assert_eq!(app.document.objects().count(), 1);
        assert_eq!(app.document.object(id).unwrap().attributes(), &attributes);
        let recipe = updated.edge_blend_recipe().unwrap();
        assert!(recipe.source.shares_storage_with(&original));
        assert_eq!(recipe.edges[0].1.ends, [2.; 2]);
        let Geometry::Brep(solid) = &*updated else {
            unreachable!()
        };
        assert_eq!(
            solid.edges().iter().map(|e| e.curve()).collect::<Vec<_>>(),
            preview.iter().collect::<Vec<_>>()
        );
        assert_eq!(app.document.undo_label(), Some(command));
        assert!(app.document.undo().unwrap().is_some());
        assert!(snapshot(&app, id).shares_storage_with(&joined));
        assert!(app.document.redo().unwrap().is_some());
        assert!(snapshot(&app, id).shares_storage_with(&updated));
    }
}

#[test]
fn edge_fillet_edit_double_click_and_select_edges_removes_only_the_latest_set() {
    let mut app = test_app();
    let first = box_pick(&mut app);
    let id = first.object;
    apply(&mut app, "FilletEdge 1", first);
    let earlier = snapshot(&app, id);
    let Geometry::Brep(solid) = &*earlier else {
        unreachable!()
    };
    let far_edge = solid
        .edges()
        .iter()
        .position(|edge| {
            edge.vertices().iter().all(|&index| {
                let p = solid.vertices()[index].point();
                p.y() == 10. && p.z() == 10.
            })
        })
        .unwrap();
    let latest = ComponentPick {
        index: far_edge,
        ..first
    };
    apply(&mut app, "ChamferEdge 0.5", latest);
    let chamfered = snapshot(&app, id);

    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    let click = screen_point(&app, point(0., 5., 10.));
    let mut time = context.input(|input| input.time);
    for double in [false, true] {
        for pressed in [true, false] {
            time += 0.04;
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000., 800.),
                        )),
                        time: Some(time),
                        events: vec![
                            egui::Event::PointerMoved(click),
                            egui::Event::PointerButton {
                                pos: click,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| app.show_frame(ui, None),
                )
                .drop_without_applying_deltas();
        }
        assert_eq!(
            context.input(|input| input
                .pointer
                .button_double_clicked(egui::PointerButton::Primary)),
            double
        );
    }
    assert_eq!(
        app.edge_blend_edit_target(),
        Some(id),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.edge_blend_size_handles()[0].size, 0.5);
    submit(&mut app, "SelectEdges");
    pick_edge(&mut app, latest, true);
    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none());
    let restored = snapshot(&app, id);
    assert!(restored.shares_storage_with(&earlier));
    assert!(restored.edge_blend_recipe().is_some());
    assert!(app.document.undo().unwrap().is_some());
    assert!(snapshot(&app, id).shares_storage_with(&chamfered));
}

#[test]
fn edge_fillet_edit_uses_original_edges_for_chain_picking_and_cancel() {
    let mut app = test_app();
    let first = box_pick(&mut app);
    let id = first.object;
    let unrelated = box_pick(&mut app);
    apply(&mut app, "FilletEdge", first);
    let joined = snapshot(&app, id);
    app.document.clear_history().unwrap();

    assert!(app.handle_viewport_action(ViewportOutput {
        object_double_click: Some(id),
        ..Default::default()
    }));
    submit(&mut app, "SelectEdges");
    submit(&mut app, "NextRadius=2");
    submit(&mut app, "ChainEdges");
    let other = ComponentPick { index: 3, ..first };
    app.pick_edge_blend_chain(vec![unrelated, other]);
    submit(&mut app, "");
    submit(&mut app, "");
    let handles = app.edge_blend_size_handles();
    assert_eq!(handles.len(), 4);
    assert!(handles.iter().all(|h| h.id.pick.object == id));
    assert!(
        handles
            .iter()
            .all(|h| h.size == if h.id.pick == first { 1. } else { 2. })
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.edge_fillet_prompt.is_none());
    assert!(snapshot(&app, id).shares_storage_with(&joined));
    assert!(!app.document.can_undo());
}

#[test]
fn edge_fillet_edit_rejects_a_replaced_result() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    let id = pick.object;
    apply(&mut app, "FilletEdge", pick);
    submit(&mut app, &format!("FilletEdge Edit={id}"));
    submit(&mut app, "SetAll=2");
    let replacement = Geometry::Point(point(5., 5., 5.));
    app.document
        .replace_object_geometries([(id, replacement.clone())])
        .unwrap();
    app.document.clear_history().unwrap();
    submit(&mut app, "");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("The edge blend changed")
    );
    assert_eq!(&*snapshot(&app, id), &replacement);
    assert!(!app.document.can_undo());
}

#[test]
fn edge_fillet_edit_can_make_detached_surfaces_in_one_undo_step() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    let id = pick.object;
    let original = snapshot(&app, id);
    apply(&mut app, "FilletEdge", pick);
    let joined = snapshot(&app, id);
    app.document.clear_history().unwrap();
    submit(&mut app, &format!("FilletEdge Edit={id} TrimAndJoin=No"));
    submit(&mut app, "SetAll=2");
    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
    assert!(snapshot(&app, id).shares_storage_with(&original));
    let detached = app.document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(patch) = detached.geometry() else {
        panic!("expected a detached B-rep");
    };
    assert_eq!(patch.faces().len(), 1);
    assert!(detached.geometry_snapshot().edge_blend_recipe().is_none());
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
    assert!(app.document.undo().unwrap().is_some());
    assert_eq!(app.document.objects().count(), 1);
    assert!(snapshot(&app, id).shares_storage_with(&joined));
}
