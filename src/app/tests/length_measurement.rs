use super::chain_selection::native as chain_native;
use super::*;
use serde_json::{Value, json};
use viboceros_command::chain_selection::Target;
use viboceros_command::component_selection::{ComponentPick, ComponentSelectionKind};
#[path = "../../../crates/viboceros-oracle/tests/support/length_chain.rs"]
mod native;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture(scene: &str) -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    app.document
        .set_units(viboceros_geometry::LengthUnitSystem::Millimeters, false)
        .unwrap();
    let ids = viboceros_oracle::chain_selection::source(scene, app.document.tolerance())
        .unwrap()
        .into_iter()
        .map(|g| app.document.add_geometry(g).unwrap())
        .collect();
    app.document.clear_history().unwrap();
    (app, ids)
}
fn phase(app: &VibocerosApp) -> &'static str {
    if let Some(p) = &app.chain_prompt {
        if p.session.choosing_branch() {
            "branch"
        } else if p.session.has_seed() {
            "connected"
        } else {
            "seed"
        }
    } else if app.object_prompt.is_none() {
        "idle"
    } else if app.length_units_pending {
        "units"
    } else {
        "parent"
    }
}
fn snapshot(app: &VibocerosApp, ids: &[ObjectId]) -> Value {
    let mut components = app
        .commands
        .component_selection()
        .checked_picks(&app.document)
        .unwrap()
        .into_iter()
        .map(|p| (ids.iter().position(|&id| id == p.object).unwrap(), p.index))
        .collect::<Vec<_>>();
    components.sort_unstable();
    components.dedup();
    json!({"objects":app.document.objects().len(),"whole":ids.iter().enumerate().filter_map(|(i,&id)|app.document.is_selected(id).then_some(i)).collect::<Vec<_>>(),"components":components.into_iter().map(|(object,index)|json!({"object":object,"index":index,"kind":"BrepEdge"})).collect::<Vec<_>>(),"phase":phase(app)})
}
fn report_mm(app: &VibocerosApp) -> Option<f64> {
    app.command_log.iter().rev().find_map(|line| {
        let (_, s) = line.split_once("total length ")?;
        let value = s.split_whitespace().next()?.parse::<f64>().ok()?;
        Some(
            value
                * if s.to_ascii_lowercase().contains("centimet") {
                    10.
                } else {
                    1.
                },
        )
    })
}
#[test]
fn native_nested_chain_getters_lengths_and_cleanup_replay_through_the_app() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "length_chain_discovery",
        "length_chain_controls",
        "length_chain_followups",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let observed = native::observation(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            count += 1;
            let v = &row["value"];
            let (mut app, ids) = fixture(op["scene"].as_str().unwrap());
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            let check = |app: &VibocerosApp, s: &Value| {
                assert_eq!(snapshot(app, &ids), *s, "{}", row["id"])
            };
            check(&app, &native::normalize(&v["before"]));
            enter(&mut app, "Length");
            let continuity = op["options"]["continuity"].as_str().unwrap();
            enter(&mut app, &format!("SelChain ChainContinuity={continuity}"));
            assert!(app.object_prompt.is_some());
            assert!(app.chain_prompt.is_some());
            for (step, state) in op["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(v["trace"].as_array().unwrap())
            {
                if step["kind"] == "click" {
                    let id =
                        ids[step.get("source").unwrap_or(&op["seed"]).as_u64().unwrap() as usize];
                    let target = if app
                        .document
                        .object(id)
                        .unwrap()
                        .geometry()
                        .curve_ref()
                        .is_some()
                    {
                        Target::Curve(id)
                    } else {
                        Target::Edge(id, 0)
                    };
                    app.handle_viewport_action(crate::viewport::ViewportOutput {
                        chain_click: Some(vec![target]),
                        ..Default::default()
                    });
                } else {
                    match step["value"].as_str().unwrap() {
                        "Escape" => app.cancel_current_prompt_or_selection(),
                        "Enter" => enter(&mut app, ""),
                        value => enter(&mut app, value),
                    }
                }
                check(&app, &native::normalize_trace(state, &v["after"]));
            }
            if app.object_prompt.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            check(&app, &native::normalize(&v["after"]));
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(app.document.can_redo());
            assert_eq!(report_mm(&app).is_some(), v["success"].as_bool().unwrap());
            if let Some((value, epsilon)) = native::length_mm(v["history"].as_str().unwrap()) {
                assert!(
                    (report_mm(&app).unwrap() - value).abs() <= epsilon,
                    "{}",
                    row["id"]
                );
            }
            enter(&mut app, "Redo");
            check(&app, &native::normalize(&v["redo"]));
            enter(&mut app, "Undo");
            check(&app, &native::normalize(&v["undo"]));
        }
    }
    assert_eq!(count, 17);
}
#[test]
fn parent_length_pointer_inputs_measure_curves_and_surface_edges_in_every_display_mode() {
    use super::chain_selection::viewport::Pointer;
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        for (scene, station) in [
            ("lines", point(4.5, 0., 0.)),
            ("surface", point(1.5, 0., 0.)),
        ] {
            let (mut app, _) = fixture(scene);
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            enter(&mut app, "Length");
            Pointer::new(mode).click(&mut app, station);
            assert!(app.chain_prompt.is_none());
            assert!(app.object_prompt.is_some());
            enter(&mut app, "");
            assert_eq!(report_mm(&app), Some(3.));
            assert_eq!(app.document.selected_object_count(), 0);
            assert!(app.commands.component_selection().is_empty());
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
        }
    }
}
#[test]
fn length_edge_preselection_and_changed_overlap_choices_preserve_history() {
    let (mut app, ids) = fixture("surface");
    let pick = ComponentPick {
        object: ids[0],
        index: 0,
        kind: ComponentSelectionKind::BrepEdge,
    };
    app.commands
        .component_selection()
        .replace(&app.document, vec![pick])
        .unwrap();
    enter(&mut app, "Length");
    assert!(app.object_prompt.is_none());
    assert_eq!(report_mm(&app), Some(3.));
    assert!(app.commands.component_selection().contains(&pick));
    app.commands.component_selection().clear();
    enter(&mut app, "Length");
    app.pick_length_inputs(
        vec![Target::Edge(ids[0], 0), Target::Edge(ids[0], 1)],
        SelectionMode::Add,
    );
    enter(&mut app, "99");
    assert!(app.length_choices.is_some());
    assert!(app.commands.component_selection().is_empty());
    app.document
        .replace_object_geometries([(ids[0], Geometry::Point(point(0., 0., 0.)))])
        .unwrap();
    let label = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "1");
    assert!(app.length_choices.is_none());
    assert!(app.commands.component_selection().is_empty());
    assert_eq!(app.document.undo_label(), label.as_deref());
    app.cancel_current_prompt_or_selection();
    assert!(app.object_prompt.is_none());
}
#[test]
fn length_parent_keeps_window_selection_and_control_click_removal() {
    use super::chain_selection::viewport::Pointer;
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let (mut app, _) = fixture("lines");
        enter(&mut app, "Length");
        Pointer::new(mode).window(&mut app, point(-1., 1., 0.), point(10., -1., 0.));
        assert_eq!(app.document.selected_object_count(), 3);
        enter(&mut app, "");
        assert_eq!(report_mm(&app), Some(9.));
        let (mut app, _) = fixture("surface");
        enter(&mut app, "Length");
        let mut pointer = Pointer::new(mode);
        pointer.click(&mut app, point(1.5, 0., 0.));
        assert_eq!(app.commands.component_selection().len(), 1);
        pointer.click_with_modifiers(&mut app, point(1.5, 0., 0.), egui::Modifiers::COMMAND);
        assert!(app.commands.component_selection().is_empty());
        assert!(app.object_prompt.is_some());
        app.cancel_current_prompt_or_selection();
    }
}
