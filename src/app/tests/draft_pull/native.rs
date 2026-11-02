//! Replays native direction choices through app command and picking handlers.
use super::*;
use serde_json::Value;
use viboceros_geometry::{Circle3, CircularArc3, Polyline3};
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn frame(normal: [f64; 3]) -> Frame3 {
    Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_from(normal).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn curve_geometry(op: &Value) -> Geometry {
    let points: [Point3; 2] = std::array::from_fn(|i| {
        Point3::try_from(serde_json::from_value::<[f64; 3]>(op["points"][i].clone()).unwrap())
            .unwrap()
    });
    let reverse = op["reverse"].as_bool().unwrap();
    match op["curve"].as_str().unwrap() {
        "Line" => Geometry::Line(
            LineSegment::try_new(
                points[usize::from(reverse)],
                points[usize::from(!reverse)],
                Tolerance::DEFAULT,
            )
            .unwrap(),
        ),
        "Polyline" => {
            let p = Polyline3::try_new(
                vec![point(0., 0., 0.), point(2., 3., 6.), point(4., 3., 6.)],
                Tolerance::DEFAULT,
            )
            .unwrap();
            Geometry::Polyline(if reverse { p.reversed() } else { p })
        }
        "Arc" => {
            let circle = Circle3::try_from_frame(
                point(0., 0., 0.),
                2.,
                Vector3::try_from([1., 0., 0.])
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap(),
                frame([0., 0., 1.]).z_axis(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let arc =
                CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
            Geometry::Arc(if reverse {
                arc.reversed(Tolerance::DEFAULT).unwrap()
            } else {
                arc
            })
        }
        "BoxEdge" => viboceros_oracle::curvature_analysis::Fixture {
            source: "box".into(),
            style: "Gaussian".into(),
            range: [-1., 1.],
            preselect: true,
            steps: vec!["CurvatureAnalysis".into()],
        }
        .source(Tolerance::DEFAULT)
        .unwrap(),
        other => panic!("unexpected curve {other}"),
    }
}

#[test]
fn directions_match_frozen_owned_native_panel_choices_and_set_actions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/draft_pull_direction.json")).unwrap(),
    )
    .unwrap();
    let n: Value = serde_json::from_reader(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("observations/draft_pull_direction.json.gz")).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        q["operations"].as_array().unwrap().len(),
        n["results"].as_array().unwrap().len()
    );
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        assert!(row.get("error").is_none(), "{row}");
        let (mut app, id) = setup();
        let source = app.document.object(id).unwrap().geometry_snapshot().clone();
        let plane = frame(serde_json::from_value(op["plane"].clone()).unwrap());
        let view: [f64; 3] = serde_json::from_value(op["view"].clone()).unwrap();
        let vector = Vector3::try_from(view).unwrap();
        let vframe = frame(view);
        let location = point(10., 10., 10.);
        let camera = ThreeDmNamedView {
            camera_axes: None,
            name: "Owned pull camera".into(),
            projection: ThreeDmProjection::Parallel,
            camera_location: location,
            camera_direction: vector,
            camera_up: vframe.y_axis().as_vector(),
            target: Some(location.translated(vector.scaled(10.).unwrap()).unwrap()),
            construction_plane: plane,
            frustum: [-2., 2., -2., 2., 1., 1000.],
            screen_port: [0, 800, 600, 0],
        };
        app.viewports[0].restore_file_camera(&camera).unwrap();
        let target =
            (op["curve"] != "None").then(|| app.document.add_geometry(curve_geometry(op)).unwrap());
        let snapshots = app
            .document
            .objects()
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        app.document.clear_history().unwrap();
        let initial = up(&app);
        let mode = op["mode"].as_str().unwrap();
        enter(&mut app, &format!("DraftAngleAnalysis Direction={mode}"));
        if mode == "TwoPoints" {
            let first: [f64; 3] = serde_json::from_value(op["points"][0].clone()).unwrap();
            assert!(app.apply_drafting_point(Point3::try_from(first).unwrap()));
            if op["cancel"] == true {
                app.cancel_interactive_command(false);
            } else {
                let end: [f64; 3] = serde_json::from_value(op["points"][1].clone()).unwrap();
                assert!(app.apply_drafting_point(Point3::try_from(end).unwrap()));
            }
        } else if mode == "Curve" {
            if op["cancel"] == true {
                app.cancel_interactive_command(false);
            } else {
                let edge =
                    (op["curve"] == "BoxEdge").then(|| op["edge"].as_u64().unwrap() as usize);
                if let Some(edge) = edge {
                    app.handle_viewport_action(ViewportOutput {
                        edge_click: Some(vec![crate::viewport::EdgePick {
                            object: target.unwrap(),
                            edge,
                        }]),
                        ..Default::default()
                    });
                } else {
                    app.apply_selection_click(SelectionClick {
                        object_id: target,
                        mode: SelectionMode::Replace,
                    });
                }
            }
        }
        let expected: [f64; 3] = serde_json::from_value(row["value"]["up"].clone()).unwrap();
        close(up(&app), expected);
        if op["cancel"] == true {
            assert_eq!(up(&app), initial);
        }
        assert!(
            app.active_command.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        assert!(source.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot()));
        assert!(app.commands.object_has_draft_angle_analysis(id));
        for (id, before) in snapshots {
            assert!(
                before.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot())
            );
        }
        assert!(!app.document.can_undo());
        assert_eq!(
            app.document.objects().count(),
            if target.is_some() { 2 } else { 1 }
        );
    }
}
