use super::*;
use viboceros_geometry::{CurveOffsetOperations, PlanarInsetOperations, PlanarInsetOptions};

#[test]
fn cusp_commands_match_native_outputs_metadata_history_and_3dm() {
    replay_commands("inset_cusp_command", (66, 6));
}
pub(super) fn replay_commands(family: &str, counts: (usize, usize)) {
    replay_command_cases(common::cases_for(family), counts);
}
pub(super) fn replay_command_cases(
    cases: Vec<(serde_json::Value, serde_json::Value)>,
    counts: (usize, usize),
) {
    let mut successes = Vec::new();
    let mut rejected = 0;
    for (op, native) in cases {
        if native["results"][0] == "Success" {
            successes.push((op, native));
            continue;
        }
        assert_eq!(native["results"][0], "Failure");
        let (doc, id) = native_surface::fixture(&native);
        let before = doc.object(id).unwrap().geometry_snapshot().clone();
        let options = viboceros_command::inset::Options {
            distance: op["distance"].as_f64().unwrap(),
            crease_corners: op["crease_corners"].as_bool().unwrap(),
            loose: op["loose"].as_bool().unwrap_or(false),
            split: op["split"].as_bool().unwrap(),
            join_outputs: op["join_outputs"].as_bool().unwrap(),
            ..Default::default()
        };
        viboceros_command::inset::Plan::prepare(&doc, [(id, 0)], options).unwrap_err();
        assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &before);
        assert!(!doc.can_undo());
        rejected += 1;
    }
    assert_eq!((successes.len(), rejected), counts);
    outputs::replay(successes);
}

#[test]
fn cusp_sdk_offsets_match_native_trimmed_loops() {
    replay(None);
}
#[cfg(feature = "native-smlib")]
#[test]
fn cusp_sdk_offsets_with_usd_brep_match_native_trimmed_loops() {
    replay(Some(viboceros_smlib::curve_offset_operations()));
}
fn replay(operations: Option<CurveOffsetOperations>) {
    replay_family("inset_cusp_sdk", (22, 2), operations);
}
pub(super) fn replay_family(
    family: &str,
    counts: (usize, usize),
    operations: Option<CurveOffsetOperations>,
) {
    replay_cases(common::cases_for(family), counts, operations);
}
pub(super) fn replay_cases(
    cases: Vec<(serde_json::Value, serde_json::Value)>,
    counts: (usize, usize),
    operations: Option<CurveOffsetOperations>,
) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap();
    let mut good = 0;
    let mut rejected = 0;
    for (op, native) in cases {
        let label = op["id"].as_str().unwrap();
        eprintln!("Inset SDK: {label}");
        let source = source(&native["before"]);
        let before = source.clone();
        let output = source.try_inset_planar_faces_with_options(
            &[0],
            PlanarInsetOptions {
                distance: op["distance"].as_f64().unwrap(),
                loose: op["loose"].as_bool().unwrap_or(false),
                crease_corners: op["crease_corners"].as_bool().unwrap(),
            },
            tolerance,
            PlanarInsetOperations {
                curves: operations,
                edit_points: native_edit_solver(operations.is_some()),
            },
        );
        assert_eq!(source, before, "{label}");
        if native["output"].is_null() {
            assert!(output.is_err(), "{label}");
            rejected += 1;
            continue;
        }
        let output = output.unwrap_or_else(|e| panic!("{label}: {e}"));
        native_surface::assert_geometry_with_epsilon(
            &output,
            &native["output"]["geometry"],
            tolerance,
            native_surface::epsilon_for(&op),
            label,
        );
        assert!(output.faces().iter().all(|f| {
            source
                .faces()
                .iter()
                .any(|old| old.surface() == f.surface())
        }));
        good += 1;
    }
    assert_eq!((good, rejected), counts);
}
