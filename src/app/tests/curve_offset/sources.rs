use super::*;
use crate::viewport::{ComponentClick, ComponentPick, EdgePick};
use viboceros_command::ComponentSelectionKind;
use viboceros_geometry::{Brep, NurbsSurface};

#[derive(Deserialize)]
struct NativeCase {
    name: String,
    shape: String,
    edge: bool,
    #[serde(default)]
    input_layer: bool,
    #[serde(default)]
    preselect: bool,
    #[serde(default)]
    connected: bool,
    inputs: Vec<String>,
    curves: Vec<NativeCurve>,
}

#[derive(Deserialize)]
struct NativeCurve {
    #[serde(flatten)]
    geometry: Curve,
    layer: String,
}

fn parent(app: &mut VibocerosApp, shape: &str) -> (ObjectId, Option<usize>) {
    let tolerance = app.document.tolerance();
    let frame = app.viewports[app.active_viewport].construction_plane();
    let geometry = match shape {
        "lines" => {
            enter(app, "Line w0,0,0 w8,0,0");
            enter(app, "Line w8,0,0 w8,6,0");
            return (app.document.objects().next().unwrap().id(), None);
        }
        "line" | "circle" => {
            enter(
                app,
                if shape == "line" {
                    "Line w0,0,0 w8,0,0"
                } else {
                    "Circle w0,0,0 4"
                },
            );
            return (app.document.objects().last().unwrap().id(), None);
        }
        "box" => {
            Geometry::Brep(Brep::try_box(frame, [[0., 8.], [0., 6.], [0., 2.]], tolerance).unwrap())
        }
        "sheet" => Geometry::NurbsSurface(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                vec![
                    point(0., 0., 0.),
                    point(8., 0., 0.),
                    point(0., 6., 0.),
                    point(8., 6., 0.),
                ],
                vec![0., 0., 8., 8.],
                vec![0., 0., 6., 6.],
            )
            .unwrap(),
        ),
        "cylinder" => Geometry::Brep(Brep::try_cylinder(frame, 4., 0., 3., tolerance).unwrap()),
        _ => panic!("unknown source geometry {shape}"),
    };
    let brep = match &geometry {
        Geometry::Brep(brep) => brep.clone(),
        Geometry::NurbsSurface(surface) => {
            Brep::try_surface_face(surface.clone(), tolerance).unwrap()
        }
        _ => unreachable!(),
    };
    let edge = brep
        .edges()
        .iter()
        .position(|edge| {
            let curve = edge.curve();
            let a = curve.evaluate(*curve.domain().start()).unwrap();
            let b = curve.evaluate(*curve.domain().end()).unwrap();
            if shape == "cylinder" {
                curve.is_closed().unwrap() && a.z().abs() < 1e-9
            } else {
                let start = point(0., 0., 0.);
                let end = point(8., 0., 0.);
                (a == start && b == end) || (a == end && b == start)
            }
        })
        .expect("fixture edge");
    let id = app.document.add_geometry(geometry).unwrap();
    (id, Some(edge))
}

fn preselect(app: &mut VibocerosApp, object: ObjectId, index: usize) {
    assert!(app.handle_viewport_action(ViewportOutput {
        component_click: Some(ComponentClick {
            picks: vec![ComponentPick {
                object,
                index,
                kind: ComponentSelectionKind::BrepEdge
            }],
            preselection: true,
            modifiers: egui::Modifiers::NONE,
        }),
        ..Default::default()
    }));
}

