use super::*;
use viboceros_command::chain_selection::Target;
use viboceros_geometry::{Brep, NurbsSurface};

#[test]
fn sweep_two_rail_chains_pick_loose_curves_and_brep_edges_without_model_edits() {
    let mut app = test_app();
    let tolerance = app.document.tolerance();
    for text in [
        "Line 0,0,0 0,1,2.5",
        "Line 0,0,5 0,1,2.5",
        "Line 0,0,0 2,0,0",
        "Line 0,0,5 2,0,5",
    ] {
        enter(&mut app, text);
    }
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let targets = [(0., 0., 1., 2.5), (1., 2.5, 0., 5.)].map(|(ay, az, by, bz)| {
        let surface = NurbsSurface::try_bilinear([
            point(2., ay, az),
            point(3., ay, az),
            point(3., by, bz),
            point(2., by, bz),
        ])
        .unwrap();
        let brep = Brep::try_surface_face(surface, tolerance).unwrap();
        let index = brep
            .edges()
            .iter()
            .position(|edge| {
                edge.curve()
                    .control_points()
                    .iter()
                    .all(|control| control.point().x() == 2.)
            })
            .unwrap();
        let id = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
        Target::Edge(id, index)
    });
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();

    enter(&mut app, "Sweep2");
    enter(&mut app, "ChainEdges AutoChain=No");
    assert!(app.command_hint().starts_with("Pick rail segments"));
    viewport_frame(&mut app, &context, vec![]);
    let pick = screen_point(&app, point(0., 0.5, 3.75));
    viewport_click(&mut app, &context, pick);
    assert_eq!(
        app.chain_prompt.as_ref().unwrap().highlights(),
        &[Target::Curve(ids[1])]
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, &ids[0].to_string());
    assert_eq!(app.chain_prompt.as_ref().unwrap().highlights().len(), 2);
    enter(&mut app, "Undo");
    assert_eq!(
        app.chain_prompt.as_ref().unwrap().highlights(),
        &[Target::Curve(ids[1])]
    );
    enter(&mut app, &ids[0].to_string());
    enter(&mut app, "");
    assert!(app.chain_prompt.as_ref().unwrap().sweep_rail());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1], ids[0]]
    );
    enter(&mut app, &ids[0].to_string());
    assert!(app.chain_prompt.as_ref().unwrap().highlights().is_empty());
    for (z, target) in [1.25, 3.75].into_iter().zip(targets) {
        let pick = screen_point(&app, point(2., 0.5, z));
        viewport_click(&mut app, &context, pick);
        assert!(
            app.chain_prompt
                .as_ref()
                .unwrap()
                .highlights()
                .contains(&target)
        );
    }
    assert!(
        app.component_selection
            .valid_picks(&app.document)
            .is_empty()
    );
    enter(&mut app, "");
    assert!(app.chain_prompt.is_none());
    for id in &ids[2..] {
        enter(&mut app, &id.to_string());
    }
    enter(&mut app, "");
    let wires = wait_for_draft(&mut app, &context);
    assert_eq!(app.document.undo_label(), None);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    enter(&mut app, "");

    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Expected the chained Sweep2 surface");
    };
    assert_eq!((brep.faces().len(), brep.edges().len()), (2, 7));
    assert_eq!(brep.wireframe_curves(1, tolerance).unwrap(), wires);
    assert_eq!(app.document.undo_label(), Some("Sweep2"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn sweep_two_rail_chain_can_autogrow_or_cancel_with_its_parent_intact() {
    let mut app = test_app();
    for text in ["Line 0,0,0 0,0,2", "Line 0,0,4 0,0,2", "Line 3,0,0 3,0,4"] {
        enter(&mut app, text);
    }
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "Sweep2 ChainEdges");
    enter(&mut app, &ids[0].to_string());
    assert_eq!(app.chain_prompt.as_ref().unwrap().highlights().len(), 2);
    app.cancel_current_prompt_or_selection();
    assert!(app.chain_prompt.is_none());
    assert!(app.command_hint().contains("first rail"));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "ChainEdges AutoChain=No");
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let pixel = screen_point(&app, point(0., 0., 3.));
    viewport_click(&mut app, &context, pixel);
    assert_eq!(app.chain_prompt.as_ref().unwrap().highlights().len(), 1);
    viewport_click(&mut app, &context, pixel);
    assert_eq!(app.chain_prompt.as_ref().unwrap().highlights().len(), 2);
    enter(&mut app, "");
    assert_eq!(app.document.selected_object_count(), 2);
    enter(&mut app, "Cancel");
    enter(&mut app, "Undo");
    assert!(app.command_hint().contains("first rail"));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Cancel");
    assert!(app.command_line_idle());
    assert_eq!(app.document.undo_label(), None);
}
