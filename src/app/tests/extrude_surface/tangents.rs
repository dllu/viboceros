use super::super::circle_preview::{screen_point, viewport_frame};
use super::*;

pub(in crate::app::tests) fn profile(
    name: &str,
    tolerance: viboceros_geometry::Tolerance,
) -> Geometry {
    fixture_profile(
        include_str!("../../../../tools/rhino_oracle/fixtures/surface_apex_tangents.json"),
        &format!("apex-tangent-{name}-0000"),
        tolerance,
    )
}

fn fixture_profile(request: &str, id: &str, tolerance: viboceros_geometry::Tolerance) -> Geometry {
    let request: serde_json::Value = serde_json::from_str(request).unwrap();
    let operation = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["id"] == id)
        .unwrap();
    let fixture: viboceros_oracle::SurfaceApexExtrusionFixture =
        serde_json::from_value(operation.clone()).unwrap();
    fixture.source_face(tolerance).unwrap()
}

#[test]
fn surface_extrusion_near_tangent_boundaries_match_rhino_splits() {
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let threshold =
        include_str!("../../../../tools/rhino_oracle/fixtures/surface_apex_tangent_threshold.json");
    for (index, expected) in [[0, 2], [0, 2], [1, 3], [1, 3]].into_iter().enumerate() {
        let id = format!("apex-tangent-threshold-{index}-00");
        let Geometry::Brep(source) = fixture_profile(threshold, &id, tolerance) else {
            panic!("expected a trimmed face for {id}");
        };
        for (split, expected) in [false, true].into_iter().zip(expected) {
            let cuts = source.sweep_boundary_splits(split).unwrap();
            assert_eq!(
                cuts.iter().map(|(_, cuts)| cuts.len()).sum::<usize>(),
                expected,
                "{id} with SplitAtTangents={split}"
            );
        }
    }
}

#[cfg(feature = "native-smlib")]
#[test]
fn surface_extrusion_tangent_splits_match_rhino_topology() {
    let tolerance = viboceros_geometry::Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let cases = [
        ("capsule", [(3, 3, 2), (6, 12, 8)]),
        ("capsule_c1", [(3, 3, 2), (6, 12, 8)]),
        ("capsule_seam", [(3, 3, 2), (7, 15, 10)]),
        ("circle", [(3, 3, 2), (3, 3, 2)]),
        ("circle_speed", [(3, 3, 2), (3, 3, 2)]),
        ("corners", [(6, 12, 8), (6, 12, 8)]),
        ("hole", [(4, 6, 4), (7, 15, 10)]),
        ("tilted", [(3, 3, 2), (6, 12, 8)]),
    ];
    for (name, topology) in cases {
        let source = profile(name, tolerance);
        for (split, expected) in ["No", "Yes"].into_iter().zip(topology) {
            let mut doc = viboceros_document::Document::new(tolerance);
            let id = doc.add_geometry(source.clone()).unwrap();
            viboceros_command::CommandRegistry::with_builtins()
                .execute(
                    &mut doc,
                    &format!("ExtrudeSrf 4 Solid=Yes FaceRefs={id}:0 SplitAtTangents={split}"),
                )
                .unwrap_or_else(|error| panic!("{name} with SplitAtTangents={split}: {error}"));
            let mut results = doc.selected_objects();
            let Geometry::Brep(result) = results.next().unwrap().geometry() else {
                panic!("expected a solid extrusion of {name}");
            };
            assert!(results.next().is_none());
            assert!(result.is_solid());
            assert_eq!(
                (
                    result.faces().len(),
                    result.edges().len(),
                    result.vertices().len()
                ),
                expected,
                "{name} with SplitAtTangents={split}"
            );
            if name.starts_with("capsule") || name == "tilted" {
                let expected = 4. * (8. + std::f64::consts::PI);
                assert!((result.signed_volume(tolerance).unwrap().abs() - expected).abs() < 1e-6);
            }
        }
    }

    for (split, expected) in [("No", vec![(1, 3), (1, 3)]), ("Yes", vec![(1, 3), (4, 12)])] {
        let mut doc = viboceros_document::Document::new(tolerance);
        let id = doc.add_geometry(profile("hole", tolerance)).unwrap();
        viboceros_command::CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!("ExtrudeSrf 4 FaceRefs={id}:0 Solid=No SplitAtTangents={split}"),
            )
            .unwrap();
        let mut topology = doc
            .selected_objects()
            .map(|object| {
                let Geometry::Brep(wall) = object.geometry() else {
                    panic!("expected an open wall")
                };
                assert!(!wall.is_solid());
                (wall.faces().len(), wall.edges().len())
            })
            .collect::<Vec<_>>();
        topology.sort_unstable();
        assert_eq!(topology, expected);
    }
}

#[test]
fn surface_extrusion_tangent_seams_toggle_in_the_live_preview() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Front);
    app.active_viewport = 0;
    app.maximized_viewport = Some(0);
    app.osnap = false;
    app.grid_snap = false;
    let source = profile("capsule_seam", app.document.tolerance());
    let id = app.document.add_geometry(source).unwrap();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ExtrudeSrf Solid=No");
    let ctx = egui::Context::default();
    let has_seam = |app: &mut VibocerosApp| {
        viewport_frame(app, &ctx, vec![]);
        viewport_frame(app, &ctx, vec![]);
        let pointer = screen_point(app, point(0., 0., 4.));
        let lines = viewport_frame(app, &ctx, vec![egui::Event::PointerMoved(pointer)]);
        let height = app.surface_extrusion_preview().unwrap().hover.unwrap();
        assert!((height - 4.).abs() < 1e-5);
        let start = screen_point(app, point(-2., -1., 0.));
        let end = screen_point(app, point(-2., -1., height));
        lines
            .iter()
            .any(|[a, b]| a.distance(start) < 1e-3 && b.distance(end) < 1e-3)
    };
    assert!(!has_seam(&mut app));
    enter(&mut app, "SplitAtTangents");
    assert!(has_seam(&mut app));
    enter(&mut app, "SplitAtTangents=No");
    assert!(!has_seam(&mut app));
    assert_eq!(format!("{:?}", app.document), before);
}
