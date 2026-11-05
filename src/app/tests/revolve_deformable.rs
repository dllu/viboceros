use super::*;
use viboceros_command::revolve::Options;
use viboceros_oracle::revolve_deformable::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn revolve_full_circle_and_enter_use_the_active_construction_plane_axis() {
    use viboceros_geometry::{Frame3, Vector3};
    let mut app = test_app();
    enter(&mut app, "Line 3,0,0 3,0,5");
    enter(&mut app, "SelAll");
    let frame = Frame3::try_from_directions(
        point(0., 0., 0.),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        app.document.tolerance(),
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(frame);
    enter(&mut app, "Revolve Deformable=Yes PointCount=8 Angle=120");
    app.accept_drafting_point(point(0., 0., 0.));
    enter(&mut app, "FullCircle");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 2);
    let Geometry::NurbsSurface(s) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("deformable NURBS surface required")
    };
    assert!(s.is_periodic_u());
    assert_eq!(s.control_point_count_u(), 11);
    assert!(
        s.evaluate(s.parameter_at_u(0.25).unwrap(), *s.domain_v().end())
            .unwrap()
            .distance_to(point(3., -5., 0.))
            .unwrap()
            < 1e-11
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn deformable_revolution_axis_getters_match_native_geometry_and_history() {
    compare_family("revolve_deformable", 51);
    compare_family("revolve_deformable_holdouts", 12);
    compare_family("revolve_deformable_domains", 18);
    compare_family("revolve_deformable_thresholds", 18);
    compare_family("revolve_deformable_scales", 26);
    compare_family("revolve_deformable_policy", 26);
    compare_family("revolve_deformable_upper", 17);
}

fn compare_family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, family);
    let ops = request["operations"].as_array().unwrap();
    assert_eq!(ops.len(), count);
    assert_eq!(ops.len(), native["results"].as_array().unwrap().len());
    for (op, row) in ops.iter().zip(native["results"].as_array().unwrap()) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let (doc, source) = setup(&f, app.document.tolerance()).unwrap();
        app.document = doc;
        let compare = |app: &VibocerosApp, phase: &str| {
            super::edge_analysis_mixed_support::compare(
                &snapshot(&app.document, source).unwrap(),
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            );
        };
        compare(&app, "before");
        enter(&mut app, "Revolve Deformable=Yes Output=Surface");
        assert!(app.active_command.is_some());
        enter(
            &mut app,
            &format!(
                "Angle={} StartAngle={} PointCount={} DeleteInput={}",
                f.sweep_degrees,
                f.start_degrees,
                f.point_count,
                if f.delete_input { "Yes" } else { "No" }
            ),
        );
        app.accept_drafting_point(point(f.origin[0], f.origin[1], f.origin[2]));
        app.accept_drafting_point(point(f.axis_end[0], f.axis_end[1], f.axis_end[2]));
        assert!(app.active_command.is_none());
        compare(&app, "after");
        assert_eq!(app.document.undo_label(), Some("Revolve"));
        enter(&mut app, "Undo");
        compare(&app, "undo");
        enter(&mut app, "Redo");
        compare(&app, "redo");
    }
}

#[test]
fn revolve_options_remain_atomic_between_axis_picks_and_cancel_preserves_redo() {
    let mut app = test_app();
    enter(&mut app, "Line 3,0,0 3,0,5");
    enter(&mut app, "SelAll");
    enter(&mut app, "Revolve 0,0,0 0,0,1 90");
    enter(&mut app, "Undo");
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "Revolve");
    app.accept_drafting_point(point(0., 0., 0.));
    enter(
        &mut app,
        "Deformable=Yes PointCount=16 Angle=-270 StartAngle=20 DeleteInput=Yes",
    );
    let expected = InteractiveCommand::Revolve {
        axis_start: Some(point(0., 0., 0.)),
        options: Options {
            deformable: true,
            point_count: 16,
            sweep_degrees: -270.,
            start_angle_degrees: 20.,
            delete_input: true,
        },
    };
    assert_eq!(app.active_command, Some(expected));
    for input in [
        "PointCount=3",
        "PointCount=1001",
        "PointCount=4.5",
        "Deformable=Maybe",
        "Angle=0",
        "Angle=NaN",
        "Angle=361",
        "StartAngle=inf",
        "Output=SubD",
        "PointCount=8 PointCount=16",
        "Angle=90 SweepAngle=120",
        "FullCircle=Yes Angle=30",
        "Deformable=No PointCount=0",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, Some(expected), "{input}");
    }
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}
