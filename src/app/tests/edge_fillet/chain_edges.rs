use super::*;
use crate::app::tests::circle_preview::{screen_point, viewport_frame};
use crate::viewport::ComponentClick;
use viboceros_geometry::{Brep, TriangleMesh};

fn pick(app: &mut VibocerosApp, pick: ComponentPick) {
    app.accept_component_click(ComponentClick {
        picks: vec![pick],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
}

fn selected(app: &VibocerosApp) -> Vec<ComponentPick> {
    app.component_selection
        .checked_picks(&app.document)
        .unwrap()
}

#[test]
fn edge_fillet_chain_edges_uses_the_parent_picker_and_commits_its_preview() {
    for command in ["FilletEdge", "ChamferEdge", "BlendEdge"] {
        let mut app = test_app();
        let edge = box_pick(&mut app);
        let original = app.document.object(edge.object).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        submit(&mut app, &format!("{command} 1 Preview=Yes _ChainEdges"));
        assert!(app.picking_edge_blend_chain(), "{:?}", app.command_log);
        pick(&mut app, edge);
        assert_eq!(selected(&app), [edge]);
        let context = egui::Context::default();
        let start = std::time::Instant::now();
        let staged = loop {
            viewport_frame(&mut app, &context, vec![]);
            if !app.edge_blend_preview.curves().is_empty() {
                break app.edge_blend_preview.curves().to_vec();
            }
            assert!(start.elapsed().as_secs() < 15, "{:?}", app.command_log);
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        submit(&mut app, "_Enter");
        assert!(!app.picking_edge_blend_chain());
        assert!(app.edge_fillet_prompt.is_some());
        assert!(!app.document.can_undo());
        submit(&mut app, "");
        assert!(app.adjusting_edge_blend());
        submit(&mut app, "");
        let Geometry::Brep(rounded) = app.document.object(edge.object).unwrap().geometry() else {
            panic!("expected rounded geometry");
        };
        assert_eq!(
            rounded
                .edges()
                .iter()
                .map(|edge| edge.curve())
                .collect::<Vec<_>>(),
            staged.iter().collect::<Vec<_>>()
        );
        assert_eq!(app.document.undo_label(), Some(command));
        submit(&mut app, "Undo");
        assert_eq!(
            app.document.object(edge.object).unwrap().geometry(),
            &original
        );
    }
}

#[test]
fn edge_fillet_chain_edges_next_previews_only_one_branch_and_undo_releases_it() {
    let mut app = test_app();
    let edge = box_pick(&mut app);
    app.document.clear_history().unwrap();
    submit(&mut app, "FilletEdge 1");
    submit(&mut app, "ChainEdges Direction=Forward");
    pick(&mut app, edge);
    assert_eq!(selected(&app), [edge]);
    submit(&mut app, "ChainContinuity");
    submit(&mut app, "Position");
    submit(&mut app, "GapTolerance=0.01 AngleTolerance=1");
    submit(&mut app, "Next");
    let prompt = app.chain_prompt.as_ref().unwrap();
    assert!(prompt.session.choosing_branch());
    assert_eq!(prompt.highlights().len(), 1);
    assert_eq!(selected(&app), [edge]);
    submit(&mut app, "2");
    let picks = selected(&app);
    assert_eq!(picks.len(), 2);
    assert!(picks.contains(&edge));
    submit(&mut app, "Undo");
    assert_eq!(selected(&app), [edge]);
    submit(&mut app, "All");
    assert!(app.chain_prompt.as_ref().unwrap().highlights().len() > 1);
    submit(&mut app, "");
    assert!(app.picking_edge_blend_chain());
    assert!(selected(&app).len() > 2);
    app.cancel_current_prompt_or_selection();
    assert!(!app.picking_edge_blend_chain());
    assert!(app.edge_fillet_prompt.is_none());
    assert!(selected(&app).is_empty());
    assert!(!app.document.can_undo());
}

#[test]
fn edge_fillet_double_click_selects_two_tangent_edges_in_the_viewport() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    let points = [
        [0., 0., 0.],
        [5., 0., 0.],
        [10., 0., 0.],
        [10., 10., 0.],
        [0., 10., 0.],
        [0., 0., 10.],
        [10., 0., 10.],
        [10., 10., 10.],
        [0., 10., 10.],
    ]
    .map(|p| Point3::try_from(p).unwrap())
    .to_vec();
    let triangles = vec![
        [0, 4, 1],
        [4, 3, 1],
        [1, 3, 2],
        [5, 6, 7],
        [5, 7, 8],
        [0, 1, 5],
        [1, 6, 5],
        [1, 2, 6],
        [2, 3, 7],
        [2, 7, 6],
        [3, 4, 8],
        [3, 8, 7],
        [4, 0, 5],
        [4, 5, 8],
    ];
    let mesh = TriangleMesh::try_new(points, triangles, tolerance).unwrap();
    let brep = Brep::try_from_mesh(&mesh, true, tolerance).unwrap();
    let bounds = brep.bounds();
    let edges = brep
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            edge.vertices()
                .iter()
                .all(|&vertex| {
                    let point = brep.vertices()[vertex].point();
                    point.y() == 0. && point.z() == 0.
                })
                .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 2);
    let object = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Perspective);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    submit(&mut app, "FilletEdge 1");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_bounding_box(bounds).unwrap();
    let station = screen_point(&app, point(2.5, 0., 0.));
    let mut time = context.input(|i| i.time);
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
                            egui::Event::PointerMoved(station),
                            egui::Event::PointerButton {
                                pos: station,
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
            context.input(|i| i
                .pointer
                .button_double_clicked(egui::PointerButton::Primary)),
            double
        );
        if !double {
            assert_eq!(selected(&app).len(), 1, "{:?}", app.command_log);
        }
    }
    assert_eq!(
        selected(&app),
        edges
            .into_iter()
            .map(|index| ComponentPick {
                object,
                index,
                kind: ComponentSelectionKind::BrepEdge,
            })
            .collect::<Vec<_>>(),
        "{:?}",
        app.command_log
    );
    assert!(!app.picking_edge_blend_chain());
}
