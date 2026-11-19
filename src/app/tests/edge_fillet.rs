use super::*;
use crate::viewport::{ComponentPick, ComponentWindow};
use viboceros_command::ComponentSelectionKind;
fn submit(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn box_pick(app: &mut VibocerosApp) -> ComponentPick {
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 10.]; 3],
        app.document.tolerance(),
    )
    .unwrap();
    ComponentPick {
        object: app.document.add_geometry(Geometry::Brep(brep)).unwrap(),
        kind: ComponentSelectionKind::BrepEdge,
        index: 0,
    }
}
#[test]
fn command_first_keeps_geometry_until_confirmation_and_escape_cancels() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    let original = app.document.object(pick.object).unwrap().geometry().clone();
    submit(&mut app, "FilletEdge Radius=1");
    assert!(app.edge_fillet_prompt.is_some());
    assert!(!app.component_preselection_available());
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: egui::Modifiers::NONE,
        preselection: false,
        crossing: false,
        inverted: false,
    });
    submit(&mut app, "Radius=0.5");
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &original
    );
    submit(&mut app, "Cancel");
    assert!(app.edge_fillet_prompt.is_none());
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &original
    );
}
#[test]
fn picked_edge_confirmation_creates_one_undoable_fillet() {
    let mut app = test_app();
    let pick = box_pick(&mut app);
    app.document.clear_history().unwrap();
    let before = app.document.object(pick.object).unwrap().geometry().clone();
    submit(&mut app, "FilletEdge 1");
    app.accept_component_window(ComponentWindow {
        picks: vec![pick],
        modifiers: egui::Modifiers::NONE,
        preselection: false,
        crossing: false,
        inverted: false,
    });
    submit(&mut app, "");
    assert!(app.edge_fillet_prompt.is_none());
    let Geometry::Brep(result) = app.document.object(pick.object).unwrap().geometry() else {
        panic!()
    };
    assert!(result.is_solid());
    assert_eq!(result.faces().len(), 7);
    assert_eq!(app.document.undo_label(), Some("FilletEdge"));
    submit(&mut app, "Undo");
    assert_eq!(
        app.document.object(pick.object).unwrap().geometry(),
        &before
    );
}

#[test]
fn edge_fillet_preview_tracks_options_and_commits_the_displayed_geometry() {
    use super::circle_preview::viewport_frame;
    use std::time::{Duration, Instant};

    for (command, parameter) in [
        ("FilletEdge", "Radius"),
        ("ChamferEdge", "Distance"),
        ("BlendEdge", "Radius"),
    ] {
        let mut app = test_app();
        let pick = box_pick(&mut app);
        let original = app.document.object(pick.object).unwrap().geometry().clone();
        app.document.clear_history().unwrap();
        let context = egui::Context::default();
        submit(&mut app, &format!("{command} {parameter}=1 Preview=Yes"));
        assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
        app.accept_component_window(ComponentWindow {
            picks: vec![pick],
            modifiers: egui::Modifiers::NONE,
            preselection: false,
            crossing: false,
            inverted: false,
        });

        let wait = |app: &mut VibocerosApp| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let strokes = viewport_frame(app, &context, Vec::new());
                if !strokes.is_empty() && !app.edge_blend_preview.curves().is_empty() {
                    return app.edge_blend_preview.curves().to_vec();
                }
                assert!(
                    Instant::now() < deadline,
                    "{command}: {:?}",
                    app.command_log
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let first = wait(&mut app);
        if command == "FilletEdge" {
            submit(&mut app, "Radius=100");
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
                if app
                    .command_log
                    .back()
                    .unwrap()
                    .starts_with("Preview unavailable.")
                {
                    break;
                }
                assert!(Instant::now() < deadline, "{:?}", app.command_log);
                std::thread::sleep(Duration::from_millis(5));
            }
            submit(&mut app, "");
            assert!(app.edge_fillet_prompt.is_some());
            assert!(!app.document.can_undo());
        }
        submit(&mut app, "Preview=No");
        assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
        submit(&mut app, &format!("Preview {parameter}=2"));
        let edited = wait(&mut app);
        assert_ne!(first, edited);
        assert_eq!(
            app.document.object(pick.object).unwrap().geometry(),
            &original
        );
        assert!(!app.document.can_undo());

        submit(&mut app, "");
        assert!(app.edge_fillet_prompt.is_none(), "{:?}", app.command_log);
        let Geometry::Brep(result) = app.document.object(pick.object).unwrap().geometry() else {
            panic!("expected a staged edge blend");
        };
        assert_eq!(
            result
                .edges()
                .iter()
                .map(|edge| edge.curve())
                .collect::<Vec<_>>(),
            edited.iter().collect::<Vec<_>>()
        );
        assert_eq!(app.document.undo_label(), Some(command));
        submit(&mut app, "Undo");
        assert_eq!(
            app.document.object(pick.object).unwrap().geometry(),
            &original
        );
        assert!(viewport_frame(&mut app, &context, Vec::new()).is_empty());
    }
}
