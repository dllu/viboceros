use super::*;
use serde::Deserialize;
use viboceros_command::component_selection::{
    ComponentAction, ComponentPick, ComponentSelectionKind,
};
use viboceros_document::ControlPointId;
use viboceros_geometry::{Polyline3, SubD};

#[derive(Deserialize)]
struct Replay {
    name: String,
    source: String,
    selected: Vec<usize>,
    inputs: Vec<String>,
    before: Vec<[f64; 3]>,
    after: Vec<[f64; 3]>,
    point_before: Option<[f64; 3]>,
    point_after: Option<[f64; 3]>,
}

#[test]
fn alignment_grips_and_subd_components_replay_native_targets() {
    let cases: Vec<Replay> = serde_json::from_str(include_str!("rhino_grips.json")).unwrap();
    for case in cases {
        let mut app = super::preview::top_app();
        let points = case
            .before
            .iter()
            .map(|&p| Point3::try_from(p).unwrap())
            .collect();
        let geometry = if case.source == "curve" {
            Geometry::Polyline(Polyline3::try_new(points, app.document.tolerance()).unwrap())
        } else {
            Geometry::SubD(SubD::try_from_polygons(points, vec![vec![0, 1, 3, 2]]).unwrap())
        };
        let owner = app.document.add_geometry(geometry).unwrap();
        let peer = case.point_before.map(|p| {
            let id = app
                .document
                .add_geometry(Geometry::Point(Point3::try_from(p).unwrap()))
                .unwrap();
            app.document
                .select_objects_direct([id], SelectionMode::Add)
                .unwrap();
            id
        });
        let mut selected = case.selected.clone();
        let component = match case.source.as_str() {
            "subd-face" => Some(ComponentSelectionKind::SubDFace),
            "subd-edge" => {
                let Geometry::SubD(subd) = app.document.object(owner).unwrap().geometry() else {
                    unreachable!()
                };
                selected[0] = subd
                    .edges()
                    .iter()
                    .position(|edge| edge.contains(&0) && edge.contains(&1))
                    .unwrap();
                Some(ComponentSelectionKind::SubDEdge)
            }
            "subd-vertex" => Some(ComponentSelectionKind::SubDVertex),
            _ => None,
        };
        if let Some(kind) = component {
            let picks = selected
                .iter()
                .map(|&index| ComponentPick {
                    object: owner,
                    kind,
                    index,
                })
                .collect();
            app.commands
                .component_selection()
                .apply(&app.document, picks, ComponentAction::Add)
                .unwrap();
        } else {
            app.document.enable_control_points([owner]).unwrap();
            app.document
                .select_control_points(
                    selected.iter().map(|&index| ControlPointId {
                        object: owner,
                        index,
                    }),
                    SelectionMode::Add,
                )
                .unwrap();
        }
        let target = case.inputs.iter().any(|token| token == "@curve").then(|| {
            let line = viboceros_geometry::LineSegment::try_new(
                point(0., 0., 0.),
                point(20., 0., 0.),
                app.document.tolerance(),
            )
            .unwrap();
            app.document.add_geometry(Geometry::Line(line)).unwrap()
        });
        app.document.clear_history().unwrap();
        for token in &case.inputs {
            if token == "@curve" {
                enter(&mut app, &format!("CurveId={}", target.unwrap()));
            } else {
                enter(&mut app, token);
            }
        }
        assert!(
            app.command_line_idle(),
            "{}: {:?}",
            case.name,
            app.command_log
        );
        let check =
            |app: &VibocerosApp, positions: &[[f64; 3]], expected_peer: Option<[f64; 3]>| {
                let actual = match app.document.object(owner).unwrap().geometry() {
                    Geometry::SubD(subd) => subd.control_points(),
                    _ => app.document.control_point_locations(owner).unwrap(),
                };
                assert_eq!(actual.len(), positions.len(), "{}", case.name);
                for (&actual, expected) in actual.iter().zip(positions) {
                    assert!(
                        actual
                            .to_array()
                            .iter()
                            .zip(expected)
                            .all(|(a, b)| (a - b).abs() < 1e-8),
                        "{}: {actual:?} != {expected:?}",
                        case.name
                    );
                }
                if let Some(id) = peer {
                    let Geometry::Point(point) = app.document.object(id).unwrap().geometry() else {
                        panic!("peer")
                    };
                    assert_eq!(point.to_array(), expected_peer.unwrap(), "{}", case.name);
                    assert!(app.document.is_selected(id));
                }
                if component.is_none() {
                    assert_eq!(
                        app.document.selected_control_points().count(),
                        selected.len(),
                        "{}",
                        case.name
                    );
                }
            };
        check(&app, &case.after, case.point_after);
        if case.before == case.after && case.point_before == case.point_after {
            assert!(!app.document.can_undo(), "{}", case.name);
        } else {
            assert_eq!(app.document.undo_label(), Some("Align"), "{}", case.name);
            app.document.undo().unwrap();
            assert!(!app.document.can_undo(), "{}", case.name);
            check(&app, &case.before, case.point_before);
            app.document.redo().unwrap();
            check(&app, &case.after, case.point_after);
        }
    }
}

#[test]
fn command_first_align_accepts_displayed_grips_without_whole_objects() {
    let mut app = super::preview::top_app();
    let curve = Polyline3::try_new(
        vec![point(0., 0., 0.), point(2., 3., 0.), point(4., 5., 0.)],
        app.document.tolerance(),
    )
    .unwrap();
    let id = app
        .document
        .add_geometry(Geometry::Polyline(curve))
        .unwrap();
    app.document.enable_control_points([id]).unwrap();
    enter(&mut app, "Align");
    assert!(app.selecting_alignment_sources());
    enter(&mut app, "AlignTo");
    enter(&mut app, "World");
    app.document
        .select_control_points(
            [ControlPointId {
                object: id,
                index: 1,
            }],
            SelectionMode::Add,
        )
        .unwrap();
    enter(&mut app, "");
    assert!(!app.selecting_alignment_sources(), "{:?}", app.command_log);
    enter(&mut app, "Left");
    enter(&mut app, "w6,0,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(
        app.document.control_point_locations(id).unwrap(),
        [point(0., 0., 0.), point(6., 3., 0.), point(4., 5., 0.)]
    );
}
