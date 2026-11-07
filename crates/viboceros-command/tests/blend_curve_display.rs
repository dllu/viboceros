use std::sync::Arc;
use viboceros_command::{CommandRegistry, blend_curve::Input, curvature_graph::Options};
use viboceros_document::Document;
use viboceros_geometry::Point3;
#[path = "support/blend_curve.rs"]
#[allow(dead_code)]
mod blend;
#[path = "support/blend_curve_display.rs"]
mod display;
#[path = "support/blend_curve_edges.rs"]
#[allow(dead_code)]
mod edges;
#[path = "support/blend_curve_empty.rs"]
#[allow(dead_code)]
mod empty;
#[path = "support/point_grid_input.rs"]
#[allow(dead_code)]
mod grid;
#[path = "support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/blend_curve_measurements.rs"]
#[allow(dead_code)]
mod measurements;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/blend_curve_points.rs"]
#[allow(dead_code)]
mod points;
#[path = "support/curve_portion_input.rs"]
#[allow(dead_code)]
mod portion;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;
#[path = "support/command_file.rs"]
mod replay;

fn scene() -> (Document, CommandRegistry, Input) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    for command in ["Line -5,0,0 0,0,0", "Line 10,3,0 15,3,0"] {
        registry.execute(&mut doc, command).unwrap();
    }
    doc.clear_history().unwrap();
    let mut input = Input::new(doc.tolerance());
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    input
        .pick_curve(&doc, ids[0], None, Point3::try_new(0., 0., 0.).unwrap())
        .unwrap();
    input
        .pick_curve(&doc, ids[1], None, Point3::try_new(10., 3., 0.).unwrap())
        .unwrap();
    (doc, registry, input)
}

#[test]
fn curvature_enable_and_cache_are_readonly_and_geometry_changes_invalidate_samples() {
    let (doc, _, mut input) = scene();
    let model = format!("{doc:?}");
    let arguments = input.arguments().unwrap();
    let preview = input.preview().unwrap().clone();
    let preferences = input.preferences();
    assert!(
        input
            .curvature_display(&Options::default())
            .unwrap()
            .is_none()
    );
    input.option("_DisplayCurvature").unwrap();
    let first = input
        .curvature_display(&Options::default())
        .unwrap()
        .unwrap();
    input.option("DisplayCurvature").unwrap();
    assert!(input.curvature_display_enabled());
    let second = input
        .curvature_display(&Options::default())
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&first.data, &second.data));
    assert_eq!(input.preview(), Some(&preview));
    assert_eq!(input.arguments().unwrap(), arguments);
    assert_eq!(input.preferences(), preferences);
    let styled = Options {
        scale: 110,
        color: [3, 211, 137],
        ..Options::default()
    };
    let changed_style = input.curvature_display(&styled).unwrap().unwrap();
    assert!(Arc::ptr_eq(&first.data, &changed_style.data));
    assert!(
        first
            .data
            .blend
            .hairs
            .iter()
            .any(|h| h.curvature.to_array() != [0.; 3])
    );
    let dense = Options {
        density: 4,
        ..styled.clone()
    };
    let resampled = input.curvature_display(&dense).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&first.data, &resampled.data));
    let invalid = Options {
        density: 11,
        ..dense.clone()
    };
    assert!(input.curvature_display(&invalid).is_err());
    assert!(Arc::ptr_eq(
        &resampled.data,
        &input.curvature_display(&dense).unwrap().unwrap().data
    ));
    input.option("Handle1=3").unwrap();
    let reshaped = input.curvature_display(&dense).unwrap().unwrap();
    assert!(!Arc::ptr_eq(&resampled.data, &reshaped.data));
    input.set_curvature_display(false).unwrap();
    assert!(input.curvature_display(&dense).unwrap().is_none());
    input.option("DisplayCurvature").unwrap();
    assert!(Arc::ptr_eq(
        &reshaped.data,
        &input.curvature_display(&dense).unwrap().unwrap().data
    ));
    assert_eq!(format!("{doc:?}"), model);
    assert!(!doc.can_undo());
    input.reset(doc.tolerance());
    assert!(!input.curvature_display_enabled());
    assert!(input.option("DisplayCurvature").is_err());
}

