#[path = "support/subd_sources.rs"]
mod source_archives;
use serde_json::{Value, json};
use viboceros_geometry::{SubD, SubDExtrudeDirection as Direction};
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/subd_extrude.rs"]
mod support;
fn value(s: &SubD) -> Value {
    json!({"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":s.faces(),"edges":s.edges(),"vertex_tags":s.vertex_tags(),"edge_tags":s.edge_tags(),"valid":true})
}
#[test]
fn extrusion_controls_topology_tags_and_limit_points_match_128_native_commands() {
    let sources = support::sources();
    for family in [
        "subd_extrude_command",
        "subd_extrude_extra",
        "subd_extrude_mixed",
    ] {
        for (op, native) in common::cases_for(family) {
            let target = op["target"].as_str().unwrap();
            let scene = target.split('_').next().unwrap();
            let source = &sources[scene];
            let archive = source.archive().to_vec();
            let direction = match op["mode"].as_str().unwrap() {
                "N" => Direction::VertexNormals,
                "V" => Direction::EdgeOutward,
                _ => Direction::World(viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap()),
            };
            let output = source
                .try_extrude(
                    &support::selection(target),
                    op["distance"].as_f64().unwrap(),
                    direction,
                )
                .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
            assert_eq!(source.archive(), archive);
            let mut expected = native["after"][0]["subd"].clone();
            expected.as_object_mut().unwrap().remove("surface_mesh");
            common::compare(&value(&output), &expected, op["id"].as_str().unwrap());
        }
    }
}

#[test]
fn native_extrusion_commands_preserve_metadata_selection_and_history() {
    use viboceros_command::{
        CommandRegistry,
        extrude_subd::{Plan, Target},
    };
    use viboceros_document::SelectionMode;
    use viboceros_oracle::subd_join::snapshot;
    let sources = support::sources();
    for family in [
        "subd_extrude_command",
        "subd_extrude_extra",
        "subd_extrude_mixed",
    ] {
        for (op, native) in common::cases_for(family) {
            let label = op["id"].as_str().unwrap();
            let target = op["target"].as_str().unwrap();
            let (mut doc, id) = support::fixture(&sources[target.split('_').next().unwrap()]);
            let selection = support::selection(target);
            let targets = match selection {
                viboceros_geometry::SubDExtrudeSelection::AllFaces => {
                    doc.select_objects_direct([id], SelectionMode::Replace)
                        .unwrap();
                    vec![Target::Whole(id)]
                }
                viboceros_geometry::SubDExtrudeSelection::Components { faces, edges } => faces
                    .into_iter()
                    .map(|f| Target::Face(id, f))
                    .chain(edges.into_iter().map(|e| Target::Edge(id, e)))
                    .collect(),
            };
            common::compare(
                &snapshot(&doc, &[id]),
                &support::expected(&native["before"]),
                label,
            );
            let options = support::options(&op);
            if target.ends_with("whole") {
                CommandRegistry::with_builtins()
                    .execute(&mut doc, &options.command())
                    .unwrap();
            } else {
                Plan::prepare(&doc, targets, options)
                    .unwrap()
                    .commit(&mut doc)
                    .unwrap();
            }
            common::compare(
                &snapshot(&doc, &[id]),
                &support::expected(&native["after"]),
                label,
            );
            doc.clear_selection();
            common::compare(
                &snapshot(&doc, &[id]),
                &support::expected(&native["after_script"]),
                label,
            );
            if doc.can_undo() {
                doc.undo().unwrap();
            } else {
                assert_eq!(target, "grid_mixed");
                assert_eq!(op["mode"], "Z");
            }
            common::compare(
                &snapshot(&doc, &[id]),
                &support::expected(&native["undo"]),
                label,
            );
            if doc.can_redo() {
                doc.redo().unwrap();
            } else {
                assert_eq!(target, "grid_mixed");
                assert_eq!(op["mode"], "Z");
            }
            common::compare(
                &snapshot(&doc, &[id]),
                &support::expected(&native["redo"]),
                label,
            );
        }
    }
}

#[test]
fn refined_extrusions_retain_soft_sharpness_through_native_3dm_roundtrip() {
    use viboceros_document::Geometry;
    let sources = support::sources();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extruded.3dm");
    let source = sources["bent"].subdivided(1).unwrap();
    let edge = source
        .edge_face_counts()
        .iter()
        .position(|&n| n == 2)
        .unwrap();
    let source = source
        .try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
        .unwrap()
        .0;
    for selection in [
        viboceros_geometry::SubDExtrudeSelection::AllFaces,
        viboceros_geometry::SubDExtrudeSelection::Components {
            faces: vec![0],
            edges: vec![],
        },
    ] {
        let result = source
            .try_extrude(&selection, -0.75, Direction::VertexNormals)
            .unwrap();
        assert_eq!(result.edge_sharpness()[edge], source.edge_sharpness()[edge]);
        let (doc, id) = support::fixture(&result);
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        let Geometry::SubD(s) = restored.object(id).unwrap().geometry() else {
            panic!()
        };
        common::compare(&value(s), &value(&result), "refined extrusion roundtrip");
        assert_eq!(s.edge_sharpness(), result.edge_sharpness());
    }
}
