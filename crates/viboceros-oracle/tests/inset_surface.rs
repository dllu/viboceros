//! Native topology and spatial witnesses, independent of edge clocks/face order.
use viboceros_geometry::Tolerance;
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/inset_surface.rs"]
mod support;
use support::{assert_geometry, source};

#[test]
fn planar_polygon_insets_match_native_vertices_incidence_and_trimmed_regions() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_sdk")
        .into_iter()
        .chain(common::cases_for("inset_surface_controls"))
    {
        let scene = op["scene"].as_str().unwrap();
        if !matches!(
            scene,
            "rectangle"
                | "skew"
                | "concave"
                | "hole"
                | "box_one"
                | "box_pair"
                | "triangle"
                | "reversed"
                | "parallelogram"
                | "collinear"
        ) {
            continue;
        }
        let id = op["id"].as_str().unwrap();
        let original = source(&native["before"]);
        let before = original.clone();
        let indices = native["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let output = original
            .try_inset_planar_faces(
                &indices,
                op["distance"].as_f64().unwrap(),
                op["crease_corners"].as_bool().unwrap(),
                tolerance,
            )
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(original, before);
        assert_eq!(native["source_unchanged"], true);
        assert_eq!(native["output"]["valid"], true);
        let expected = source(&native["output"]);
        assert_geometry(&output, &native["output"]["geometry"], tolerance, id);
        assert!(output.faces().iter().all(|f| {
            original
                .faces()
                .iter()
                .any(|s| f.surface() == s.surface() && f.is_reversed() == s.is_reversed())
        }));
        assert!(
            (output.area(tolerance).unwrap() - expected.area(tolerance).unwrap()).abs() < 1e-8,
            "{id}: area"
        );
        covered += 1;
    }
    assert_eq!(covered, 41);
}

#[test]
fn nonplanar_surface_recipes_remain_explicitly_unsupported() {
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_sdk") {
        if !matches!(op["scene"].as_str().unwrap(), "warped" | "cylinder") {
            continue;
        }
        let original = source(&native["before"]);
        let before = original.clone();
        assert!(
            original
                .try_inset_planar_faces(
                    &[0],
                    0.3,
                    op["crease_corners"].as_bool().unwrap(),
                    Tolerance::DEFAULT
                )
                .is_err()
        );
        assert_eq!(original, before);
        assert_eq!(native["output"]["valid"], true);
        covered += 1;
    }
    assert_eq!(covered, 9);
}

#[test]
fn native_ellipse_command_fit_error_is_measured_against_the_source() {
    let (_, native) = common::cases_for("inset_planar_curves_command").remove(0);
    let source = source(&native);
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let actual = source
        .try_inset_planar_faces(&[0], 0.3, false, tolerance)
        .unwrap();
    let expected =
        support::nurbs_definition(&native["after"][0]["brep"]["edges"][1]["curve"]["definition"]);
    let source = source.edges()[0].curve();
    let query = Tolerance::try_new(1e-12, 1e-14, 1e-10).unwrap();
    for (label, curve) in [("native", &expected), ("shared", actual.edges()[1].curve())] {
        let mut largest = 0_f64;
        for (a, b) in curve.spans() {
            for i in 0..=4 {
                let p = curve.evaluate(a + (b - a) * (i as f64 / 4.)).unwrap();
                let t = source.closest_parameter(p, query).unwrap();
                largest =
                    largest.max((source.evaluate(t).unwrap().distance_to(p).unwrap() - 0.3).abs());
            }
        }
        eprintln!("{label} ellipse inset maximum sampled distance error: {largest}");
        if label == "shared" {
            assert!(largest < tolerance.absolute() * 0.1, "{label}: {largest}");
        } else {
            assert!(
                largest > tolerance.absolute() * 0.5 && largest < tolerance.absolute(),
                "{label}: {largest}"
            );
        }
    }
}

