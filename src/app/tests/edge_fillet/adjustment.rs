use super::*;
use crate::app::tests::circle_preview::{screen_point, viewport_click, viewport_frame};
use crate::viewport::ComponentClick;
use crate::viewport::edge_blend::{HandleEnd, SizeHandleId};

#[test]
fn edge_fillet_adjustment_edits_viewport_handles_and_returns_to_select_edges() {
    let mut app = test_app();
    let source = box_pick(&mut app).object;
    let original = app.document.object(source).unwrap().geometry().clone();
    let Geometry::Brep(brep) = &original else {
        panic!("expected a box")
    };
    let choose = |y, z| ComponentPick {
        object: source,
        kind: ComponentSelectionKind::BrepEdge,
        index: brep
            .edges()
            .iter()
            .position(|edge| {
                edge.vertices().iter().all(|&vertex| {
                    let point = brep.vertices()[vertex].point();
                    point.y() == y && point.z() == z
                })
            })
            .unwrap(),
    };
    let first = choose(0., 0.);
    let second = choose(10., 10.);
    let start = SizeHandleId {
        pick: first,
        end: HandleEnd::Start,
    };
    let click = |app: &mut VibocerosApp, pick| {
        app.accept_component_click(ComponentClick {
            picks: vec![pick],
            preselection: false,
            modifiers: egui::Modifiers::NONE,
        })
    };
    let sizes = |app: &VibocerosApp| {
        let mut sizes = std::collections::BTreeMap::new();
        for handle in app.edge_blend_size_handles() {
            handle
                .id
                .end
                .set_size(sizes.entry(handle.id.pick).or_insert([0.; 2]), handle.size);
        }
        sizes
    };
    let handle_point = |app: &VibocerosApp| {
        app.edge_blend_size_handles()
            .into_iter()
            .find(|h| h.id == start)
            .unwrap()
            .point
    };
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Perspective);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    submit(&mut app, "ChamferEdge 1 RailType=RollingBall");
    click(&mut app, first);
    submit(&mut app, "");
    assert!(app.adjusting_edge_blend());
    assert_eq!(sizes(&app).get(&first), Some(&[1.; 2]));
    click(&mut app, second);
    assert_eq!(sizes(&app).len(), 1);

    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_bounding_box(brep.bounds()).unwrap();
    viewport_frame(&mut app, &context, vec![]);
    let pixel = screen_point(&app, handle_point(&app));
    viewport_click(&mut app, &context, pixel);
    assert!(app.command_log.back().unwrap().contains("Enter a size"));
    submit(&mut app, "2");
    assert_eq!(sizes(&app).get(&first), Some(&[2., 1.]));

    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    let pixel = screen_point(&app, handle_point(&app));
    let dragged = pixel - egui::vec2(0., 24.);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    viewport_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(pixel), button(pixel, true)],
    );
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(dragged)]);
    let dragged_size = sizes(&app)[&first];
    assert!(
        dragged_size[0] > 2. && dragged_size[1] == 1.,
        "{:?}",
        app.command_log
    );
    assert!(!app.document.can_undo());
    viewport_frame(&mut app, &context, vec![button(dragged, false)]);
    assert_eq!(sizes(&app).get(&first), Some(&dragged_size));
    assert!(app.pick_edge_size_handle(start));
    submit(&mut app, "2");

    submit(&mut app, "SelectEdges");
    assert!(!app.adjusting_edge_blend());
    click(&mut app, second);
    submit(&mut app, "");
    assert_eq!(sizes(&app), [(first, [2., 1.]), (second, [1.; 2])].into());
    submit(&mut app, "LinkHandles=Yes");
    assert!(app.pick_edge_size_handle(start));
    submit(&mut app, "1.25");
    assert_eq!(
        sizes(&app),
        [(first, [1.25; 2]), (second, [1.25; 2])].into()
    );
    submit(&mut app, "LinkHandles=No");
    submit(&mut app, "SetAll");
    submit(&mut app, "1.5");
    assert_eq!(sizes(&app), [(first, [1.5; 2]), (second, [1.5; 2])].into());
    assert!(app.pick_edge_size_handle(start));
    app.cancel_current_prompt_or_selection();
    assert!(app.adjusting_edge_blend());
    assert_eq!(sizes(&app), [(first, [1.5; 2]), (second, [1.5; 2])].into());
    assert!(app.pick_edge_size_handle(start));
    submit(&mut app, "FromTwoPoints");
    submit(&mut app, "w0,0,0");
    submit(&mut app, "w2,0,0");
    assert_eq!(sizes(&app), [(first, [2., 1.5]), (second, [1.5; 2])].into());
    assert!(!app.document.can_undo());
    assert_eq!(app.document.object(source).unwrap().geometry(), &original);

    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
    let Geometry::Brep(blended) = app.document.object(source).unwrap().geometry() else {
        panic!("expected chamfer geometry");
    };
    assert!(blended.is_solid());
    let curve = brep.edges()[first.index].curve();
    let ends = [*curve.domain().start(), *curve.domain().end()];
    for (t, size) in ends.into_iter().zip([2., 1.5]) {
        let x = curve.evaluate(t).unwrap().x();
        for [y, z] in [[size, 0.], [0., size]] {
            let expected = point(x, y, z);
            assert!(
                blended.vertices().iter().any(|vertex| vertex
                    .point()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-6)
            );
        }
    }
    assert_eq!(app.document.undo_label(), Some("ChamferEdge"));
    submit(&mut app, "Undo");
    assert_eq!(app.document.object(source).unwrap().geometry(), &original);
}
