use super::*;
use crate::viewport::ComponentClick;
use std::collections::BTreeSet;

#[test]
fn edge_fillet_face_edges_selects_viewport_faces_and_restores_prior_edges_on_escape() {
    use crate::app::tests::circle_preview::{screen_point, viewport_click, viewport_frame};

    let mut app = test_app();
    let source = box_pick(&mut app).object;
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);

    let original = app.document.object(source).unwrap().geometry().clone();
    let Geometry::Brep(brep) = &original else {
        panic!("expected a box");
    };
    let edges_at = |z| {
        brep.edges()
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.vertices()
                    .iter()
                    .all(|&i| brep.vertices()[i].point().z() == z)
            })
            .map(|(index, _)| ComponentPick {
                object: source,
                index,
                kind: ComponentSelectionKind::BrepEdge,
            })
            .collect::<BTreeSet<_>>()
    };
    let prior = *edges_at(0.).first().unwrap();
    let top = edges_at(10.);
    assert_eq!(top.len(), 4);
    let picks = |app: &VibocerosApp| {
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>()
    };
    submit(&mut app, "FilletEdge Radius=1");
    app.accept_component_click(ComponentClick {
        picks: vec![prior],
        preselection: false,
        modifiers: Default::default(),
    });
    assert_eq!(picks(&app), BTreeSet::from([prior]));

    let context = egui::Context::default();
    submit(&mut app, "FaceEdges");
    viewport_frame(&mut app, &context, vec![]);
    viewport_frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_bounding_box(brep.bounds()).unwrap();
    let face = screen_point(&app, point(5., 5., 10.));
    viewport_click(&mut app, &context, face);
    assert!(app.picking_edge_blend_faces());
    assert_eq!(picks(&app), top.iter().copied().chain([prior]).collect());
    app.cancel_current_prompt_or_selection();
    assert!(!app.picking_edge_blend_faces());
    assert!(app.edge_fillet_prompt.is_some());
    assert_eq!(picks(&app), BTreeSet::from([prior]));

    submit(&mut app, "FaceEdges");
    viewport_click(&mut app, &context, face);
    viewport_click(&mut app, &context, face);
    submit(&mut app, "");
    assert!(!app.picking_edge_blend_faces());
    assert!(app.edge_fillet_prompt.is_some());
    assert_eq!(picks(&app), top.into_iter().chain([prior]).collect());
    assert_eq!(app.document.object(source).unwrap().geometry(), &original);
    assert!(!app.document.can_undo());
}

#[test]
fn edge_fillet_face_edges_can_start_each_command_and_omits_cylinder_seams() {
    for command in ["FilletEdge", "ChamferEdge", "BlendEdge"] {
        let mut app = test_app();
        let cylinder = viboceros_geometry::Brep::try_cylinder(
            viboceros_command::CommandContext::default().construction_plane,
            5.,
            0.,
            10.,
            app.document.tolerance(),
        )
        .unwrap();
        let object = app.document.add_geometry(Geometry::Brep(cylinder)).unwrap();

        submit(&mut app, &format!("{command} 1 _FaceEdges"));
        assert!(app.picking_edge_blend_faces(), "{:?}", app.command_log);
        app.accept_component_window(ComponentWindow {
            picks: [0, 2]
                .map(|index| ComponentPick {
                    object,
                    kind: ComponentSelectionKind::BrepFace,
                    index,
                })
                .to_vec(),
            modifiers: egui::Modifiers::NONE,
            preselection: false,
            crossing: false,
            inverted: false,
        });
        submit(&mut app, "_Enter");
        assert!(!app.picking_edge_blend_faces());
        assert!(app.edge_fillet_prompt.is_some());
        assert_eq!(
            app.component_selection
                .checked_picks(&app.document)
                .unwrap(),
            [0, 1].map(|index| ComponentPick {
                object,
                kind: ComponentSelectionKind::BrepEdge,
                index,
            }),
            "{:?}",
            app.command_log,
        );
    }
}
