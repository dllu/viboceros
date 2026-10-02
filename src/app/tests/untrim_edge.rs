use super::*;
use crate::viewport::EdgePick;
use crate::viewport::{ComponentClick, ComponentPick};
use viboceros_command::ComponentSelectionKind;

fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn band(app: &mut VibocerosApp) -> EdgePick {
    let surface = viboceros_geometry::NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., 0.),
            point(10., 0., 0.),
            point(0., 10., 0.),
            point(10., 10., 0.),
        ],
        vec![0., 0., 10., 10.],
        vec![0., 0., 10., 10.],
    )
    .unwrap();
    let brep = viboceros_geometry::Brep::try_rectangular_surface_face(
        surface,
        2.0..=8.,
        0.0..=10.,
        app.document.tolerance(),
    )
    .unwrap();
    EdgePick {
        object: app.document.add_geometry(Geometry::Brep(brep)).unwrap(),
        edge: 1,
    }
}
fn objects(app: &VibocerosApp) -> Vec<viboceros_document::Object> {
    app.document.objects().cloned().collect()
}
#[test]
fn general_untrim_ignores_preselection_edits_immediately_and_keeps_escape_history() {
    let mut app = test_app();
    let pick = band(&mut app);
    let group = app.document.add_group(None, [pick.object]).unwrap();
    app.document.clear_history().unwrap();
    let before = objects(&app);
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: pick.object,
            kind: ComponentSelectionKind::BrepEdge,
            index: pick.edge,
        }],
        preselection: true,
        modifiers: egui::Modifiers::NONE,
    });
    submit(&mut app, "Untrim KeepTrimObjects=Yes");
    assert_eq!(objects(&app), before);
    assert!(
        app.component_selection
            .valid_picks(&app.document)
            .is_empty()
    );
    assert_eq!(app.hole_prompt.as_ref().unwrap().name(), "Untrim");
    app.accept_hole_edges(vec![pick]);
    let after = objects(&app);
    assert_ne!(after, before);
    assert_eq!(after.len(), 2);
    assert_eq!(
        app.document.object(pick.object).unwrap().group_ids(),
        &[group]
    );
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), before);
    app.accept_hole_edges(vec![pick]);
    let after = objects(&app);
    app.cancel_current_prompt_or_selection();
    assert!(app.hole_prompt.is_none());
    assert_eq!(objects(&app), after);
    submit(&mut app, "Undo");
    assert_eq!(objects(&app), before);
    submit(&mut app, "Redo");
    assert_eq!(objects(&app), after);
}
#[test]
fn general_options_are_independent_from_hole_options_and_all_similar_still_picks_edges() {
    let mut app = test_app();
    let pick = band(&mut app);
    submit(&mut app, "Untrim");
    submit(&mut app, "AllSimilar");
    assert!(!app.hole_prompt.as_ref().unwrap().picking_edges());
    submit(&mut app, "Yes");
    assert!(app.hole_prompt.as_ref().unwrap().picking_edges());
    assert!(!app.hole_prompt.as_ref().unwrap().picking_faces());
    submit(&mut app, "KeepTrimObjects=Yes");
    let options = app.hole_prompt.as_ref().unwrap().options;
    submit(&mut app, "AllSimilar=No MaximumEdgeLength=1");
    assert_eq!(app.hole_prompt.as_ref().unwrap().options, options);
    app.accept_hole_edges(vec![pick]);
    submit(&mut app, "");
    submit(&mut app, "UntrimHoles");
    assert!(!app.hole_prompt.as_ref().unwrap().options.keep_trim_objects);
    submit(&mut app, "");
    submit(&mut app, "Untrim");
    assert_eq!(app.hole_prompt.as_ref().unwrap().options, options);
}