#[test]
fn shared_graph_options_without_sources_are_atomic_and_preserve_pending_redo() {
    let (mut doc, registry, _) = scene();
    registry.execute(&mut doc, "Point 90,91,92").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let options = Options {
        scale: 110,
        density: 3,
        ..Options::default()
    };
    registry
        .configure_curvature_graph(&doc, options.clone())
        .unwrap();
    assert_eq!(registry.curvature_graph_options(), options);
    assert!(registry.curvature_graph_view(&doc).unwrap().is_none());
    assert!(
        registry
            .configure_curvature_graph(
                &doc,
                Options {
                    scale: 301,
                    ..options.clone()
                }
            )
            .is_err()
    );
    assert_eq!(registry.curvature_graph_options(), options);
    assert_eq!(format!("{doc:?}"), before);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 3);
}

#[test]
fn display_lifecycle_and_curvature_match_native_without_model_or_history_changes() {
    let mut count = 0;
    for family in display::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            println!("DISPLAY_CASE {id}");
            let value = &row["value"];
            let registry = CommandRegistry::with_builtins();
            registry.remember_blend_curve_preferences(edges::preferences(value));
            let mut doc = display::setup(&registry, value);
            let options = registry.curvature_graph_options();
            let state = empty::State::capture(&doc, value);
            let before = display::curvature(&doc, &value["before"], &options);
            let mut input = display::input(&doc, &registry, value);
            let mut getter = blend::Getter::default();
            for step in portion::steps(&doc, value) {
                match step {
                    portion::Step::Portion(p) => input
                        .pick_subcurve(&doc, p.object, p.edge, p.parameters, p.wrap)
                        .unwrap(),
                    portion::Step::Input(index, action) => {
                        if action.get("view_snapshot").is_some() {
                            display::snapshot(&input, &doc, &registry, value, index);
                        } else if action.get("transparent").is_some() {
                            registry.execute(&mut doc, "CurvatureGraphOff").unwrap();
                        } else if !getter.action(&mut input, &doc, value, index, action) {
                            break;
                        }
                    }
                }
            }
            registry.remember_blend_curve_preferences(input.preferences());
            if blend::accepted(value) {
                registry
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            display::completed(&doc, &registry, value);
            let after = display::curvature(&doc, &value["after"], &options);
            let mut metrics = display::measurements(before, after, state.verify(&mut doc, value));
            metrics["id"] = serde_json::json!(id);
            println!("BLEND_CURVE_DISPLAY_COMMAND {metrics}");
            count += 1;
        }
    }
    assert_eq!(count, display::CASES);
}

#[test]
fn native_continuity_defaults_distinguish_preselection_from_preview_controls() {
    for family in [
        "blend_curve_display_defaults",
        "blend_curve_display_control_defaults",
    ] {
        let registry = CommandRegistry::with_builtins();
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            assert_eq!(
                registry.blend_curve_preferences().continuity,
                points::initial_continuities(value),
                "{}",
                row["id"]
            );
            let mut doc = display::setup(&registry, value);
            let mut input =
                Input::with_preferences(doc.tolerance(), registry.blend_curve_preferences());
            let mut getter = blend::Getter::default();
            for (index, action) in value["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if action.get("view_snapshot").is_some() {
                    continue;
                }
                if !getter.action(&mut input, &doc, value, index, action) {
                    break;
                }
            }
            registry.remember_blend_curve_preferences(input.preferences());
            if blend::accepted(value) {
                registry
                    .execute(&mut doc, &input.arguments().unwrap())
                    .unwrap();
            }
            let (error, domain) = blend::check_high(&doc, &value["after"], false);
            assert!(
                error <= 1e-6 && domain <= 2e-6,
                "{}: {error:e}, {domain:e}",
                row["id"]
            );
        }
    }
}
