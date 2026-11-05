use super::*;
use viboceros_oracle::revolve_splitting::{Fixture, adjacency, boundary_samples, snapshot};

#[path = "../../../crates/viboceros-oracle/tests/support/revolve_splitting.rs"]
mod topology;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn revolve_splitting_axis_getters_match_native_partitions_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut total = 0;
    for family in [
        "revolve_splitting",
        "revolve_splitting_holdouts",
        "revolve_splitting_mixed",
    ] {
        let request: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = super::oracle_observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            total += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let mut app = test_app();
            let (doc, source) =
                viboceros_oracle::revolve_deformable::setup(&f.profile, app.document.tolerance())
                    .unwrap();
            app.document = doc;
            enter(
                &mut app,
                if f.crease_splitting {
                    "CreaseSplitting Enable"
                } else {
                    "CreaseSplitting Disable"
                },
            );
            let compare = |app: &VibocerosApp, phase: &str| {
                super::edge_analysis_mixed_support::compare(
                    &snapshot(&app.document, source).unwrap(),
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                );
            };
            compare(&app, "before");
            let yes = |b| if b { "Yes" } else { "No" };
            enter(
                &mut app,
                &format!(
                    "Revolve Deformable={} PointCount={} Angle={} StartAngle={} DeleteInput={}",
                    yes(f.deformable),
                    f.profile.point_count,
                    f.profile.sweep_degrees,
                    f.profile.start_degrees,
                    yes(f.profile.delete_input)
                ),
            );
            let p = f.profile.origin;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            assert!(app.active_command.is_some());
            enter(
                &mut app,
                &format!("SplitAtTangents={}", yes(f.split_at_tangents)),
            );
            let p = f.profile.axis_end;
            app.accept_drafting_point(point(p[0], p[1], p[2]));
            assert!(app.active_command.is_none());
            compare(&app, "after");
            assert_eq!(
                adjacency(&app.document, source).unwrap(),
                topology::adjacency(&row["value"]["full"])
            );
            if total == 1 {
                topology::compare_boundaries(
                    &boundary_samples(&app.document, source).unwrap(),
                    &row["value"]["full"],
                    "interactive boundary",
                );
            }
            assert_eq!(app.document.undo_label(), Some("Revolve"));
            enter(&mut app, "Undo");
            compare(&app, "undo");
            enter(&mut app, "Redo");
            compare(&app, "redo");
        }
    }
    assert_eq!(total, 160);
}
