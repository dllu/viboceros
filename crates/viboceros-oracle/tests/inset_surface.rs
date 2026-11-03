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
fn curved_surface_and_boundary_recipes_remain_explicitly_unsupported() {
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_sdk") {
        if !matches!(
            op["scene"].as_str().unwrap(),
            "circle" | "annulus" | "warped" | "cylinder"
        ) {
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
    assert_eq!(covered, 17);
}

#[test]
fn joined_surface_commands_keep_native_geometry_metadata_selection_and_history() {
    use viboceros_command::inset::{Options, Plan};
    use viboceros_document::{Geometry, SelectionMode};
    let mut covered = 0;
    for (op, native) in common::cases_for("inset_surface_ready") {
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
        assert_geometry(result, &native["after"][0]["brep"], doc.tolerance(), label);
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
        assert_geometry(result, &native["after"][0]["brep"], doc.tolerance(), label);
        covered += 1;
    }
    assert_eq!(covered, 12);
}
