use super::*;
use viboceros_geometry::{Brep, Circle3, UnitVector3};

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
        let tolerance = app.document.tolerance();
        let curve = Circle3::try_new(
            point(0., 0., 0.),
            3.,
            UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let profile = Brep::try_planar_face(&curve, tolerance).unwrap();
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
