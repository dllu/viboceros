use super::*;
use crate::viewport::ComponentClick;

fn pick(app: &mut VibocerosApp, edge: ComponentPick) {
    app.accept_component_click(ComponentClick {
        picks: vec![edge],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
}

fn picked(app: &VibocerosApp) -> Vec<ComponentPick> {
    app.component_selection
        .checked_picks(&app.document)
        .unwrap()
}

#[test]
fn edge_fillet_previous_selection_recalls_canceled_edges_and_updates_preview() {
    let mut app = test_app();
    let first = box_pick(&mut app);
    app.document.clear_history().unwrap();
    let original = app
        .document
        .object(first.object)
        .unwrap()
        .geometry()
        .clone();
    for (index, command) in ["FilletEdge", "ChamferEdge", "BlendEdge"]
        .into_iter()
        .enumerate()
    {
        let edge = ComponentPick { index, ..first };
        submit(&mut app, &format!("{command} 1"));
        submit(&mut app, "PreviousEdgeSelection");
        assert!(picked(&app).is_empty());
        pick(&mut app, edge);
        app.cancel_current_prompt_or_selection();
        assert!(picked(&app).is_empty());
        assert!(!app.document.can_undo());
    }
    for (index, command) in ["FilletEdge", "ChamferEdge", "BlendEdge"]
        .into_iter()
        .enumerate()
    {
        submit(
            &mut app,
            &format!("{command} 1 Preview=Yes _PreviousEdgeSelection"),
        );
        assert_eq!(picked(&app), [ComponentPick { index, ..first }]);
        assert!(!app.document.can_undo());
        let context = egui::Context::default();
        let start = std::time::Instant::now();
        let preview = loop {
            app.update_edge_blend_preview(&context);
            if !app.edge_blend_preview.curves().is_empty() {
                break app.edge_blend_preview.curves().to_vec();
            }
            assert!(start.elapsed().as_secs() < 15, "{:?}", app.command_log);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        submit(&mut app, "");
        let Geometry::Brep(rounded) = app.document.object(first.object).unwrap().geometry() else {
            panic!("expected the recalled edge blend");
        };
        assert_eq!(
            rounded
                .edges()
                .iter()
                .map(|edge| edge.curve())
                .collect::<Vec<_>>(),
            preview.iter().collect::<Vec<_>>(),
        );
        assert_eq!(app.document.undo_label(), Some(command));
        submit(&mut app, "Undo");
        assert_eq!(
            app.document.object(first.object).unwrap().geometry(),
            &original
        );
    }
}

#[test]
fn edge_fillet_previous_selection_keeps_twenty_distinct_sets_in_recency_order() {
    let mut app = test_app();
    let first = box_pick(&mut app);
    let second = box_pick(&mut app);
    app.document.clear_history().unwrap();
    let edges = (0..=20)
        .map(|i| ComponentPick {
            index: i % 12,
            ..if i < 12 { first } else { second }
        })
        .collect::<Vec<_>>();
    for i in (0..=20).chain([1]) {
        submit(&mut app, "ChamferEdge 1");
        pick(&mut app, edges[i]);
        submit(&mut app, "Cancel");
    }

    submit(&mut app, "ChamferEdge PreviousEdgeSelection");
    assert_eq!(picked(&app), [edges[1]]);
    submit(&mut app, "PreviousEdgeSelection=2");
    assert_eq!(picked(&app), [edges[20]]);
    submit(&mut app, "PreviousEdgeSelection 20");
    assert_eq!(picked(&app), [edges[2]]);
    submit(&mut app, "PreviousEdgeSelection=21");
    assert_eq!(picked(&app), [edges[2]]);
    assert!(!app.document.can_undo());
}

#[test]
fn edge_fillet_previous_selection_rejects_a_changed_source_until_undo_restores_it() {
    let mut app = test_app();
    let first = box_pick(&mut app);
    let second = box_pick(&mut app);
    app.document.clear_history().unwrap();
    submit(&mut app, "FilletEdge 1");
    pick(&mut app, first);
    pick(&mut app, second);
    submit(&mut app, "Cancel");
    app.document
        .replace_object_geometries([(first.object, Geometry::Point(point(1., 2., 3.)))])
        .unwrap();

    submit(&mut app, "FilletEdge");
    pick(&mut app, ComponentPick { index: 1, ..second });
    submit(&mut app, "PreviousEdgeSelection");
    assert_eq!(picked(&app), [ComponentPick { index: 1, ..second }]);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("No previous edge sets")
    );

    app.document.undo().unwrap();
    submit(&mut app, "PreviousEdgeSelection=1");
    let mut recalled = picked(&app);
    recalled.sort_unstable();
    let mut expected = [first, second];
    expected.sort_unstable();
    assert_eq!(recalled, expected);
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
}