#[test]
fn smooth_planar_insets_match_native_curves_and_regions() {
    replay_smooth_planar(None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn smooth_planar_insets_with_usd_brep_match_native_curves_and_regions() {
    replay_smooth_planar(Some(viboceros_smlib::curve_offset_operations()));
}
fn replay_smooth_planar(operations: Option<viboceros_geometry::CurveOffsetOperations>) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_sdk")
        .into_iter()
        .chain(common::cases_for("inset_planar_curves"))
    {
        let family = op["op"].as_str().unwrap();
        let scene = op["scene"].as_str().unwrap();
        if family == "inset_surface_sdk" && !matches!(scene, "circle" | "annulus") {
            continue;
        }
        if op["loose"] == true
            && !matches!(
                scene,
                "circle" | "annulus" | "mixed" | "stadium" | "circle_rotated" | "annulus_narrow"
            )
        {
            continue;
        }
        let id = op["id"].as_str().unwrap();
        let original = source(&native["before"]);
        let before = original.clone();
        let output = original
            .try_inset_planar_faces_with_operations(
                &[0],
                op["distance"].as_f64().unwrap(),
                op["crease_corners"].as_bool().unwrap(),
                tolerance,
                operations,
            )
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(original, before);
        assert_eq!(native["output"]["valid"], true);
        let epsilon = support::epsilon_for(&op);
        support::assert_geometry_with_epsilon(
            &output,
            &native["output"]["geometry"],
            tolerance,
            epsilon,
            id,
        );
        assert!(output.faces().iter().all(|f| {
            original
                .faces()
                .iter()
                .any(|s| f.surface() == s.surface() && f.is_reversed() == s.is_reversed())
        }));
        covered += 1;
    }
    assert_eq!(covered, 34);
}

#[test]
fn joined_surface_commands_keep_native_geometry_metadata_selection_and_history() {
    use viboceros_command::inset::{Options, Plan};
    use viboceros_document::{Geometry, SelectionMode};
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_ready")
        .into_iter()
        .chain(common::cases_for("inset_planar_curves_command"))
    {
        if op["join_outputs"] != true {
            continue;
        }
        let label = op["id"].as_str().unwrap();
        let (mut doc, id) = support::fixture(&native);
        if native["before"][0]["selected"] == true {
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        support::assert_metadata(&doc, id, &native["before"][0]);
        let indices = if op["scene"] == "box_pair" {
            vec![0, 1]
        } else {
            vec![0]
        };
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let options = Options {
            distance: 0.3,
            crease_corners: op["crease_corners"].as_bool().unwrap(),
            ..Options::default()
        };
        let plan = Plan::prepare(&doc, indices.into_iter().map(|i| (id, i)), options).unwrap();
        assert!(!plan.preview_edges().unwrap().is_empty());
        assert!(!doc.can_undo());
        plan.commit(&mut doc).unwrap();
        support::assert_metadata(&doc, id, &native["after"][0]);
        let Geometry::Brep(result) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        support::assert_geometry_with_epsilon(
            result,
            &native["after"][0]["brep"],
            doc.tolerance(),
            support::epsilon_for(&op),
            label,
        );
        let after = doc.object(id).unwrap().geometry_snapshot().clone();
        // The trailing macro Cancel releases whole-object selection separately
        // from Inset, before native history replay.
        doc.clear_selection();
        support::assert_metadata(&doc, id, &native["after_script"][0]);
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        support::assert_metadata(&doc, id, &native["undo"][0]);
        doc.redo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &after);
        support::assert_metadata(&doc, id, &native["redo"][0]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inset.3dm");
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        let Geometry::Brep(result) = restored.object(id).unwrap().geometry() else {
            panic!()
        };
        support::assert_geometry_with_epsilon(
            result,
            &native["after"][0]["brep"],
            doc.tolerance(),
            support::epsilon_for(&op),
            label,
        );
        covered += 1;
    }
    assert_eq!(covered, 30);
}