#[test]
fn source_options_portions_and_component_preselection_match_rhino() {
    let cases: Vec<NativeCase> = serde_json::from_str(include_str!("rhino_sources.json")).unwrap();
    for case in cases {
        let mut app = offset_app();
        let color = viboceros_document::ColorRgb::BLACK;
        let input_layer = app.document.add_layer("Plane sources", color).unwrap();
        let output_layer = app.document.add_layer("Plane output", color).unwrap();
        app.document
            .set_current_layer(if case.edge || case.input_layer {
                input_layer
            } else {
                output_layer
            })
            .unwrap();
        let (source, edge) = parent(&mut app, &case.shape);
        app.document.set_current_layer(output_layer).unwrap();
        let originals = app
            .document
            .objects()
            .map(|object| (object.id(), object.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        if case.edge {
            preselect(&mut app, source, edge.unwrap());
            if case.connected {
                let Geometry::Brep(brep) = app.document.object(source).unwrap().geometry() else {
                    unreachable!()
                };
                let adjacent = brep
                    .edges()
                    .iter()
                    .position(|e| {
                        let c = e.curve();
                        let a = c.evaluate(*c.domain().start()).unwrap();
                        let b = c.evaluate(*c.domain().end()).unwrap();
                        let (p, q) = (point(8., 0., 0.), point(8., 6., 0.));
                        (a == p && b == q) || (a == q && b == p)
                    })
                    .unwrap();
                preselect(&mut app, source, adjacent);
            }
        } else if case.preselect {
            enter(&mut app, "SelAll");
        }
        for token in &case.inputs {
            enter(&mut app, token);
        }
        assert!(
            app.active_command.is_none(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        assert!(
            app.object_prompt.is_none(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        assert!(app.subcurve_prompt.is_none(), "{}", case.name);
        assert_eq!(app.document.selected_object_count(), 0, "{}", case.name);
        let command = case.inputs[0].split_whitespace().next().unwrap();
        check_sources(&app, &case);
        assert_eq!(
            app.document.undo().unwrap().as_deref(),
            Some(command),
            "{}",
            case.name
        );
        assert_eq!(app.document.objects().len(), originals.len());
        for (id, original) in &originals {
            assert!(
                original.shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
            );
        }
        assert_eq!(
            app.document.redo().unwrap().as_deref(),
            Some(command),
            "{}",
            case.name
        );
        check_sources(&app, &case);
        if case.edge
            || case.name.contains("subcurve")
            || case.name.contains("portion")
            || case.name.contains("chain-lines")
            || case.name == "owner-both-cap"
        {
            for (id, original) in &originals {
                assert!(
                    original
                        .shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
                );
            }
        }
    }
}

fn check_sources(app: &VibocerosApp, case: &NativeCase) {
    let curves = app
        .document
        .objects()
        .filter(|object| object.geometry().curve_ref().is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        curves.len(),
        case.curves.len(),
        "{}: {:?}",
        case.name,
        app.command_log
    );
    for (object, expected) in curves.into_iter().zip(&case.curves) {
        let tolerance = app.document.tolerance();
        let geometry = object.geometry();
        let bounds = geometry.tight_bounds(tolerance).unwrap();
        let actual = [bounds.min().to_array(), bounds.max().to_array()];
        for (&a, &b) in actual
            .iter()
            .flatten()
            .zip(expected.geometry.bounds.iter().flatten())
        {
            assert!(
                (a - b).abs() < 1e-8,
                "{}: {actual:?} != {:?}",
                case.name,
                expected.geometry.bounds
            );
        }
        let curve = geometry.curve_ref().unwrap();
        assert_eq!(
            curve.is_closed().unwrap(),
            expected.geometry.closed,
            "{}",
            case.name
        );
        let length = curve.length(tolerance).unwrap();
        assert!(
            (length - expected.geometry.length).abs() < 1e-7,
            "{}: {length} != {}",
            case.name,
            expected.geometry.length
        );
        assert_eq!(
            app.document
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            expected.layer,
            "{}",
            case.name
        );
    }
}

#[test]
fn viewport_picks_a_surface_edge_and_previews_its_temporary_portion() {
    let mut app = offset_app();
    let (source, _) = parent(&mut app, "sheet");
    let context = egui::Context::default();
    enter(&mut app, "Offset SubCrv");
    viewport_frame(&mut app, &context, Vec::new());
    viewport_frame(&mut app, &context, Vec::new());
    assert!(app.picking_subcurve_edge());
    let edge = screen_point(&app, point(4., 0., 0.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(edge)]);
    viewport_click(&mut app, &context, edge);
    assert!(
        app.subcurve_prompt
            .as_ref()
            .and_then(subcurve_prompt::SubcurvePrompt::temporary_reference)
            .is_some(),
        "{:?}",
        app.command_log
    );
    viewport_frame(&mut app, &context, Vec::new());
    let start = screen_point(&app, point(2., 0., 0.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(start)]);
    viewport_click(&mut app, &context, start);
    let end = screen_point(&app, point(6., 0., 0.));
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(end)]);
    let preview = app.subcurve_draft_preview().unwrap();
    assert!(
        preview.geometry.endpoints[0]
            .distance_to(point(2., 0., 0.))
            .unwrap()
            < 1e-3
    );
    assert!(
        preview.geometry.endpoints[1]
            .distance_to(point(6., 0., 0.))
            .unwrap()
            < 1e-3
    );
    assert_eq!(app.document.objects().len(), 1);
    viewport_click(&mut app, &context, end);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CurveOffset { .. })
    ));
    enter(&mut app, "Cap=Flat");
    enter(&mut app, "w4,-4,0");
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.object(source).is_some());
}

#[test]
fn a_new_source_prompt_clears_temporary_picks_and_keeps_edited_settings_on_cancel() {
    let mut app = offset_app();
    let (line, _) = parent(&mut app, "line");
    for input in ["Offset", "Distance", "2.5", "Cap=Round", "Loose", "Cancel"] {
        enter(&mut app, input);
    }
    assert!(app.object_prompt.is_none());
    assert!(app.active_command.is_none());
    assert!(!app.document.is_selected(line));
    let settings = app.commands.offset_settings(Kind::Offset);
    assert_eq!(settings.distance, 2.5);
    assert_eq!(settings.cap, Cap::Round);
    assert!(!settings.loose);
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn multiple_sources_can_retain_picked_edges_across_another_source_getter() {
    let mut app = offset_app();
    let (sheet, edge) = parent(&mut app, "sheet");
    let (line, _) = parent(&mut app, "line");
    enter(&mut app, "OffsetMultiple OffsetCount=2");
    let edge = EdgePick {
        object: sheet,
        edge: edge.unwrap(),
    };
    assert!(app.handle_viewport_action(ViewportOutput {
        edge_click: Some(vec![edge]),
        ..Default::default()
    }));
    assert!(app.object_prompt.is_some());
    enter(&mut app, "SubCrv");
    assert!(app.pick_subcurve_source(Some(line)));
    enter(&mut app, "w2,0,0");
    enter(&mut app, "w6,0,0");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "Enter");
    enter(&mut app, "w4,-4,0");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 6, "{:?}", app.command_log);
    assert!(app.document.object(sheet).is_some());
    assert!(app.document.object(line).is_some());
}

#[test]
fn disconnected_preselection_returns_offset_to_a_single_source_pick() {
    let mut app = offset_app();
    let (first, _) = parent(&mut app, "line");
    enter(&mut app, "Line w0,6,0 w8,6,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "Offset");
    assert!(app.object_prompt.is_some());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    app.select_prompt_objects([first], SelectionMode::Add);
    enter(&mut app, "w4,-4,0");
    assert_eq!(app.document.objects().len(), 3);
}
