use super::*;
use std::collections::BTreeSet;
use viboceros_command::chain_selection::Target;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn fixture(scene: &str) -> (VibocerosApp, Vec<ObjectId>) {
    fixture_with_tolerance(scene, Tolerance::DEFAULT)
}
fn fixture_with_tolerance(scene: &str, tolerance: Tolerance) -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    app.document = Document::new(tolerance);
    let ids = viboceros_oracle::chain_selection::source(scene, app.document.tolerance())
        .unwrap()
        .into_iter()
        .map(|g| app.document.add_geometry(g).unwrap())
        .collect();
    app.document.clear_history().unwrap();
    (app, ids)
}
#[test]
fn mixed_curve_edge_chain_getters_preserve_redo_and_cancel_restores_original_selection() {
    for scene in ["lines", "reversed", "corner", "surface", "mixed"] {
        for cancel in [false, true] {
            let (mut app, ids) = fixture(scene);
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            let baseline = ids.last().copied().unwrap();
            app.document
                .select_objects_direct([baseline], SelectionMode::Add)
                .unwrap();
            enter(&mut app, "SelChain");
            assert!(app.chain_prompt.is_some());
            assert!(!app.command_line_idle());
            let target = if scene == "surface" || scene == "mixed" {
                Target::Edge(ids[0], 0)
            } else {
                Target::Curve(ids[1])
            };
            app.pick_chain(vec![target]);
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
            if cancel {
                app.cancel_current_prompt_or_selection();
                assert_eq!(
                    app.document.selected_object_ids().collect::<Vec<_>>(),
                    [baseline]
                );
                assert!(app.commands.component_selection().is_empty());
            } else {
                enter(&mut app, "");
                assert!(app.document.is_selected(baseline));
            }
            assert!(app.chain_prompt.is_none());
            assert!(app.document.can_redo());
        }
    }
}
#[test]
fn chain_ambiguity_options_and_stale_sources_are_explicit_and_atomic() {
    let (mut app, ids) = fixture("corner");
    enter(&mut app, "SelChain");
    app.pick_chain(vec![Target::Curve(ids[0]), Target::Curve(ids[1])]);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "1");
    assert!(app.document.is_selected(ids[0]));
    enter(&mut app, "GapTolerance=-1");
    assert_eq!(app.document.selected_object_count(), 1);
    enter(&mut app, "ChainContinuity");
    enter(&mut app, "Position");
    enter(&mut app, "All");
    assert_eq!(app.document.selected_object_count(), 3);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 2);
    let point = point(2., 3., 4.);
    app.document
        .replace_object_geometries([(ids[1], Geometry::Point(point))])
        .unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.validate_chain();
    assert!(app.chain_prompt.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(app.command_log.back().unwrap().contains("changed"));
}
#[path = "chain_selection/viewport.rs"]
mod viewport;

#[path = "../../../crates/viboceros-oracle/tests/support/chain_selection.rs"]
mod native;
#[test]
fn native_chain_states_replay_through_command_input_and_viewport_actions() {
    use std::io::Read;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "discovery",
        "reversed",
        "lines",
        "corner",
        "cycle",
        "surface",
        "mixed",
        "tolerance",
        "lifecycle",
        "curvature",
        "curvature_limits",
        "curvature_boundary",
        "acceptance",
        "branch",
        "prefer_curve",
        "forks",
        "angle_precision",
        "option_minimums",
        "option_boundary",
    ] {
        let name = format!("chain_selection_{family}");
        let request: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{name}.json"))).unwrap(),
        )
        .unwrap();
        let path = base.join(format!("observations/{name}.json"));
        let bytes = if path.exists() {
            std::fs::read(path).unwrap()
        } else {
            let bytes = std::fs::read(path.with_extension("json.gz")).unwrap();
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(bytes.as_slice())
                .read_to_end(&mut raw)
                .unwrap();
            raw
        };
        let observed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(observed["results"].as_array().unwrap())
        {
            count += 1;
            let v = &row["value"];
            let t = &request["tolerance"];
            let tolerance = Tolerance::try_new(
                t["absolute"].as_f64().unwrap(),
                t["relative"].as_f64().unwrap(),
                t["angular"].as_f64().unwrap(),
            )
            .unwrap();
            let (mut app, ids) = fixture_with_tolerance(op["scene"].as_str().unwrap(), tolerance);
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            app.document
                .select_objects_direct(
                    op["pre"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| ids[i.as_u64().unwrap() as usize]),
                    SelectionMode::Add,
                )
                .unwrap();
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            let check = |app: &VibocerosApp, state: &serde_json::Value| {
                let mut components = app
                    .commands
                    .component_selection()
                    .checked_picks(&app.document)
                    .unwrap()
                    .iter()
                    .map(|p| (ids.iter().position(|&id| id == p.object).unwrap(), p.index))
                    .collect::<Vec<_>>();
                components.sort();
                components.dedup();
                let actual = serde_json::json!({"whole":ids.iter().enumerate().filter_map(|(i,&id)|app.document.is_selected(id).then_some(i)).collect::<Vec<_>>(),"objects":app.document.objects().len(),"components":components.into_iter().map(|(object,index)|serde_json::json!({"object":object,"index":index,"kind":"BrepEdge"})).collect::<Vec<_>>()});
                assert_eq!(actual, native::normalize(state), "{}", row["id"]);
            };
            check(&app, &v["before"]);
            let mut input = "SelChain".to_owned();
            if let Some(options) = op["options"].as_object() {
                for (key, value) in options {
                    let key = match key.as_str() {
                        "continuity" => "ChainContinuity",
                        "direction" => "Direction",
                        "gap" => "GapTolerance",
                        "angle" => "AngleTolerance",
                        _ => panic!(),
                    };
                    input.push_str(&format!(" {key}={}", value.as_str().unwrap()));
                }
            }
            enter(&mut app, &input);
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
                    enter(&mut app, step["value"].as_str().unwrap());
                }
                check(&app, state);
            }
            if app.chain_prompt.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
            }
            check(&app, &v["after"]);
            assert_eq!(
                app.document.objects().cloned().collect::<Vec<_>>(),
                original
            );
            assert!(app.document.can_redo());
            enter(&mut app, "Redo");
            check(&app, &v["redo"]);
            enter(&mut app, "Undo");
            check(&app, &v["undo"]);
        }
    }
    assert_eq!(count, 144);
}
