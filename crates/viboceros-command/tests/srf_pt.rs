#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/srf_pt.rs"]
mod replay;
use viboceros_command::{CommandContext, CommandRegistry};

#[test]
fn corner_surface_spaced_calculator_points_and_alias_keep_native_geometry() {
    let data = native::read("srf_pt_origin");
    for (name, script) in [
        (
            "zero_quad",
            "SurfaceFromCorners 0 0 0 8/2 0 0 8/2 10/2 0 0 10/2 0",
        ),
        ("zero_triangle", "SrfPt 0 0 0 8/2 0 0 2/2 10/2 0 Enter"),
    ] {
        let row = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == name)
            .unwrap();
        let v = &row["value"];
        let mut doc = native::setup(v);
        CommandRegistry::with_builtins()
            .execute(&mut doc, script)
            .unwrap();
        assert!(replay::check(&doc, v, "after") <= 1e-6);
    }
}

#[test]
fn corner_surfaces_match_native_charts_trims_topology_and_history() {
    replay_families(
        &[
            "srf_pt_pilot",
            "srf_pt_holdouts",
            "srf_pt_precision",
            "srf_pt_admission",
            "srf_pt_singular",
            "srf_pt_origin",
        ],
        60,
        "SRF_PT_SCRIPT",
    );
}

#[test]
fn corner_surfaces_remote_admission_and_degenerate_orientation_followup() {
    replay_families(&["srf_pt_followup"], 37, "SRF_PT_FOLLOWUP_SCRIPT");
}

fn replay_families(families: &[&str], expected: usize, prefix: &str) {
    let registry = CommandRegistry::with_builtins();
    let mut completed = 0;
    for &family in families {
        let data = native::read(family);
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            let allow_ambiguous = replay::FACE_SENSE_GAPS.contains(&row["id"].as_str().unwrap());
            let mut doc = native::setup(v);
            if v["success"] != true {
                registry.execute(&mut doc, "Point 100,200,300").unwrap();
                doc.undo().unwrap();
            }
            let parents = doc.objects().cloned().collect::<Vec<_>>();
            let command = format!("SrfPt {}", replay::tokens(v).join(" "));
            let result = registry.execute_in_context(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: native::frame(v),
                },
            );
            assert_eq!(
                result.is_ok(),
                v["success"] == true,
                "{} {result:?}",
                row["id"]
            );
            let (mut error, face_sense_matches) =
                replay::check_with_face_sense(&doc, v, "after", allow_ambiguous);
            if v["success"] == true {
                assert_eq!(doc.undo_label(), Some("SrfPt"));
                doc.undo().unwrap();
                error = error.max(replay::check(&doc, v, "undo"));
                assert!(doc.undo().unwrap().is_none());
                doc.redo().unwrap();
                error =
                    error.max(replay::check_with_face_sense(&doc, v, "redo", allow_ambiguous).0);
            } else {
                assert!(!doc.can_undo());
                assert!(doc.can_redo());
            }
            for parent in parents {
                assert_eq!(doc.object(parent.id()), Some(&parent));
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(doc.object(parent.id()).unwrap().geometry_snapshot())
                );
            }
            println!(
                "{prefix} {}",
                serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error,"face_sense_matches_native":face_sense_matches})
            );
            assert!(error <= 1e-6, "{} {error:e}", row["id"]);
            completed += 1;
        }
    }
    assert_eq!(completed, expected);
}
