use super::*;
use viboceros_command::inset::{Options, Plan};
use viboceros_document::{Geometry, SelectionMode};

#[test]
fn polygon_output_modes_match_native_geometry_metadata_and_history() {
    let cases = common::cases_for("inset_surface_ready")
        .into_iter()
        .filter(|(op, _)| op["join_outputs"] != true)
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 24);
    replay(cases);
}
#[test]
fn curved_output_modes_match_native_geometry_metadata_and_history() {
    let cases = common::cases_for("inset_output_modes");
    assert_eq!(cases.len(), 72);
    replay(cases);
}
pub(super) fn replay(cases: Vec<(serde_json::Value, serde_json::Value)>) {
    for (op, native) in cases {
        let label = op["id"].as_str().unwrap();
        eprintln!("Inset command: {label}");
        let (mut doc, id) = native_surface::fixture(&native);
        if native["before"][0]["selected"] == true {
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        let options = Options {
            distance: op["distance"].as_f64().unwrap_or(0.3),
            crease_corners: op["crease_corners"].as_bool().unwrap(),
            loose: op["loose"].as_bool().unwrap_or(false),
            split: op["split"].as_bool().unwrap(),
            join_outputs: op["join_outputs"].as_bool().unwrap(),
            ..Options::default()
        };
        let indices = if op["scene"] == "box_pair" {
            vec![0, 1]
        } else {
            vec![0]
        };
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let plan = Plan::prepare(&doc, indices.iter().map(|&i| (id, i)), options)
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(!plan.preview_edges().unwrap().is_empty());
        assert!(!doc.can_undo());
        assert_eq!(doc.objects().len(), 1);
        plan.commit(&mut doc).unwrap();
        if !options.split {
            assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        } else if !options.join_outputs {
            assert!(doc.object(id).is_none());
        } else {
            assert!(doc.object(id).is_some());
        }
        let epsilon = native_surface::epsilon_for(&op);
        output_support::assert_document(&doc, id, &native["after"], epsilon, label);
        let after = doc
            .objects()
            .map(|o| (o.id(), o.geometry_snapshot().clone()))
            .collect::<Vec<_>>();
        doc.clear_selection();
        output_support::assert_document(&doc, id, &native["after_script"], epsilon, label);
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        output_support::assert_document(&doc, id, &native["undo"], epsilon, label);
        doc.redo().unwrap();
        assert_eq!(
            doc.objects()
                .map(|o| (o.id(), o.geometry_snapshot().clone()))
                .collect::<Vec<_>>(),
            after
        );
        output_support::assert_document(&doc, id, &native["redo"], epsilon, label);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inset-outputs.3dm");
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        output_support::assert_document(&restored, id, &native["after_script"], epsilon, label);
        assert!(
            doc.objects()
                .all(|o| matches!(o.geometry(), Geometry::Brep(_) | Geometry::NurbsCurve(_)))
        );
    }
}

#[test]
fn output_modes_reject_native_collapses_and_replay_large_insets() {
    let mut rejected = 0;
    let mut supported = Vec::new();
    for (op, native) in common::cases_for("inset_output_limits") {
        if native["results"][0] == "Success" {
            supported.push((op, native));
            continue;
        }
        let (doc, id) = native_surface::fixture(&native);
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let options = Options {
            distance: op["distance"].as_f64().unwrap(),
            loose: op["loose"].as_bool().unwrap(),
            split: op["split"].as_bool().unwrap(),
            join_outputs: false,
            ..Options::default()
        };
        Plan::prepare(&doc, [(id, 0)], options).unwrap_err();
        assert_eq!(native["results"][0], "Failure");
        rejected += 1;
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        assert!(!doc.can_undo());
    }
    assert_eq!((rejected, supported.len()), (12, 4));
    replay(supported);
}
