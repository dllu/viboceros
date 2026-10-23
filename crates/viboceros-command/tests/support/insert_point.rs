//! Point witnesses, lifecycle evidence and explicit mesh representation comparisons.
use super::{mesh_native, native, subd_native};
pub const CELL_GAPS: [&str; 1] = ["insert_point_full_mesh_same_edge"];
pub const NGON_CELL_GAPS: [&str; 0] = [];
pub const DEPTH_CELL_GAPS: [&str; 0] = [];
pub const SPATIAL_CELL_GAPS: [&str; 3] = [
    "insert_point_spatial_bottom_mid_left_high",
    "insert_point_spatial_tilted_plane",
    "insert_point_spatial_same_edge_warp",
];
pub const SPATIAL_SURFACE_GAPS: [&str; 2] = [
    "insert_point_spatial_bottom_mid_left_high",
    "insert_point_spatial_same_edge_warp",
];
#[path = "insert_point/mesh.rs"]
mod mesh;
pub use mesh::{compare_mesh, compare_mesh_diagnostic};
use serde_json::{Value, json};
use viboceros_command::insert_point::{InsertAction, InsertSession};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{
    LineSegment, MeshProjectionIndex, Point3, PolygonEdgePoint, Tolerance, TriangleMesh, Vector3,
};
pub fn rows() -> Vec<Value> {
    serde_json::from_str::<Value>(include_str!(
        "../../../../tools/rhino_oracle/observations/insert_point_full.json"
    ))
    .unwrap()["results"]
        .as_array()
        .unwrap()
        .clone()
}
pub fn actions(v: &Value) -> Vec<InsertAction> {
    v["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, a)| {
            if a.is_object() {
                let click = v["input_trace"][0]["clicks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c["action"] == index)
                    .unwrap();
                InsertAction::Point(native::point(&click["projected_point"]))
            } else {
                match a.as_str().unwrap() {
                    "Undo" => InsertAction::Undo,
                    "Enter" => InsertAction::Finish,
                    "Cancel" => InsertAction::Cancel,
                    _ => panic!(),
                }
            }
        })
        .collect()
}
pub fn script(v: &Value) -> String {
    std::iter::once("InsertPoint".to_owned())
        .chain(actions(v).into_iter().map(|a| match a {
            InsertAction::Point(p) => p.to_array().map(|v| v.to_string()).join(","),
            InsertAction::Undo => "Undo".into(),
            InsertAction::Finish => "Enter".into(),
            InsertAction::Cancel => "Cancel".into(),
            _ => panic!(),
        }))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn edge_script(session: &InsertSession, v: &Value) -> String {
    std::iter::once("InsertPoint".to_owned())
        .chain(
            actions(v)
                .into_iter()
                .enumerate()
                .map(|(index, a)| match a {
                    InsertAction::Point(_) => {
                        let pick = captured_pick(session, v, index);
                        format!("Edge={} Parameter={:.17}", pick.edge, pick.parameter)
                    }
                    InsertAction::Undo => "Undo".into(),
                    InsertAction::Finish => "Enter".into(),
                    InsertAction::Cancel => "Cancel".into(),
                    _ => panic!(),
                }),
        )
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn stage(session: &mut InsertSession, doc: &Document, v: &Value) -> bool {
    session.prepare_picking().unwrap();
    for (index, action) in actions(v).into_iter().enumerate() {
        match action {
            InsertAction::Point(_) => {
                let pick = captured_pick(session, v, index);
                session
                    .insert(
                        doc,
                        pick,
                        Some(
                            Vector3::try_new(0., 0., 1.)
                                .unwrap()
                                .normalized_nonzero()
                                .unwrap(),
                        ),
                    )
                    .unwrap();
            }
            InsertAction::Undo => {
                session.undo_point(doc).unwrap();
            }
            InsertAction::Cancel => return false,
            InsertAction::Finish => break,
            _ => panic!(),
        }
    }
    true
}
/// Resolve source-surface depth or camera-target depth from independent input rays.
/// Old captures used an unchanged ZoomExtents view, whose target is the ray midpoint.
/// Generic SDK MeshRay witnesses use the shorter quad diagonal. Display picking
/// uses fixed 0-2 facets, computed here from the original source archive instead.
pub fn captured_pick(session: &InsertSession, v: &Value, action: usize) -> PolygonEdgePoint {
    let click = v["input_trace"][0]["clicks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["action"] == action)
        .unwrap();
    let screen = native::point(&click["projected_point"]);
    let mut pick = pick_in_top_view(session, screen);
    let [a, b] = [
        native::point(&click["ray"][0]),
        native::point(&click["ray"][1]),
    ];
    let direction = a.vector_to(b).unwrap();
    let mesh = session.source_pick_mesh().unwrap();
    let triangles = TriangleMesh::try_from_face_records(
        mesh.vertices().to_vec(),
        mesh.triangles()
            .iter()
            .copied()
            .map(viboceros_geometry::MeshFace::Triangle)
            .collect(),
    )
    .unwrap();
    let index = MeshProjectionIndex::new_owned(triangles).unwrap();
    let hits = index
        .project_point(a, direction, Tolerance::default())
        .unwrap();
    let surface = hits.last().copied();
    let fallback = if click["view_target_point"].is_array() {
        native::point(&click["view_target_point"])
    } else if click["view_target"].is_array() {
        let target = native::point(&click["view_target"]);
        let normal = native::point(&click["camera_direction"]).to_array();
        let dot = |v: [f64; 3]| v.into_iter().zip(normal).map(|(a, b)| a * b).sum::<f64>();
        let parameter = dot(a.vector_to(target).unwrap().to_array()) / dot(direction.to_array());
        a.translated(direction.scaled(parameter).unwrap()).unwrap()
    } else {
        a.translated(direction.scaled(0.5).unwrap()).unwrap()
    };
    pick.parameter = session.source_edges()[pick.edge]
        .closest_parameter(surface.unwrap_or(fallback), Tolerance::MESH_VALIDATION)
        .unwrap();
    pick
}
pub fn pick_in_top_view(session: &InsertSession, point: Point3) -> PolygonEdgePoint {
    let flat = |p: Point3| Point3::try_new(p.x(), p.y(), 0.).unwrap();
    let point = flat(point);
    session
        .source_edges()
        .iter()
        .enumerate()
        .filter_map(|(edge, line)| {
            let projected = LineSegment::try_new(
                flat(line.start()),
                flat(line.end()),
                Tolerance::MESH_VALIDATION,
            )
            .ok()?;
            let parameter = projected
                .closest_parameter(point, Tolerance::MESH_VALIDATION)
                .unwrap();
            Some((
                projected
                    .point_at(parameter)
                    .unwrap()
                    .distance_to(point)
                    .unwrap(),
                PolygonEdgePoint { edge, parameter },
            ))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}
pub fn check(doc: &Document, ids: &[ObjectId], rows: &Value, selection: bool) -> Value {
    native::check_metadata(doc, rows, selection);
    let target = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == 0)
        .unwrap();
    match doc.object(ids[0]).unwrap().geometry() {
        Geometry::Mesh(mesh) => compare_mesh(mesh, &target["mesh"]),
        Geometry::SubD(s) => {
            let result = subd_native::compare(s, &target["subd"]);
            json!({"qualified":true,"logical_regions":true,"channels":true,"vertices":s.control_points().len(),"faces":s.faces().len(),"maximum_error":result["maximum_control_or_limit_point_error"].as_f64().unwrap().max(result["maximum_sampled_surface_error"].as_f64().unwrap()),"stored_pieces_correspond":true,"stored_facets_correspond":true})
        }
        _ => panic!(),
    }
}
pub fn assert_known_cell_gap(id: &str, report: &Value) {
    assert_eq!(
        report["stored_pieces_correspond"],
        !CELL_GAPS.contains(&id),
        "native mesh cell gap set changed: {id}"
    );
    assert_eq!(
        report["stored_facets_correspond"],
        !CELL_GAPS.contains(&id),
        "native mesh facet gap set changed: {id}"
    );
}
pub fn assert_ngon_cell_gap(id: &str, report: &Value) {
    for flag in ["stored_pieces_correspond", "stored_facets_correspond"] {
        assert_eq!(
            report[flag],
            !NGON_CELL_GAPS.contains(&id),
            "n-gon cell gap changed: {id}"
        );
    }
}
pub fn assert_additional_cell_gap(id: &str, report: &Value) {
    assert_eq!(
        report["stored_pieces_correspond"],
        !DEPTH_CELL_GAPS.contains(&id)
    );
    assert_eq!(report["qualified"], true);
    if report.get("maximum_fixed_facet_sample_error").is_some() {
        assert_eq!(report["fixed_facet_surface_qualified"], true);
    }
    assert_eq!(report["stored_facets_correspond"], true);
}
pub fn check_scene(
    doc: &Document,
    ids: &[ObjectId],
    rows: &Value,
    selection: bool,
    family: &str,
) -> Value {
    if family != "insert_point_spatial_layouts" {
        return check(doc, ids, rows, selection);
    }
    native::check_metadata(doc, rows, selection);
    let target = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == 0)
        .unwrap();
    let Geometry::Mesh(mesh) = doc.object(ids[0]).unwrap().geometry() else {
        panic!()
    };
    let result = compare_mesh_diagnostic(mesh, &target["mesh"]);
    assert_eq!(result["logical_regions"], true);
    assert_eq!(result["ngon_overlays_correspond"], true);
    result
}
pub fn assert_spatial_result(id: &str, report: &Value) {
    assert_eq!(report["qualified"], !SPATIAL_SURFACE_GAPS.contains(&id));
    assert_eq!(
        report["fixed_facet_surface_qualified"],
        !SPATIAL_SURFACE_GAPS.contains(&id)
    );
    for field in ["stored_pieces_correspond", "stored_facets_correspond"] {
        assert_eq!(report[field], !SPATIAL_CELL_GAPS.contains(&id));
    }
    let maximum = report["maximum_error"]
        .as_f64()
        .unwrap()
        .max(report["maximum_fixed_facet_sample_error"].as_f64().unwrap());
    assert!(maximum.is_finite());
    assert_eq!(maximum <= 1e-6, !SPATIAL_SURFACE_GAPS.contains(&id));
}
