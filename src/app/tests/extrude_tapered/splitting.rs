use super::*;
use std::time::{Duration, Instant};
use viboceros_geometry::{NurbsCurve, NurbsSurface, WeightedPoint3};

fn capsule() -> NurbsCurve {
    let points = [
        [-2., 1.],
        [-3., 1.],
        [-3., 0.],
        [-3., -1.],
        [-2., -1.],
        [0., -1.],
        [2., -1.],
        [3., -1.],
        [3., 0.],
        [3., 1.],
        [2., 1.],
        [0., 1.],
        [-2., 1.],
    ];
    let controls = points
        .into_iter()
        .enumerate()
        .map(|(index, [x, y])| {
            let weight = if [1, 3, 7, 9].contains(&index) {
                std::f64::consts::FRAC_1_SQRT_2
            } else {
                1.
            };
            WeightedPoint3::try_new(point(x, y, 0.), weight).unwrap()
        })
        .collect();
    let knots = vec![
        0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 5., 5., 6., 6., 6.,
    ];
    NurbsCurve::try_new_rational(2, controls, knots).unwrap()
}

fn wait_for_preview(app: &mut VibocerosApp, expected: usize) {
    let context = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        app.update_tapered_extrusion_preview(Some(point(0., 0., 1.)), &context);
        if let Some((height, edges)) = app
            .tapered_extrusion_session
            .as_ref()
            .and_then(|session| session.preview_edges(&app.document, app.active_command))
        {
            assert_eq!(edges.len(), expected);
            assert!((height - 1.).abs() < 1e-8);
            return;
        }
        assert!(
            Instant::now() < deadline,
            "taper preview did not finish for {expected} edges"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn check_capsule(app: &VibocerosApp, source: ObjectId, cutter: Option<ObjectId>, split: bool) {
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|object| object.id() != source && Some(object.id()) != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected tapered capsule: {:?}", app.command_log);
    };
    assert!(result.is_solid());
    assert_eq!(
        (result.faces().len(), result.edges().len()),
        if split { (6, 12) } else { (3, 3) }
    );
    let volume = result
        .signed_volume(app.document.tolerance())
        .unwrap()
        .abs();
    assert!((volume - 12.16596199467655).abs() < 1e-5, "volume={volume}");
}

#[test]
fn tapered_split_at_tangents_updates_live_preview_and_replays_the_rhino_capsule() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(capsule()))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtrudeCrvTapered DraftAngle=-8 Solid=Yes");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("SplitAtTangents=No")
    );
    wait_for_preview(&mut app, 3);
    enter(&mut app, "_SplitAtTangents");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("SplitAtTangents=Yes")
    );
    wait_for_preview(&mut app, 12);
    assert_eq!(app.document.objects().count(), 1);
    assert!(app.accept_drafting_point(point(0., 0., 1.)));
    check_capsule(&app, source, None, true);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
    enter(&mut app, "Redo");
    check_capsule(&app, source, None, true);
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ExtrudeCrvTapered");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("SplitAtTangents=Yes")
    );
    enter(&mut app, "SplitAtTangents=No");
    enter(&mut app, "1");
    check_capsule(&app, source, None, false);
}

#[test]
fn tapered_boundary_preserves_the_curve_split_option() {
    for split in [false, true] {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::NurbsCurve(capsule()))
            .unwrap();
        let cutter = NurbsSurface::try_bilinear([
            point(-5., -3.25, 1.),
            point(5., -3.25, 1.),
            point(5., 3.25, 1.),
            point(-5., 3.25, 1.),
        ])
        .unwrap();
        let cutter = app
            .document
            .add_geometry(Geometry::NurbsSurface(cutter))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        let value = if split { "Yes" } else { "No" };
        enter(
            &mut app,
            &format!(
                "ExtrudeCrvTapered ToBoundary={cutter} DraftAngle=-8 Solid=Yes SplitAtTangents={value}"
            ),
        );
        check_capsule(&app, source, Some(cutter), split);
    }
}

#[test]
fn tapered_rational_seams_match_rhino_for_both_split_at_tangents_states() {
    let controls = [
        (3., 0., 1.),
        (4., 1., 0.75),
        (2., 3., 1.),
        (5., 4., 1.5),
        (3., 6., 1.),
    ]
    .map(|(x, z, w)| WeightedPoint3::try_new(point(x, 0., z), w).unwrap())
    .to_vec();
    let curve =
        NurbsCurve::try_new_rational(2, controls, vec![0., 0., 0., 0.4, 0.7, 1., 1., 1.]).unwrap();
    for (angle, height, split, expected) in [
        (5., 1., "No", &[][..]),
        (8., 1., "No", &[0.4][..]),
        (9.5, 1., "No", &[0.4, 0.7][..]),
        (2.012271, 4., "No", &[0.4, 0.7][..]),
        (-8., 1., "Yes", &[0.4, 0.7][..]),
    ] {
        let mut app = test_app();
        let source = app
            .document
            .add_geometry(Geometry::NurbsCurve(curve.clone()))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        enter(
            &mut app,
            &format!("ExtrudeCrvTapered {height} DraftAngle={angle} SplitAtTangents={split}"),
        );
        let Geometry::Brep(result) = app
            .document
            .objects()
            .find(|object| object.id() != source)
            .unwrap()
            .geometry()
        else {
            panic!("expected a tapered sheet");
        };
        assert!(!result.is_solid());
        let faces = 1 + expected.len();
        assert_eq!(
            (result.faces().len(), result.edges().len()),
            (faces, faces * 3 + 1),
            "angle={angle}, height={height}, split={split}"
        );
        for parameter in expected {
            let cut = curve.evaluate(*parameter).unwrap();
            assert!(
                result
                    .vertices()
                    .iter()
                    .any(|vertex| vertex.point().distance_to(cut).unwrap() < 1e-6)
            );
        }
    }
}
