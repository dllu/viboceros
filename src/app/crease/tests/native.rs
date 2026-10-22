//! Getter replay uses saved native geometry; physical native gestures are separate.
use super::*;
use crate::app::{subd_native_tests as subd_native, subdivision_native_tests as mesh_native};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use viboceros_command::crease::CreaseTarget;
use viboceros_geometry::{PointCloud3, PointCloudProjection};
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-command/tests/support/crease.rs"]
pub(super) mod fixture;

fn pick_kind(document: &Document, target: CreaseTarget) -> ComponentPick {
    match target {
        CreaseTarget::Vertex(object, index) => ComponentPick {
            object,
            index,
            kind: ComponentSelectionKind::SubDVertex,
        },
        CreaseTarget::Edge(object, index) => ComponentPick {
            object,
            index,
            kind: if matches!(
                document.object(object).unwrap().geometry(),
                Geometry::SubD(_)
            ) {
                ComponentSelectionKind::SubDEdge
            } else {
                ComponentSelectionKind::MeshEdge
            },
        },
    }
}
fn check_components(app: &VibocerosApp, ids: &[ObjectId], rows: &Value) {
    let expected = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o.get("subd").is_some())
        .flat_map(|o| {
            let object = ids[o["source"].as_u64().unwrap() as usize];
            [
                (&o["selected_vertices"], ComponentSelectionKind::SubDVertex),
                (&o["selected_edges"], ComponentSelectionKind::SubDEdge),
            ]
            .into_iter()
            .flat_map(move |(indices, kind)| {
                indices
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(move |i| ComponentPick {
                        object,
                        kind,
                        index: i.as_u64().unwrap() as usize,
                    })
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        app.component_selection
            .checked_picks(&app.document)
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>(),
        expected
    );
}
fn edge_sample_error(document: &Document, ids: &[ObjectId], rows: &Value) -> f64 {
    let mut maximum = 0f64;
    for obj in rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|o| o.get("subd").is_some())
    {
        let id = ids[obj["source"].as_u64().unwrap() as usize];
        let Geometry::SubD(subd) = document.object(id).unwrap().geometry() else {
            panic!("source SubD")
        };
        let mesh = &obj["subd"]["surface_mesh"];
        let points = mesh["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(fixture::point)
            .collect::<Vec<_>>();
        let cloud = PointCloud3::try_new(points.clone()).unwrap();
        let mut neighbors = vec![BTreeSet::new(); points.len()];
        for face in mesh["faces"].as_array().unwrap() {
            let v = face.as_array().unwrap();
            for i in 0..v.len() {
                let a = v[i].as_u64().unwrap() as usize;
                let b = v[(i + 1) % v.len()].as_u64().unwrap() as usize;
                neighbors[a].insert(b);
                neighbors[b].insert(a);
            }
        }
        let lines = subd.edge_polylines(2).unwrap();
        assert_eq!(lines.len(), subd.edges().len());
        for (i, line) in lines.iter().enumerate() {
            let edge = &obj["subd"]["edges"][i];
            for (point, endpoint) in [(line.first().unwrap(), 0), (line.last().unwrap(), 1)] {
                maximum = maximum.max(
                    point
                        .distance_to(fixture::point(
                            &obj["subd"]["surface_points"]
                                [edge[endpoint].as_u64().unwrap() as usize],
                        ))
                        .unwrap(),
                );
            }
            for pair in line.windows(2) {
                let (index, a, _) = cloud
                    .nearest_visible_projected_relative_with_filter(
                        PointCloudProjection::Xy,
                        pair[0],
                        [0.; 2],
                        1e-6,
                        |i, p| {
                            p.distance_to(pair[0]).unwrap() <= 1e-6
                                && neighbors[i]
                                    .iter()
                                    .any(|&j| points[j].distance_to(pair[1]).unwrap() <= 1e-6)
                        },
                    )
                    .unwrap()
                    .expect("sampled original edge must follow a captured native facet edge");
                let b = neighbors[index]
                    .iter()
                    .map(|&j| points[j].distance_to(pair[1]).unwrap())
                    .fold(f64::INFINITY, f64::min);
                maximum = maximum.max(a.distance_to(pair[0]).unwrap()).max(b);
            }
        }
    }
    assert!(maximum <= 1e-6);
    maximum
}
#[test]
fn preselected_and_postselected_getters_replay_all_saved_native_crease_workflows() {
    let mut failures = Vec::new();
    for phase in ["preselected", "postselected"] {
        for row in fixture::rows() {
            let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let value = &row["value"];
                let (document, ids) = fixture::setup(&row);
                let mut app = test_app();
                app.document = document;
                let mut maximum = fixture::check(&app.document, &value["before"], true);
                let mut edge_error = edge_sample_error(&app.document, &ids, &value["before"]);
                let original = ids
                    .iter()
                    .map(|&id| app.document.object(id).unwrap().attributes().clone())
                    .collect::<Vec<_>>();
                let targets = fixture::targets(&app.document, &ids, value);
                if phase == "postselected" {
                    app.document.clear_selection();
                    enter(&mut app, fixture::mode(value).command());
                    assert!(app.crease_prompt.is_some());
                }
                for target in targets {
                    if phase == "preselected" && value["input"]["grips"] == true {
                        continue;
                    }
                    let pick = pick_kind(&app.document, target);
                    select(
                        &mut app,
                        pick.object,
                        pick.kind,
                        pick.index,
                        phase == "preselected",
                    );
                }
                enter(
                    &mut app,
                    if phase == "preselected" {
                        fixture::mode(value).command()
                    } else {
                        ""
                    },
                );
                assert!(app.crease_prompt.is_none(), "{:?}", app.command_log);
                for (&id, attrs) in ids.iter().zip(&original) {
                    assert_eq!(app.document.object(id).unwrap().attributes(), attrs);
                }
                maximum = maximum.max(fixture::check(&app.document, &value["after"], true));
                edge_error =
                    edge_error.max(edge_sample_error(&app.document, &ids, &value["after"]));
                check_components(&app, &ids, &value["after"]);
                // Reproduce the producer's explicit post-command cleanup,
                // independently of any physical Escape-gesture parity claim.
                app.component_selection.clear();
                app.document.disable_control_points();
                maximum = maximum.max(fixture::check(&app.document, &value["after_script"], true));
                check_components(&app, &ids, &value["after_script"]);
                enter(&mut app, "Undo");
                maximum = maximum.max(fixture::check(&app.document, &value["undo"], true));
                check_components(&app, &ids, &value["undo"]);
                enter(&mut app, "Redo");
                maximum = maximum.max(fixture::check(&app.document, &value["redo"], true));
                check_components(&app, &ids, &value["redo"]);
                println!(
                    "CREASE_GETTER {}",
                    json!({"id":row["id"],"phase":phase,"maximum_error":maximum,"maximum_edge_sample_error":edge_error,"identity":true,"attributes":true,"history":true,"selection":true})
                );
            }));
            if run.is_err() {
                failures.push(format!("{phase}:{}", row["id"]));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "crease getter differences: {failures:?}"
    );
}
