use super::*;
use crate::viewport::ComponentClick;

#[test]
fn edge_fillet_next_size_is_captured_for_each_pick() {
    for (command, option, rail) in [
        ("FilletEdge", "NextRadius", ""),
        ("ChamferEdge", "NextChamferDistance", "RailType=RollingBall"),
        ("BlendEdge", "NextRadius", "RailType=RollingBall"),
    ] {
        let mut app = test_app();
        let first = box_pick(&mut app);
        let original = app
            .document
            .object(first.object)
            .unwrap()
            .geometry()
            .clone();
        let Geometry::Brep(brep) = &original else {
            panic!("expected box")
        };
        let opposite = brep
            .edges()
            .iter()
            .position(|edge| {
                edge.vertices().iter().all(|&vertex| {
                    let p = brep.vertices()[vertex].point();
                    p.y() == 10. && p.z() == 10.
                })
            })
            .unwrap();
        let second = ComponentPick {
            index: opposite,
            ..first
        };
        let click = |app: &mut VibocerosApp, pick, modifiers| {
            app.accept_component_click(ComponentClick {
                picks: vec![pick],
                preselection: false,
                modifiers,
            });
        };
        let sizes = |app: &VibocerosApp| {
            let handles = app.edge_blend_size_handles();
            [first, second].map(|pick| {
                handles
                    .iter()
                    .filter(|h| h.id.pick == pick)
                    .map(|h| h.size)
                    .collect::<Vec<_>>()
            })
        };

        app.document.clear_history().unwrap();
        submit(
            &mut app,
            &format!("{command} {option}=1 {rail} TrimAndJoin=No"),
        );
        click(&mut app, first, egui::Modifiers::NONE);
        submit(&mut app, option);
        app.cancel_current_prompt_or_selection();
        assert!(app.edge_fillet_prompt.is_some());
        assert!(!app.adjusting_edge_blend());
        submit(&mut app, option);
        submit(&mut app, "2mm");
        app.accept_component_window(ComponentWindow {
            picks: vec![first, second],
            modifiers: egui::Modifiers::NONE,
            preselection: false,
            crossing: false,
            inverted: false,
        });
        submit(&mut app, &format!("_{option} 3"));
        submit(&mut app, "");
        assert_eq!(sizes(&app), [vec![1.; 2], vec![2.; 2]], "{command}");

        submit(&mut app, "SetAll 1.5");
        assert_eq!(sizes(&app), [vec![1.5; 2], vec![1.5; 2]], "{command}");
        submit(&mut app, "SelectEdges");
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains(&format!("{option}=1.5"))
        );
        submit(&mut app, &format!("{option}=3"));
        click(&mut app, second, egui::Modifiers::CTRL);
        click(&mut app, second, egui::Modifiers::NONE);
        submit(&mut app, "");
        assert_eq!(sizes(&app), [vec![1.5; 2], vec![3.; 2]], "{command}");
        assert!(!app.document.can_undo());

        submit(&mut app, "");
        assert!(
            app.edge_fillet_prompt.is_none(),
            "{command}: {:?}",
            app.command_log
        );
        assert_eq!(
            app.document.object(first.object).unwrap().geometry(),
            &original
        );
        let mut setbacks = app
            .document
            .objects()
            .filter(|object| object.id() != first.object)
            .map(|object| {
                let Geometry::Brep(patch) = object.geometry() else {
                    panic!("expected patch")
                };
                let bounds = viboceros_geometry::BoundingBox3::from_points(
                    patch.vertices().iter().map(|vertex| vertex.point()),
                )
                .unwrap();
                bounds.max().y() - bounds.min().y()
            })
            .collect::<Vec<_>>();
        setbacks.sort_by(f64::total_cmp);
        assert_eq!(setbacks.len(), 2);
        for (measured, expected) in setbacks.into_iter().zip([1.5, 3.]) {
            assert!((measured - expected).abs() < 1e-6, "{command}: {measured}");
        }
    }
}

#[test]
fn edge_fillet_startup_next_size_preserves_preselection_size() {
    let mut app = test_app();
    let preselected = box_pick(&mut app);
    let next = box_pick(&mut app);
    app.accept_component_click(ComponentClick {
        picks: vec![preselected],
        preselection: true,
        modifiers: egui::Modifiers::NONE,
    });
    submit(&mut app, "FilletEdge NextRadius=2");
    app.accept_component_click(ComponentClick {
        picks: vec![next],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    submit(&mut app, "");
    let handles = app.edge_blend_size_handles();
    assert_eq!(handles.len(), 4);
    for handle in handles {
        assert_eq!(
            handle.size,
            if handle.id.pick == preselected {
                1.
            } else {
                2.
            }
        );
    }
}
