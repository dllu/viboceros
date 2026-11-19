use super::*;
use viboceros_geometry::{Brep, Circle3, NurbsCurve, UnitVector3};

fn circle(app: &VibocerosApp) -> NurbsCurve {
    let tolerance = app.document.tolerance();
    Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn surface_taper_oblique_boundary_incomplete_cutter_pick_keeps_the_source() {
    for (heights, rhino_height) in [
        ([5.; 4], 19.345623455800396),
        (
            [
                4.8931818181818185,
                5.415909090909091,
                6.134848484848485,
                5.513636363636364,
            ],
            19.679462525258618,
        ),
    ] {
        let mut app = test_app();
        let profile = Brep::try_planar_face(&circle(&app), app.document.tolerance()).unwrap();
        let source = app.document.add_geometry(Geometry::Brep(profile)).unwrap();
        let corners = [(0., -6.), (10., -6.), (10., 7.), (0., 7.)];
        let chart = NurbsSurface::try_bilinear(std::array::from_fn(|i| {
            point(corners[i].0, corners[i].1, heights[i])
        }))
        .unwrap();
        let aim = chart.evaluate(0.5, 0.03).unwrap();
        let cutter = app
            .document
            .add_geometry(Geometry::NurbsSurface(chart))
            .unwrap();
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();

        enter(
            &mut app,
            "ExtrudeSrfTapered ToBoundary DraftAngle=-10 Solid=No DeleteInput=Yes",
        );
        enter(&mut app, "Direction");
        enter(&mut app, "w0,0,0");
        enter(&mut app, "w1,0,6");
        assert!(app.picking_tapered_boundary());
        pick_boundary_in_top_view(&mut app, [point(-5., -8., 0.), point(12., 9., 8.)], aim);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.command_log.back().unwrap().starts_with("Error: "));
        assert!(app.document.is_selected(source));
        assert!(app.document.is_selected(cutter));
        let Geometry::Brep(preview) = app
            .document
            .objects()
            .find(|object| ![source, cutter].contains(&object.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected the fallback stock");
        };
        assert_eq!((preview.faces().len(), preview.edges().len()), (5, 12));
        assert!(!preview.is_solid());
        let height = preview
            .vertices()
            .iter()
            .map(|vertex| vertex.point().z())
            .max_by(f64::total_cmp)
            .unwrap();
        assert!(
            (height - rhino_height).abs() < 1e-6,
            "{height} vs {rhino_height}"
        );
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        enter(&mut app, "Redo");
        assert_eq!(app.document.objects().len(), 3);
        assert!(app.document.is_selected(source));
        assert!(!app.document.is_selected(cutter));
    }
}

#[test]
fn surface_taper_oblique_boundary_completes_a_cut_that_misses_the_rim() {
    for (bounds, aim) in [
        ([0.3, -1.2, 1.8, 1.2], point(1.05, -1.18, 5.)),
        ([7., -6., 12., 7.], point(9.5, -5.8, 5.)),
    ] {
        let mut app = test_app();
        let profile = Brep::try_planar_face(&circle(&app), app.document.tolerance()).unwrap();
        let source = app.document.add_geometry(Geometry::Brep(profile)).unwrap();
        let cutter = rectangle(&mut app, bounds, 5.);
        app.document
            .select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        app.document.clear_history().unwrap();
        enter(
            &mut app,
            "ExtrudeSrfTapered Direction=1,0,6 ToBoundary DraftAngle=-10 DeleteInput=Yes",
        );
        assert!(app.picking_tapered_boundary());
        pick_boundary_in_top_view(&mut app, [point(-5., -8., 0.), point(13., 8., 5.)], aim);
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        assert!(app.command_log.back().unwrap().starts_with("Error: "));
        assert_eq!(app.document.objects().len(), 2);
        assert!(app.document.is_selected(source));
        assert!(app.document.is_selected(cutter));
        assert!(!app.document.can_undo());
    }
}

#[test]
fn curve_taper_oblique_boundary_viewport_pick_caps_a_tangent_section() {
    use super::super::super::circle_preview::viewport_click;

    let mut app = test_app();
    app.document = Document::new(Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap());
    let profile = circle(&app);
    let source = app
        .document
        .add_geometry(Geometry::NurbsCurve(profile))
        .unwrap();
    let support = NurbsSurface::try_bilinear([
        point(-2., -2.7, 4.941136363636363),
        point(3.136774381710049, -2.7, 5.222491505907301),
        point(3.136774381710049, 2.7, 5.493051037468842),
        point(-2., 2.7, 5.190681818181818),
    ])
    .unwrap();
    let aim = support.evaluate(0.5, 0.012).unwrap();
    let cutter = app
        .document
        .add_geometry(Geometry::NurbsSurface(support))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(
        &mut app,
        "ExtrudeCrvTapered Direction=1,0,6 DraftAngle=8 Solid=Yes DeleteInput=Yes",
    );
    let ctx = top_view(&mut app, [point(-5., -10., 0.), point(5., 5., 6.)]);
    move_pointer(&mut app, &ctx, point(1.832, -8.99, 0.));
    enter(&mut app, "ToBoundary");
    assert!(app.picking_tapered_boundary());
    let cursor = move_pointer(&mut app, &ctx, aim);
    viewport_click(&mut app, &ctx, cursor);
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.document.object(source).is_none());
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|object| object.id() != cutter)
        .unwrap()
        .geometry()
    else {
        panic!("expected a cut B-rep");
    };
    assert!(result.is_solid());
    assert_eq!((result.faces().len(), result.edges().len()), (6, 12));
    enter(&mut app, "Undo");
    assert!(app.document.is_selected(source));
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "Redo");
    assert!(app.document.object(source).is_none());
}
