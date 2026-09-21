//! Test-only camera reconstruction shared by constrained and free-point probes.
use crate::*;

pub(crate) fn projection(frame: &Value) -> impl Fn(Point3) -> Option<[f64; 2]> {
    let camera = projected_object_snap::SnapCamera {
        world_to_screen: serde_json::from_value(frame["world_to_screen"].clone()).unwrap(),
        location: serde_json::from_value(frame["camera_location"].clone()).unwrap(),
        direction: serde_json::from_value(frame["camera_direction"].clone()).unwrap(),
    };
    move |point| camera.project(point)
}

#[test]
fn unconstrained_snaps_validate_full_targets_and_expose_corner_edge_priority_differences() {
    use viboceros_drafting::{ObjectSnapCache, ObjectSnapKind, ObjectSnapModes, ObjectSnapOptions};
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/point_snaps.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/point_snaps.json"
    ))
    .unwrap();
    let targets: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/point_snap_targets.json"
    ))
    .unwrap();
    let (mut matches, mut misses, mut priority_differences) = (0, 0, 0);
    let operations = request["operations"].as_array().unwrap();
    let rows = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 101);
    assert_eq!(operations.len(), rows.len());
    for (operation, row) in operations.iter().zip(rows) {
        let id = operation["id"].as_str().unwrap();
        assert_eq!(row["id"], id);
        let value = &row["value"];
        let mut doc = Document::default();
        let source: object_source::ObjectSource =
            serde_json::from_value(operation["sources"][0].clone()).unwrap();
        let object = doc
            .add_geometry(source.geometry(Tolerance::DEFAULT).unwrap())
            .unwrap();
        let mut modes = ObjectSnapModes::NONE;
        for name in operation["persistent_snaps"].as_array().unwrap() {
            modes = modes.with(
                match name.as_str().unwrap() {
                    "Mid" => ObjectSnapKind::Mid,
                    "Near" => ObjectSnapKind::Near,
                    _ => panic!("unexpected calibration mode"),
                },
                true,
            );
        }
        let frame = &value["frame"];
        let cursor = serde_json::from_value(frame["click_client"].clone()).unwrap();
        let radius = operation["capture_radius"].as_f64().unwrap_or(12.);
        let snap = ObjectSnapCache::default()
            .nearest_projected_with_options(
                &doc,
                cursor,
                radius,
                projection(frame),
                ObjectSnapOptions {
                    modes,
                    mesh_edges: operation["snap_to_meshes"].as_bool().unwrap(),
                },
            )
            .unwrap();
        assert_eq!(value["before"], value["after"], "{id}");
        assert_eq!(
            value["mesh_snap_setting"]["before"], value["mesh_snap_setting"]["restored"],
            "{id}"
        );
        if targets[id].is_null() {
            assert!(snap.is_none(), "{id}: unexpected {snap:?}");
            assert_eq!(value["kind"], "None", "{id}");
            assert!(value["source"].is_null(), "{id}");
            misses += 1;
            continue;
        }
        let snap = snap.unwrap_or_else(|| panic!("{id}: missing capture"));
        assert_eq!(snap.object_id(), object, "{id}");
        assert_eq!(value["source"], 0, "{id}");
        let name = match snap.kind() {
            ObjectSnapKind::Near => "Near",
            ObjectSnapKind::Mid => "Midpoint",
            _ => panic!(),
        };
        assert_eq!(value["kind"], name, "{id}");
        assert_eq!(targets[id]["kind"], name, "{id}");
        let point = |v: &Value| {
            Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
        };
        assert!(
            snap.point()
                .distance_to(point(&targets[id]["point"]))
                .unwrap()
                < 1e-9,
            "{id}"
        );
        let rhino = point(&value["point"]);
        if matches!(
            id,
            "threshold-tilted--10-0" | "threshold-tilted--9-0" | "aperture-tilted--8-0-r16"
        ) {
            // Rhino chooses an adjacent wire's endpoint, not the closest
            // screen target on the hovered wire. Retain the entire discrepancy.
            assert_eq!(rhino, point(&operation["sources"][0]["vertices"][0]));
            assert!(snap.point().distance_to(rhino).unwrap() > 0.5, "{id}");
            let image = projection(frame)(rhino).unwrap();
            let distance = (image[0] - cursor[0]).hypot(image[1] - cursor[1]);
            assert!(snap.distance() < distance && distance < radius, "{id}");
            if !value["component"].is_null() {
                assert_eq!(
                    value["component"],
                    json!({"type":"MeshTopologyEdge", "index":1})
                );
            }
            priority_differences += 1;
        } else {
            assert!(
                snap.point().distance_to(rhino).unwrap() < 1e-9,
                "{id}: {:?} != {rhino:?}",
                snap.point()
            );
            matches += 1;
        }
    }
    assert_eq!((matches, misses, priority_differences), (90, 8, 3));
}
